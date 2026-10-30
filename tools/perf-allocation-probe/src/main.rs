//! 独立分配计数探针；保留 mimalloc，不将插桩耗时解释为吞吐性能。
use std::alloc::{GlobalAlloc, Layout};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use tswn_core::namerena::eval_name::WIN_RATE_EVAL_RQ;
use tswn_core::runtime::{RuntimeRunner, prepared_runtime_win_rate_range, runtime_groups_win_rate};

struct CountedMimalloc;
#[global_allocator]
static ALLOCATOR: CountedMimalloc = CountedMimalloc;
static ALLOC: AtomicU64 = AtomicU64::new(0);
static ZEROED: AtomicU64 = AtomicU64::new(0);
static REALLOC: AtomicU64 = AtomicU64::new(0);
static DEALLOC: AtomicU64 = AtomicU64::new(0);
static FAILED: AtomicU64 = AtomicU64::new(0);
static REQUESTED: AtomicU64 = AtomicU64::new(0);
static RELEASED: AtomicU64 = AtomicU64::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn added(size: usize) {
    REQUESTED.fetch_add(size as u64, Ordering::Relaxed);
    let live = LIVE.fetch_add(size, Ordering::Relaxed) + size;
    PEAK.fetch_max(live, Ordering::Relaxed);
}
fn removed(size: usize) {
    RELEASED.fetch_add(size as u64, Ordering::Relaxed);
    LIVE.fetch_sub(size, Ordering::Relaxed);
}

// SAFETY: 原样转发全部指针、布局和尺寸给 MiMalloc；计数器不分配内存、不解引用指针。
unsafe impl GlobalAlloc for CountedMimalloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOC.fetch_add(1, Ordering::Relaxed);
        let ptr = unsafe { mimalloc::MiMalloc.alloc(layout) };
        if ptr.is_null() {
            FAILED.fetch_add(1, Ordering::Relaxed);
        } else {
            added(layout.size());
        }
        ptr
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOC.fetch_add(1, Ordering::Relaxed);
        ZEROED.fetch_add(1, Ordering::Relaxed);
        let ptr = unsafe { mimalloc::MiMalloc.alloc_zeroed(layout) };
        if ptr.is_null() {
            FAILED.fetch_add(1, Ordering::Relaxed);
        } else {
            added(layout.size());
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        DEALLOC.fetch_add(1, Ordering::Relaxed);
        unsafe { mimalloc::MiMalloc.dealloc(ptr, layout) };
        removed(layout.size());
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        REALLOC.fetch_add(1, Ordering::Relaxed);
        let next = unsafe { mimalloc::MiMalloc.realloc(ptr, layout, size) };
        if next.is_null() {
            FAILED.fetch_add(1, Ordering::Relaxed);
        } else {
            removed(layout.size());
            added(size);
        }
        next
    }
}

#[derive(Clone, Copy)]
struct Snapshot {
    counts: [u64; 7],
    live: usize,
    peak: usize,
}
impl Snapshot {
    fn read() -> Self {
        Self {
            counts: [&ALLOC, &ZEROED, &REALLOC, &DEALLOC, &FAILED, &REQUESTED, &RELEASED]
                .map(|counter| counter.load(Ordering::Relaxed)),
            live: LIVE.load(Ordering::Relaxed),
            peak: PEAK.load(Ordering::Relaxed),
        }
    }
    fn begin() -> Self {
        PEAK.store(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
        Self::read()
    }
}

fn files(input: &Path) -> std::io::Result<Vec<PathBuf>> {
    if !input.is_dir() {
        return Ok(vec![input.to_owned()]);
    }
    let mut paths = std::fs::read_dir(input)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "txt"));
    paths.sort();
    Ok(paths)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = None;
    let mut start = 0usize;
    let mut runs = 1000usize;
    let mut threads = 1u32;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or("参数缺少值；支持 --input --start --runs --threads")?;
        match flag.as_str() {
            "--input" | "-i" => input = Some(PathBuf::from(value)),
            "--start" => start = value.parse()?,
            "--runs" | "-n" => runs = value.parse()?,
            "--threads" => threads = value.parse()?,
            _ => return Err(format!("未知参数：{flag}").into()),
        }
    }
    if runs == 0 || (start != 0 && threads != 1) {
        return Err("runs 必须大于零；非零 start 仅支持 threads=1".into());
    }
    let end = start.checked_add(runs).ok_or("轮次区间溢出")?;
    let paths = files(&input.ok_or("需要 --input FILE_OR_DIRECTORY")?)?;
    if paths.is_empty() {
        return Err("目录中没有 txt 输入".into());
    }
    for path in paths {
        let (groups, _) = RuntimeRunner::split_namerena_into_groups(std::fs::read_to_string(&path)?);
        if groups.is_empty() {
            return Err("输入没有分组".into());
        }
        let group_sizes: Vec<_> = groups.iter().map(Vec::len).collect();
        let before = Snapshot::begin();
        let result = if start == 0 {
            runtime_groups_win_rate(&groups, runs, WIN_RATE_EVAL_RQ, threads)
        } else {
            // 构造与析构 prepared 都在计数窗口内；range API 使用原始轮次种子。
            let prepared = RuntimeRunner::prepare_groups_with_eval_rq(&groups, WIN_RATE_EVAL_RQ)?;
            prepared_runtime_win_rate_range(&prepared, start, end)
        };
        let after = Snapshot::read();
        let summary = result?;
        let delta: [u64; 7] = std::array::from_fn(|i| after.counts[i] - before.counts[i]);
        println!(
            "{}",
            serde_json::json!({
                "input": path, "group_sizes": group_sizes, "start_inclusive": start, "end_exclusive": end,
                "runs_per_matchup": runs, "threads": threads, "allocator": "counted-mimalloc",
                "scope": "parse-excluded-prepare-run-drop-included", "total": summary.total, "wins": summary.wins,
                "errors": summary.errors, "guard_exhausted": summary.guard_exhausted,
                "alloc_calls": delta[0], "alloc_zeroed_calls": delta[1], "realloc_calls": delta[2],
                "dealloc_calls": delta[3], "failed_allocation_calls": delta[4],
                "allocated_bytes": delta[5], "deallocated_bytes": delta[6],
                "live_bytes_before": before.live, "live_bytes_after": after.live,
                "net_live_bytes": after.live as i128 - before.live as i128,
                "peak_live_bytes": after.peak, "peak_extra_live_bytes": after.peak.saturating_sub(before.live)
            })
        );
        if summary.errors != 0 || delta[4] != 0 {
            return Err("对局或分配发生错误".into());
        }
    }
    Ok(())
}

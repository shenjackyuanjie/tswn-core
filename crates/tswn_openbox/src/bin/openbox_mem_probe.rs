use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tswn_openbox::backend::{BatchRateInput, CommonBenchOptions, OutputMode, ProgressEvent, run_batch_rate};

#[derive(Default)]
struct Stats {
    logs: usize,
    log_bytes: usize,
    highlights: usize,
    progress: usize,
    done: usize,
    last_done: usize,
    total: usize,
}

fn main() {
    let args = Args::parse();
    let players = read_limited_lines(&args.players, args.limit);
    let targets = read_limited_lines(&args.targets, args.target_limit);
    eprintln!(
        "probe start players={} targets={} count={} threads={:?} show_matchups={} rss_kb={}",
        players.lines().count(),
        targets.lines().count(),
        args.count,
        args.threads,
        args.show_matchups,
        current_rss_kb()
    );

    let cancel = Arc::new(AtomicBool::new(false));
    let started = Instant::now();
    let last_report = Mutex::new(Instant::now());
    let stats = Mutex::new(Stats::default());
    let input = BatchRateInput {
        target_text: targets,
        player_text: players,
        player_double_plus: false,
        show_matchups: args.show_matchups,
        highlight_delta: None,
        output_mode: OutputMode::Log,
        output_file: None,
        options: CommonBenchOptions {
            count: args.count,
            threads: args.threads,
            keep_rq: true,
            verbose: false,
            min_screen: None,
            min_file: None,
            wr_precision: 3,
        },
        cancel: Arc::clone(&cancel),
    };

    run_batch_rate(input, |event| {
        let mut stats = stats.lock().expect("stats lock poisoned");
        match event {
            ProgressEvent::Log(line) => {
                stats.logs += 1;
                stats.log_bytes += line.len();
            }
            ProgressEvent::HighlightLog(line) | ProgressEvent::SkillBoardLog(line) => {
                stats.highlights += 1;
                stats.log_bytes += line.len();
            }
            ProgressEvent::Progress { done, total } => {
                stats.progress += 1;
                stats.last_done = done;
                stats.total = total;
            }
            ProgressEvent::Done(result) => {
                stats.done += 1;
                eprintln!("done={result:?}");
            }
        }
        let mut last_report = last_report.lock().expect("report lock poisoned");
        if last_report.elapsed() >= args.report_every {
            report(started, &stats);
            *last_report = Instant::now();
        }
    });
    report(started, &stats.lock().expect("stats lock poisoned"));
    cancel.store(true, Ordering::Relaxed);
}

struct Args {
    players: PathBuf,
    targets: PathBuf,
    limit: Option<usize>,
    target_limit: Option<usize>,
    count: usize,
    threads: Option<usize>,
    show_matchups: bool,
    report_every: Duration,
}

impl Args {
    fn parse() -> Self {
        let mut args = std::env::args().skip(1);
        let mut parsed = Self {
            players: PathBuf::from("tests/allCO3pure.txt"),
            targets: PathBuf::from("crates/tswn_openbox/assets/targets/target2.txt"),
            limit: Some(2000),
            target_limit: Some(8),
            count: 1,
            threads: Some(8),
            show_matchups: false,
            report_every: Duration::from_secs(2),
        };
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--players" => parsed.players = PathBuf::from(args.next().expect("--players needs a path")),
                "--targets" => parsed.targets = PathBuf::from(args.next().expect("--targets needs a path")),
                "--limit" => parsed.limit = parse_optional_usize(args.next().expect("--limit needs a value")),
                "--target-limit" => parsed.target_limit = parse_optional_usize(args.next().expect("--target-limit needs a value")),
                "--count" => parsed.count = args.next().expect("--count needs a value").parse().expect("invalid --count"),
                "--threads" => parsed.threads = parse_optional_usize(args.next().expect("--threads needs a value")),
                "--show-matchups" => parsed.show_matchups = true,
                "--report-ms" => {
                    let millis = args.next().expect("--report-ms needs a value").parse().expect("invalid --report-ms");
                    parsed.report_every = Duration::from_millis(millis);
                }
                other => panic!("unknown arg: {other}"),
            }
        }
        parsed
    }
}

fn parse_optional_usize(raw: String) -> Option<usize> {
    match raw.as_str() {
        "all" | "none" | "0" => None,
        _ => Some(raw.parse().expect("invalid integer")),
    }
}

fn read_limited_lines(path: &PathBuf, limit: Option<usize>) -> String {
    let content = std::fs::read_to_string(path).unwrap_or_else(|err| panic!("read failed {}: {err}", path.display()));
    let mut out = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(limit.unwrap_or(usize::MAX))
        .collect::<Vec<_>>()
        .join("\n");
    out.push('\n');
    out
}

fn report(started: Instant, stats: &Stats) {
    eprintln!(
        "t={:.1}s rss_kb={} progress={}/{} events={} logs={} log_kb={} highlights={}",
        started.elapsed().as_secs_f64(),
        current_rss_kb(),
        stats.last_done,
        stats.total,
        stats.progress,
        stats.logs,
        stats.log_bytes / 1024,
        stats.highlights,
    );
}

#[cfg(windows)]
fn current_rss_kb() -> u64 {
    let pid = std::process::id();
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("(Get-Process -Id {pid}).WorkingSet64"),
        ])
        .output();
    output
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|text| text.trim().parse::<u64>().ok())
        .map(|bytes| bytes / 1024)
        .unwrap_or(0)
}

#[cfg(not(windows))]
fn current_rss_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find_map(|line| line.strip_prefix("VmRSS:"))
                .and_then(|line| line.split_whitespace().next())
                .and_then(|kb| kb.parse::<u64>().ok())
        })
        .unwrap_or(0)
}

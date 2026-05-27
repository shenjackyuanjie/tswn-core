//! 性能样本筛选与 benchmark 工具。
//!
//! 从 case 库中抽样生成多种对局模式，先用少量运行做筛选，再对代表性慢 case 执行高次数胜率 benchmark。
//! 结果写入 `target/perf_cases`，用于稳定复现性能退化和比较优化前后的耗时。

use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tswn_core::Runner;
use tswn_core::case_gen::{
    CaseMode, GeneratedCase, case_id, deterministic_shuffle, generate_cases_for_mode, load_library, stable_hash,
};
use tswn_core::player::eval_name::WIN_RATE_EVAL_RQ;
use tswn_core::win_rate::{WinRateSummary, prepared_win_rate};

const DEFAULT_LIBRARY: &str = "tests/sqp6000.txt";
const DEFAULT_OUT_DIR: &str = "target/perf_cases";
const DEFAULT_CASE_OFFSET_PER_MODE: usize = 0;
const DEFAULT_MAX_CASES_PER_MODE: usize = 4000;
const DEFAULT_SAMPLE_RUNS: usize = 64;
const DEFAULT_BENCH_RUNS: usize = 500_000;
const DEFAULT_SELECT_COUNT: usize = 20;
const DEFAULT_SHUFFLE_SEED: u64 = 0x5EED_2026;

#[derive(Debug)]
struct Config {
    library: PathBuf,
    case_dir: Option<PathBuf>,
    out_dir: PathBuf,
    modes: Vec<CaseMode>,
    case_offset_per_mode: usize,
    max_cases_per_mode: usize,
    sample_runs: usize,
    bench_runs: usize,
    select_count: usize,
    shuffle_seed: u64,
    thread: u32,
    select_only: bool,
    quiet: bool,
}

#[derive(Debug)]
struct SampledCase {
    case: GeneratedCase,
    sample: BenchRun,
}

#[derive(Clone, Debug, Serialize)]
struct BenchRun {
    runs: usize,
    wins: usize,
    elapsed_ms: f64,
    us_per_battle: f64,
    battles_per_s: f64,
    init_us_per_battle: f64,
    fight_us_per_battle: f64,
}

#[derive(Clone, Debug, Serialize)]
struct CaseReport {
    rank: usize,
    id: String,
    mode: String,
    player_count: usize,
    input_hash: String,
    input_path: String,
    players: Vec<String>,
    sample: Option<BenchRun>,
    benchmark: Option<BenchRun>,
}

#[derive(Clone, Debug, Serialize)]
struct SummaryReport {
    group: String,
    cases: usize,
    runs: usize,
    wins: usize,
    elapsed_ms: f64,
    us_per_battle: f64,
    battles_per_s: f64,
    init_us_per_battle: f64,
    fight_us_per_battle: f64,
}

#[derive(Debug, Serialize)]
struct JsonReport {
    generated_at_unix: u64,
    version: String,
    source: String,
    library: String,
    case_dir: Option<String>,
    modes: Vec<String>,
    case_offset_per_mode: usize,
    max_cases_per_mode: usize,
    sample_runs: usize,
    bench_runs: usize,
    select_count: usize,
    shuffle_seed: u64,
    thread: u32,
    selected: Vec<CaseReport>,
    summaries: Vec<SummaryReport>,
}

fn main() {
    if let Err(err) = try_main() {
        eprintln!("错误: {err}");
        std::process::exit(1);
    }
}

fn try_main() -> Result<(), String> {
    let config = parse_args()?;
    fs::create_dir_all(&config.out_dir).map_err(|e| format!("创建输出目录失败: {e}"))?;

    if let Some(case_dir) = &config.case_dir {
        let fixed_cases = load_fixed_cases(case_dir)?;
        if fixed_cases.is_empty() {
            return Err(format!("固定 case 目录为空: {}", case_dir.display()));
        }
        if !config.quiet {
            println!(
                "固定 case 模式: 从 {} 读取 {} 个 case，跳过候选生成和采样",
                case_dir.display(),
                fixed_cases.len()
            );
        }
        let reports = run_fixed_cases(fixed_cases, &config)?;
        write_reports(&config, &reports)?;
        if !config.quiet {
            println!("报告已写入 {}", config.out_dir.display());
        }
        return Ok(());
    }

    let mut names = load_library(&config.library)?;
    if names.len() < 2 {
        return Err(format!("号库有效玩家数不足: {}，至少需要 2 个名字", names.len()));
    }
    deterministic_shuffle(&mut names, config.shuffle_seed);

    let candidates = generate_unique_cases(&names, &config);
    if candidates.is_empty() {
        return Err("没有生成任何可测试 case".to_string());
    }

    if !config.quiet {
        println!(
            "阶段 1/3: 已生成 {} 个唯一候选，开始用 {} 场/候选做复杂度采样",
            candidates.len(),
            config.sample_runs
        );
    }

    let sampled = sample_cases(candidates, &config)?;
    let selected = select_perf_ladder(sampled, config.select_count);
    if selected.is_empty() {
        return Err("没有可用采样结果".to_string());
    }

    if !config.quiet {
        println!("阶段 2/3: 已选出 {} 个阶梯 case", selected.len());
    }

    let reports = run_selected_cases(selected, &config)?;
    write_reports(&config, &reports)?;

    if !config.quiet {
        println!("阶段 3/3: 报告已写入 {}", config.out_dir.display());
    }
    Ok(())
}

fn generate_unique_cases(names: &[String], config: &Config) -> Vec<GeneratedCase> {
    let mut seen = HashSet::new();
    let mut cases = Vec::new();
    for mode in &config.modes {
        for case in generate_cases_for_mode(names, *mode, config.case_offset_per_mode, config.max_cases_per_mode) {
            if seen.insert(case.input_hash) {
                cases.push(case);
            }
        }
    }
    cases
}

fn sample_cases(cases: Vec<GeneratedCase>, config: &Config) -> Result<Vec<SampledCase>, String> {
    let mut sampled = Vec::with_capacity(cases.len());
    for (idx, case) in cases.into_iter().enumerate() {
        match bench_generated_case(&case, config.sample_runs, 1) {
            Ok(sample) => sampled.push(SampledCase { case, sample }),
            Err(err) => {
                if !config.quiet {
                    eprintln!("跳过采样失败 case #{idx}: {err}");
                }
            }
        }
        if !config.quiet && (idx + 1) % 500 == 0 {
            println!("  采样进度: {}", idx + 1);
        }
    }
    Ok(sampled)
}

fn select_perf_ladder(mut sampled: Vec<SampledCase>, count: usize) -> Vec<SampledCase> {
    sampled.sort_by(|a, b| {
        a.sample
            .us_per_battle
            .partial_cmp(&b.sample.us_per_battle)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.case.mode.label().cmp(&b.case.mode.label()))
            .then_with(|| a.case.input_hash.cmp(&b.case.input_hash))
    });

    if sampled.len() <= count {
        return sampled;
    }
    if count == 1 {
        return vec![sampled.remove(0)];
    }

    let last = sampled.len() - 1;
    let denom = count - 1;
    let mut selected_indices = HashSet::new();
    for rank in 0..count {
        let mut idx = (rank * last + denom / 2) / denom;
        while selected_indices.contains(&idx) && idx < last {
            idx += 1;
        }
        while selected_indices.contains(&idx) && idx > 0 {
            idx -= 1;
        }
        selected_indices.insert(idx);
    }

    let mut selected = sampled
        .into_iter()
        .enumerate()
        .filter_map(|(idx, item)| selected_indices.contains(&idx).then_some(item))
        .collect::<Vec<_>>();
    selected.sort_by(|a, b| {
        a.sample
            .us_per_battle
            .partial_cmp(&b.sample.us_per_battle)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    selected
}

fn run_selected_cases(selected: Vec<SampledCase>, config: &Config) -> Result<Vec<CaseReport>, String> {
    let case_dir = config.out_dir.join("cases");
    fs::create_dir_all(&case_dir).map_err(|e| format!("创建 case 目录失败: {e}"))?;

    let mut reports = Vec::with_capacity(selected.len());
    for (idx, item) in selected.into_iter().enumerate() {
        let rank = idx + 1;
        let id = case_id(item.case.mode, item.case.input_hash);
        let input_path = case_dir.join(format!("{rank:02}_{id}.txt"));
        fs::write(&input_path, &item.case.input).map_err(|e| format!("写入 {} 失败: {e}", input_path.display()))?;

        if !config.quiet {
            println!(
                "[{rank:02}/{}] {} sample={:.1}us/场",
                config.select_count.min(reports.capacity()),
                id,
                item.sample.us_per_battle
            );
        }

        let benchmark = if config.select_only {
            None
        } else {
            Some(bench_generated_case(&item.case, config.bench_runs, config.thread)?)
        };

        reports.push(CaseReport {
            rank,
            id,
            mode: item.case.mode.label(),
            player_count: item.case.mode.total_players(),
            input_hash: format!("{:016x}", item.case.input_hash),
            input_path: input_path.display().to_string(),
            players: item.case.players,
            sample: Some(item.sample),
            benchmark,
        });
    }

    Ok(reports)
}

fn load_fixed_cases(case_dir: &Path) -> Result<Vec<(PathBuf, GeneratedCase)>, String> {
    if !case_dir.is_dir() {
        return Err(format!("固定 case 目录不存在: {}", case_dir.display()));
    }
    let mut files = fs::read_dir(case_dir)
        .map_err(|e| format!("读取固定 case 目录失败: {e}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
        .collect::<Vec<_>>();
    files.sort();

    let mut cases = Vec::with_capacity(files.len());
    for path in files {
        let input = fs::read_to_string(&path)
            .map_err(|e| format!("读取 {} 失败: {e}", path.display()))?
            .replace("\r\n", "\n")
            .replace('\r', "\n");
        let mode = parse_case_mode_from_filename(&path)?;
        let players = input
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let input_hash = stable_hash(&input);
        cases.push((
            path,
            GeneratedCase {
                mode,
                players,
                input,
                input_hash,
            },
        ));
    }
    Ok(cases)
}

fn parse_case_mode_from_filename(path: &Path) -> Result<CaseMode, String> {
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("无法解析 case 文件名: {}", path.display()))?;
    let without_rank = stem
        .split_once('_')
        .and_then(|(prefix, rest)| prefix.chars().all(|ch| ch.is_ascii_digit()).then_some(rest))
        .unwrap_or(stem);
    let (mode, _) = without_rank
        .rsplit_once('-')
        .ok_or_else(|| format!("case 文件名缺少 mode-hash: {}", path.display()))?;
    parse_case_mode_label(mode).ok_or_else(|| format!("未知 case mode `{mode}`: {}", path.display()))
}

fn parse_case_mode_label(label: &str) -> Option<CaseMode> {
    match label {
        "1v1" => Some(CaseMode::OneVsOne),
        "2v2" => Some(CaseMode::TwoVsTwo),
        "3v3v3" => Some(CaseMode::ThreeVsThreeVsThree),
        _ => label
            .strip_prefix("ffa_")
            .and_then(|size| size.parse::<usize>().ok())
            .map(CaseMode::FreeForAll),
    }
}

fn run_fixed_cases(cases: Vec<(PathBuf, GeneratedCase)>, config: &Config) -> Result<Vec<CaseReport>, String> {
    let mut reports = Vec::with_capacity(cases.len());
    for (idx, (path, case)) in cases.into_iter().enumerate() {
        let rank = idx + 1;
        let id = case_id(case.mode, case.input_hash);
        if !config.quiet {
            println!("[{rank:02}/{}] {} {}", reports.capacity(), case.mode.label(), path.display());
        }
        let benchmark = if config.select_only {
            None
        } else {
            Some(bench_generated_case(&case, config.bench_runs, config.thread)?)
        };
        reports.push(CaseReport {
            rank,
            id,
            mode: case.mode.label(),
            player_count: case.mode.total_players(),
            input_hash: format!("{:016x}", case.input_hash),
            input_path: path.display().to_string(),
            players: case.players,
            sample: None,
            benchmark,
        });
    }
    Ok(reports)
}

fn bench_generated_case(case: &GeneratedCase, runs: usize, thread: u32) -> Result<BenchRun, String> {
    let (groups, _) = Runner::split_namerena_into_groups(case.input.clone());
    // 这里遍历的是大量自动生成 case。每个 case 通常只 benchmark 一次，
    // 没有把模板留在全局缓存里的收益；改走 uncached 后，离线批量分析时常驻内存更稳定。
    let prepared = Runner::prepare_groups_with_eval_rq_uncached(&groups, WIN_RATE_EVAL_RQ)
        .map_err(|e| format!("prepare 失败({}): {e}", case_id(case.mode, case.input_hash)))?;

    let started = Instant::now();
    let summary = prepared_win_rate(&prepared, runs, WIN_RATE_EVAL_RQ, thread)
        .map_err(|e| format!("benchmark 失败({}): {e}", case_id(case.mode, case.input_hash)))?;
    Ok(bench_run_from_summary(summary, started.elapsed()))
}

fn bench_run_from_summary(summary: WinRateSummary, elapsed: Duration) -> BenchRun {
    let total = summary.total.max(1) as f64;
    let elapsed_s = elapsed.as_secs_f64();
    BenchRun {
        runs: summary.total,
        wins: summary.wins,
        elapsed_ms: elapsed_s * 1000.0,
        us_per_battle: elapsed.as_micros() as f64 / total,
        battles_per_s: if elapsed_s > 0.0 { total / elapsed_s } else { 0.0 },
        init_us_per_battle: summary.timing.init_nanos as f64 / 1e3 / total,
        fight_us_per_battle: summary.timing.fight_nanos as f64 / 1e3 / total,
    }
}

fn write_reports(config: &Config, reports: &[CaseReport]) -> Result<(), String> {
    let summaries = build_summary_reports(reports);
    let json = JsonReport {
        generated_at_unix: unix_now(),
        version: tswn_core::version().to_string(),
        source: if config.case_dir.is_some() {
            "fixed-case-dir".to_string()
        } else {
            "generated-ladder".to_string()
        },
        library: config.library.display().to_string(),
        case_dir: config.case_dir.as_ref().map(|path| path.display().to_string()),
        modes: config.modes.iter().map(|mode| mode.label()).collect(),
        case_offset_per_mode: config.case_offset_per_mode,
        max_cases_per_mode: config.max_cases_per_mode,
        sample_runs: config.sample_runs,
        bench_runs: config.bench_runs,
        select_count: config.select_count,
        shuffle_seed: config.shuffle_seed,
        thread: config.thread,
        selected: reports.to_vec(),
        summaries,
    };

    let raw_json = serde_json::to_string_pretty(&json).map_err(|e| format!("序列化 JSON 失败: {e}"))?;
    fs::write(config.out_dir.join("perf_cases.json"), raw_json).map_err(|e| format!("写入 perf_cases.json 失败: {e}"))?;

    let markdown = build_markdown_report(config, reports);
    fs::write(config.out_dir.join("perf_cases.md"), markdown).map_err(|e| format!("写入 perf_cases.md 失败: {e}"))?;

    let version = sanitize_filename_part(tswn_core::version());
    let raw_json = serde_json::to_string_pretty(&json).map_err(|e| format!("序列化 JSON 失败: {e}"))?;
    fs::write(config.out_dir.join(format!("perf_cases_{version}.json")), raw_json)
        .map_err(|e| format!("写入 perf_cases_{version}.json 失败: {e}"))?;
    let markdown = build_markdown_report(config, reports);
    fs::write(config.out_dir.join(format!("perf_cases_{version}.md")), markdown)
        .map_err(|e| format!("写入 perf_cases_{version}.md 失败: {e}"))?;
    Ok(())
}

fn build_summary_reports(reports: &[CaseReport]) -> Vec<SummaryReport> {
    let groups: [(&str, fn(&CaseReport) -> bool); 5] = [
        ("overall", |_| true),
        ("core_1v1_2v2", |case| matches!(case.mode.as_str(), "1v1" | "2v2")),
        ("one_v_one", |case| case.mode == "1v1"),
        ("two_v_two", |case| case.mode == "2v2"),
        ("stress_multi", |case| matches!(case.mode.as_str(), "ffa_6" | "ffa_8" | "3v3v3")),
    ];

    groups
        .into_iter()
        .filter_map(|(group, predicate)| summarize_group(group, reports.iter().filter(|case| predicate(case))))
        .collect()
}

fn summarize_group<'a>(group: &str, cases: impl Iterator<Item = &'a CaseReport>) -> Option<SummaryReport> {
    let mut case_count = 0usize;
    let mut runs = 0usize;
    let mut wins = 0usize;
    let mut elapsed_ms = 0.0f64;
    let mut init_us_total = 0.0f64;
    let mut fight_us_total = 0.0f64;

    for case in cases {
        let Some(bench) = &case.benchmark else {
            continue;
        };
        case_count += 1;
        runs += bench.runs;
        wins += bench.wins;
        elapsed_ms += bench.elapsed_ms;
        init_us_total += bench.init_us_per_battle * bench.runs as f64;
        fight_us_total += bench.fight_us_per_battle * bench.runs as f64;
    }

    if case_count == 0 || runs == 0 {
        return None;
    }

    let elapsed_s = elapsed_ms / 1000.0;
    let runs_f = runs as f64;
    Some(SummaryReport {
        group: group.to_string(),
        cases: case_count,
        runs,
        wins,
        elapsed_ms,
        us_per_battle: elapsed_ms * 1000.0 / runs_f,
        battles_per_s: if elapsed_s > 0.0 { runs_f / elapsed_s } else { 0.0 },
        init_us_per_battle: init_us_total / runs_f,
        fight_us_per_battle: fight_us_total / runs_f,
    })
}

fn sanitize_filename_part(raw: &str) -> String {
    raw.chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => ch,
            _ => '_',
        })
        .collect()
}

fn build_markdown_report(config: &Config, reports: &[CaseReport]) -> String {
    let mut out = String::new();
    let _ = writeln!(&mut out, "# tswn_core Performance Case Ladder");
    let _ = writeln!(&mut out);
    let _ = writeln!(&mut out, "- generated_at_unix: {}", unix_now());
    let _ = writeln!(&mut out, "- version: `{}`", tswn_core::version());
    if let Some(case_dir) = &config.case_dir {
        let _ = writeln!(&mut out, "- source: `fixed-case-dir`");
        let _ = writeln!(&mut out, "- case_dir: `{}`", case_dir.display());
    } else {
        let _ = writeln!(&mut out, "- source: `generated-ladder`");
        let _ = writeln!(&mut out, "- library: `{}`", config.library.display());
        let _ = writeln!(
            &mut out,
            "- modes: `{}`",
            config.modes.iter().map(|mode| mode.label()).collect::<Vec<_>>().join(",")
        );
        let _ = writeln!(&mut out, "- candidate_per_mode: `{}`", config.max_cases_per_mode);
        let _ = writeln!(&mut out, "- sample_runs: `{}`", config.sample_runs);
    }
    let _ = writeln!(&mut out, "- bench_runs: `{}`", config.bench_runs);
    let _ = writeln!(&mut out, "- thread: `{}`", config.thread);
    let _ = writeln!(&mut out);

    let _ = writeln!(&mut out, "## Selected Cases");
    let _ = writeln!(&mut out);
    let _ = writeln!(&mut out, "| # | id | mode | players | sample us/场 | input |");
    let _ = writeln!(&mut out, "| -: | -- | -- | -: | -: | -- |");
    for item in reports {
        let sample = item
            .sample
            .as_ref()
            .map(|sample| format!("{:.1}", sample.us_per_battle))
            .unwrap_or_else(|| "-".to_string());
        let _ = writeln!(
            &mut out,
            "| {} | `{}` | `{}` | {} | `{}` | `{}` |",
            item.rank, item.id, item.mode, item.player_count, sample, item.input_path
        );
    }

    if reports.iter().any(|item| item.benchmark.is_some()) {
        let summaries = build_summary_reports(reports);
        if !summaries.is_empty() {
            let _ = writeln!(&mut out);
            let _ = writeln!(&mut out, "## Summary Groups");
            let _ = writeln!(&mut out);
            let _ = writeln!(
                &mut out,
                "| group | cases | runs | win rate | elapsed | us/场 | 场/s | init us/场 | fight us/场 |"
            );
            let _ = writeln!(&mut out, "| -- | -: | -: | -: | -: | -: | -: | -: | -: |");
            for summary in &summaries {
                let win_rate = summary.wins as f64 * 100.0 / summary.runs.max(1) as f64;
                let _ = writeln!(
                    &mut out,
                    "| `{}` | {} | {} | `{:.2}%` | `{:.3}s` | `{:.1}` | `{:.0}` | `{:.1}` | `{:.1}` |",
                    summary.group,
                    summary.cases,
                    summary.runs,
                    win_rate,
                    summary.elapsed_ms / 1000.0,
                    summary.us_per_battle,
                    summary.battles_per_s,
                    summary.init_us_per_battle,
                    summary.fight_us_per_battle
                );
            }
        }

        let _ = writeln!(&mut out);
        let _ = writeln!(&mut out, "## Benchmark Results");
        let _ = writeln!(&mut out);
        let _ = writeln!(
            &mut out,
            "| # | id | runs | win rate | elapsed | us/场 | 场/s | init us/场 | fight us/场 |"
        );
        let _ = writeln!(&mut out, "| -: | -- | -: | -: | -: | -: | -: | -: | -: |");
        for item in reports {
            let Some(bench) = &item.benchmark else {
                continue;
            };
            let win_rate = bench.wins as f64 * 100.0 / bench.runs.max(1) as f64;
            let _ = writeln!(
                &mut out,
                "| {} | `{}` | {} | `{:.2}%` | `{:.3}s` | `{:.1}` | `{:.0}` | `{:.1}` | `{:.1}` |",
                item.rank,
                item.id,
                bench.runs,
                win_rate,
                bench.elapsed_ms / 1000.0,
                bench.us_per_battle,
                bench.battles_per_s,
                bench.init_us_per_battle,
                bench.fight_us_per_battle
            );
        }
    }

    out
}

fn parse_args() -> Result<Config, String> {
    let args = std::env::args().skip(1).collect::<Vec<String>>();
    let mut config = Config {
        library: PathBuf::from(DEFAULT_LIBRARY),
        case_dir: None,
        out_dir: PathBuf::from(DEFAULT_OUT_DIR),
        modes: default_modes(),
        case_offset_per_mode: DEFAULT_CASE_OFFSET_PER_MODE,
        max_cases_per_mode: DEFAULT_MAX_CASES_PER_MODE,
        sample_runs: DEFAULT_SAMPLE_RUNS,
        bench_runs: DEFAULT_BENCH_RUNS,
        select_count: DEFAULT_SELECT_COUNT,
        shuffle_seed: DEFAULT_SHUFFLE_SEED,
        thread: 1,
        select_only: false,
        quiet: false,
    };

    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            "--library" => {
                idx += 1;
                config.library = PathBuf::from(require_arg(&args, idx, "--library")?);
            }
            "--case-dir" => {
                idx += 1;
                config.case_dir = Some(PathBuf::from(require_arg(&args, idx, "--case-dir")?));
            }
            "--out-dir" => {
                idx += 1;
                config.out_dir = PathBuf::from(require_arg(&args, idx, "--out-dir")?);
            }
            "--modes" => {
                idx += 1;
                let (modes, include_ffa) = parse_mode_names(require_arg(&args, idx, "--modes")?)?;
                config.modes = if include_ffa {
                    modes_with_ffa_sizes(modes, &[4, 6, 8])
                } else {
                    modes
                };
            }
            "--ffa-sizes" => {
                idx += 1;
                let sizes = parse_usize_csv(require_arg(&args, idx, "--ffa-sizes")?, "--ffa-sizes")?;
                let base = config
                    .modes
                    .iter()
                    .copied()
                    .filter(|mode| !matches!(mode, CaseMode::FreeForAll(_)))
                    .collect::<Vec<_>>();
                config.modes = modes_with_ffa_sizes(base, &sizes);
            }
            "--case-offset-per-mode" => {
                idx += 1;
                config.case_offset_per_mode =
                    parse_usize(require_arg(&args, idx, "--case-offset-per-mode")?, "--case-offset-per-mode")?;
            }
            "--max-cases-per-mode" => {
                idx += 1;
                config.max_cases_per_mode =
                    parse_positive_usize(require_arg(&args, idx, "--max-cases-per-mode")?, "--max-cases-per-mode")?;
            }
            "--sample-runs" => {
                idx += 1;
                config.sample_runs = parse_positive_usize(require_arg(&args, idx, "--sample-runs")?, "--sample-runs")?;
            }
            "--bench-runs" => {
                idx += 1;
                config.bench_runs = parse_positive_usize(require_arg(&args, idx, "--bench-runs")?, "--bench-runs")?;
            }
            "--select-count" => {
                idx += 1;
                config.select_count = parse_positive_usize(require_arg(&args, idx, "--select-count")?, "--select-count")?;
            }
            "--shuffle-seed" => {
                idx += 1;
                config.shuffle_seed = require_arg(&args, idx, "--shuffle-seed")?
                    .parse::<u64>()
                    .map_err(|e| format!("解析 --shuffle-seed 失败: {e}"))?;
            }
            "--thread" => {
                idx += 1;
                config.thread = require_arg(&args, idx, "--thread")?
                    .parse::<u32>()
                    .map_err(|e| format!("解析 --thread 失败: {e}"))?;
            }
            "--select-only" => config.select_only = true,
            "-q" | "--quiet" => config.quiet = true,
            other => return Err(format!("未知参数: {other}")),
        }
        idx += 1;
    }

    config.library = resolve_runtime_path(&config.library);
    config.case_dir = config.case_dir.map(|path| resolve_runtime_path(&path));
    config.out_dir = resolve_runtime_path(&config.out_dir);
    if config.case_dir.is_none() && !config.library.is_file() {
        return Err(format!("号库文件不存在: {}", config.library.display()));
    }
    Ok(config)
}

fn default_modes() -> Vec<CaseMode> {
    vec![
        CaseMode::OneVsOne,
        CaseMode::TwoVsTwo,
        CaseMode::ThreeVsThreeVsThree,
        CaseMode::FreeForAll(4),
        CaseMode::FreeForAll(6),
        CaseMode::FreeForAll(8),
    ]
}

fn modes_with_ffa_sizes(mut modes: Vec<CaseMode>, sizes: &[usize]) -> Vec<CaseMode> {
    modes.extend(sizes.iter().copied().map(CaseMode::FreeForAll));
    modes
}

fn parse_mode_names(raw: &str) -> Result<(Vec<CaseMode>, bool), String> {
    let mut modes = Vec::new();
    let mut include_ffa = false;
    for token in raw.split(',').map(str::trim).filter(|token| !token.is_empty()) {
        match token {
            "1v1" => modes.push(CaseMode::OneVsOne),
            "2v2" => modes.push(CaseMode::TwoVsTwo),
            "3v3v3" => modes.push(CaseMode::ThreeVsThreeVsThree),
            "ffa" => include_ffa = true,
            other => return Err(format!("未知模式: {other}")),
        }
    }
    Ok((modes, include_ffa))
}

fn parse_usize_csv(raw: &str, flag: &str) -> Result<Vec<usize>, String> {
    let mut values = Vec::new();
    for token in raw.split(',').map(str::trim).filter(|token| !token.is_empty()) {
        let value = parse_positive_usize(token, flag)?;
        if value < 2 {
            return Err(format!("{flag} 中的值必须 >= 2: {value}"));
        }
        values.push(value);
    }
    if values.is_empty() {
        return Err(format!("{flag} 不能为空"));
    }
    Ok(values)
}

fn parse_usize(raw: &str, flag: &str) -> Result<usize, String> {
    raw.parse::<usize>().map_err(|e| format!("解析 {flag} 失败: {e}"))
}

fn parse_positive_usize(raw: &str, flag: &str) -> Result<usize, String> {
    let value = parse_usize(raw, flag)?;
    if value == 0 {
        return Err(format!("{flag} 必须大于 0"));
    }
    Ok(value)
}

fn require_arg<'a>(args: &'a [String], idx: usize, flag: &str) -> Result<&'a str, String> {
    args.get(idx).map(String::as_str).ok_or_else(|| format!("{flag} 缺少参数"))
}

fn resolve_runtime_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn print_usage() {
    println!(
        r#"用法:
  track_perf_cases [选项]

选项:
  --library <path>              号库文件，默认 tests/sqp6000.txt
  --case-dir <path>             固定 case 输入目录；指定后跳过号库生成和采样
  --out-dir <path>              输出目录，默认 target/perf_cases
                                同时输出 perf_cases.md/json 和 perf_cases_<version>.md/json
  --modes <csv>                 默认 1v1,2v2,3v3v3,ffa
  --ffa-sizes <csv>             默认 4,6,8
  --case-offset-per-mode <N>    每种模式跳过前 N 个 case，默认 0
  --max-cases-per-mode <N>      每种模式最多生成多少候选，默认 4000
  --sample-runs <N>             候选复杂度采样轮数，默认 64
  --bench-runs <N>              选中 case 正式 benchmark 轮数，默认 500000
  --select-count <N>            从简单到困难选多少个 case，默认 20
  --shuffle-seed <N>            固定号库采样顺序
  --thread <N>                  正式 benchmark 线程参数：1=单线程，0=默认并行，N=指定线程；默认 1
  --select-only                 只选 case 和写输入，不跑正式 benchmark
  -q, --quiet                   安静模式
"#
    );
}

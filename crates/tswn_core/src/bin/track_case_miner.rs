//! JS/Rust 对局差异挖掘结果追踪工具。
//!
//! 负责驱动 `tswn_case_miner`，读取其生成的失败案例摘要，并像 `track_test`
//! 一样维护当前记录与 checkpoint 对比。用于把大量随机/采样输入中的真实行为分歧沉淀为可复查记录。

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEFAULT_MODES: &str = "1v1,2v2,3v3v3,ffa";
const DEFAULT_FFA_SIZES: &str = "4,6,8";
const DEFAULT_CASE_OFFSET_PER_MODE: usize = 0;
const DEFAULT_MAX_CASES_PER_MODE: usize = 64;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct MinerRecords {
    time: String,
    config: MinerConfigRecord,
    summary: MinerSummaryRecord,
    failed_cases: BTreeMap<String, FailedCaseRecord>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct MinerConfigRecord {
    library: Option<String>,
    md5_tool: Option<String>,
    out_dir: String,
    shared_cache_dir: String,
    bun_cache_dir: String,
    modes: String,
    ffa_sizes: String,
    case_offset_per_mode: usize,
    max_cases_per_mode: usize,
    keep_going: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct MinerSummaryRecord {
    total_generated: usize,
    unique_inputs: usize,
    executed: usize,
    ts_cache_hits: usize,
    ts_cache_misses: usize,
    bun_invocations: usize,
    ts_failures: usize,
    rust_failures: usize,
    diff_failures: usize,
    deduped_diff_failures: usize,
    per_mode_generated: BTreeMap<String, usize>,
    per_mode_failures: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct FailedCaseRecord {
    mode: String,
    idx: i64,
    diff_signature: String,
    input: String,
    ts: String,
    rust: String,
    diff: String,
    meta: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Checkpoint {
    name: String,
    time: String,
    records: MinerRecords,
}

#[derive(Debug)]
struct Config {
    library: PathBuf,
    md5_tool: Option<PathBuf>,
    out_dir: PathBuf,
    shared_cache_dir: PathBuf,
    modes: String,
    ffa_sizes: String,
    case_offset_per_mode: usize,
    max_cases_per_mode: usize,
    keep_going: bool,
    show: bool,
    reset: bool,
    quiet: bool,
    command: Option<TrackCommand>,
}

#[derive(Debug)]
enum TrackCommand {
    Save { name: Option<String> },
    List,
    Diff { name: Option<String> },
    Delete { name: String },
}

#[derive(Debug)]
struct Change {
    case_id: String,
    kind: &'static str,
    mode: String,
    idx: i64,
    prev_idx: i64,
}

fn main() {
    if let Err(err) = try_main() {
        eprintln!("错误: {err}");
        std::process::exit(1);
    }
}

fn try_main() -> Result<(), String> {
    let mut config = parse_args()?;
    let paths = Paths::new();

    if let Some(command) = &config.command {
        run_command(command, &paths)?;
        return Ok(());
    }

    if config.reset {
        if paths.record_file.exists() {
            fs::remove_file(&paths.record_file).map_err(|e| format!("删除历史记录失败: {e}"))?;
        }
        save_records(&paths.record_file, &empty_records())?;
        println!("已重置 miner 历史记录");
        return Ok(());
    }

    if config.show {
        print_current_status(&load_records(&paths.record_file));
        return Ok(());
    }

    config.library = resolve_runtime_path(&config.library);
    config.out_dir = resolve_runtime_path(&config.out_dir);
    config.shared_cache_dir = resolve_runtime_path(&config.shared_cache_dir);
    config.md5_tool = config.md5_tool.map(|path| resolve_runtime_path(&path));

    if !config.library.is_file() {
        return Err(format!("号库文件不存在: {}", config.library.display()));
    }
    let md5_tool = config
        .md5_tool
        .clone()
        .or_else(detect_default_md5_tool)
        .ok_or_else(|| "运行 miner 时必须提供 --md5-tool，或保证默认 fast-namerena 路径可自动推导".to_string())?;
    if !md5_tool.is_file() {
        return Err(format!("md5 工具文件不存在: {}", md5_tool.display()));
    }
    config.md5_tool = Some(md5_tool);

    if !config.quiet {
        println!("========================================");
        println!("  tswn_case_miner 回归追踪工具");
        println!("========================================");
        println!();
        println!("运行 miner: library={}", config.library.display());
        println!("md5 tool: {}", config.md5_tool.as_ref().unwrap().display());
        println!("shared cache: {}", config.shared_cache_dir.display());
        println!("bun cache: {}", config.shared_cache_dir.join("bun").display());
        println!("case offset per mode: {}", config.case_offset_per_mode);
        println!("max cases per mode: {}", config.max_cases_per_mode);
        println!();
        println!("准备阶段: 启动 cargo run（这里可能会出现 Rust 编译输出）");
    } else {
        println!(
            "[track_case_miner] 运行 miner: {} (offset={}, max={})",
            config.library.display(),
            config.case_offset_per_mode,
            config.max_cases_per_mode
        );
    }

    let result = run_miner(&config)?;
    if !result.success {
        println!("miner 运行失败");
        if !result.stdout.trim().is_empty() {
            println!("{}", result.stdout.trim_end());
        }
        if !result.stderr.trim().is_empty() {
            eprintln!("{}", result.stderr.trim_end());
        }
        std::process::exit(result.code.unwrap_or(1));
    }

    let summary_path = config.out_dir.join("summary.json");
    let summary = load_summary(&summary_path)?;
    let current_records = summarize_run(&summary, &config)?;
    let previous_records = load_records(&paths.record_file);
    let scope_mismatches = find_scope_mismatches(&current_records, &previous_records);
    let changes = if scope_mismatches.is_empty() {
        compare_records(&current_records, &previous_records)
    } else {
        Vec::new()
    };

    if !config.quiet {
        println!();
        println!("收尾阶段: 读取 summary.json 并比较结果");
        println!("--- vs 上次运行 ---");
        if scope_mismatches.is_empty() {
            print_changes(&changes);
        } else {
            print_scope_mismatches(&scope_mismatches);
        }
        println!();
        println!("========================================");
        println!("  汇总");
        println!("========================================");
        println!("TS cache hit: {}", current_records.summary.ts_cache_hits);
        println!("TS cache miss: {}", current_records.summary.ts_cache_misses);
        println!("bun invocations: {}", current_records.summary.bun_invocations);
        println!("failed case: {}", current_records.summary.diff_failures);
        println!("deduped failed case: {}", current_records.summary.deduped_diff_failures);
        println!("TS failures: {}", current_records.summary.ts_failures);
        println!("Rust failures: {}", current_records.summary.rust_failures);
    } else {
        println!("--- vs 上次运行 ---");
    }

    if scope_mismatches.is_empty() {
        print_conclusion(&changes);
    } else {
        println!("结论: 比较范围已变化，本次结果不与上次直接判优劣");
    }
    print_checkpoint_comparison(&paths, &current_records, config.quiet)?;

    save_records(&paths.record_file, &current_records)?;
    write_log(
        &paths.log_file,
        &format!(
            "improved:{}, regressed:{}, fixed:{}, new_failed:{}, diff_failures:{}",
            changes.iter().filter(|c| c.kind == "IMPROVED").count(),
            changes.iter().filter(|c| c.kind == "REGRESSED").count(),
            changes.iter().filter(|c| c.kind == "FIXED_CASE").count(),
            changes.iter().filter(|c| c.kind == "NEW_FAILED_CASE").count(),
            current_records.summary.diff_failures
        ),
    )?;

    Ok(())
}

struct Paths {
    record_file: PathBuf,
    log_file: PathBuf,
    checkpoint_dir: PathBuf,
}

impl Paths {
    fn new() -> Self {
        Self {
            record_file: PathBuf::from("target").join("case_miner_regression.json"),
            log_file: PathBuf::from("target").join("case_miner_regression.log"),
            checkpoint_dir: PathBuf::from("target").join("case_miner_checkpoints"),
        }
    }
}

struct RunResult {
    success: bool,
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn parse_args() -> Result<Config, String> {
    let args = env::args().skip(1).collect::<Vec<String>>();
    let mut config = Config {
        library: PathBuf::from("tests").join("sqp6000.txt"),
        md5_tool: None,
        out_dir: PathBuf::from("target").join("ts_diff_cases"),
        shared_cache_dir: PathBuf::from("tests").join("tswn_case_miner_cache"),
        modes: DEFAULT_MODES.to_string(),
        ffa_sizes: DEFAULT_FFA_SIZES.to_string(),
        case_offset_per_mode: DEFAULT_CASE_OFFSET_PER_MODE,
        max_cases_per_mode: DEFAULT_MAX_CASES_PER_MODE,
        keep_going: false,
        show: false,
        reset: false,
        quiet: false,
        command: None,
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
            "--md5-tool" => {
                idx += 1;
                config.md5_tool = Some(PathBuf::from(require_arg(&args, idx, "--md5-tool")?));
            }
            "--out-dir" => {
                idx += 1;
                config.out_dir = PathBuf::from(require_arg(&args, idx, "--out-dir")?);
            }
            "--shared-cache-dir" => {
                idx += 1;
                config.shared_cache_dir = PathBuf::from(require_arg(&args, idx, "--shared-cache-dir")?);
            }
            "--modes" => {
                idx += 1;
                config.modes = require_arg(&args, idx, "--modes")?.to_string();
            }
            "--ffa-sizes" => {
                idx += 1;
                config.ffa_sizes = require_arg(&args, idx, "--ffa-sizes")?.to_string();
            }
            "--case-offset-per-mode" => {
                idx += 1;
                config.case_offset_per_mode = require_arg(&args, idx, "--case-offset-per-mode")?
                    .parse()
                    .map_err(|e| format!("解析 --case-offset-per-mode 失败: {e}"))?;
            }
            "--max-cases-per-mode" => {
                idx += 1;
                config.max_cases_per_mode = require_arg(&args, idx, "--max-cases-per-mode")?
                    .parse()
                    .map_err(|e| format!("解析 --max-cases-per-mode 失败: {e}"))?;
                if config.max_cases_per_mode == 0 {
                    return Err("--max-cases-per-mode 必须 > 0".to_string());
                }
            }
            "--keep-going" => config.keep_going = true,
            "-s" | "--show" => config.show = true,
            "-r" | "--reset" => config.reset = true,
            "-q" | "--quiet" => config.quiet = true,
            "save" => {
                config.command = Some(TrackCommand::Save {
                    name: args.get(idx + 1).cloned(),
                });
                break;
            }
            "list" => {
                config.command = Some(TrackCommand::List);
                break;
            }
            "diff" => {
                config.command = Some(TrackCommand::Diff {
                    name: args.get(idx + 1).cloned(),
                });
                break;
            }
            "delete" => {
                config.command = Some(TrackCommand::Delete {
                    name: args.get(idx + 1).cloned().ok_or_else(|| "delete 缺少存档点名称".to_string())?,
                });
                break;
            }
            other => return Err(format!("未知参数: {other}")),
        }
        idx += 1;
    }
    Ok(config)
}

fn print_usage() {
    println!(
        r#"用法:
  track_case_miner [选项]
  track_case_miner save [name]
  track_case_miner list
  track_case_miner diff [name]
  track_case_miner delete <name>

选项:
  --library <path>              号库文件，默认 tests/sqp6000.txt
  --md5-tool <path>             out_md5.ts 路径
  --out-dir <path>              miner 输出目录，默认 target/ts_diff_cases
  --shared-cache-dir <path>     共享缓存目录，默认 tests/tswn_case_miner_cache
  --modes <csv>                 默认 1v1,2v2,3v3v3,ffa
  --ffa-sizes <csv>             默认 4,6,8
  --case-offset-per-mode <N>    默认 0
  --max-cases-per-mode <N>      默认 64
  --keep-going                  单个 case 失败时继续
  -s, --show                    只显示当前失败状态，不运行 miner
  -r, --reset                   重置历史记录
  -q, --quiet                   安静模式
"#
    );
}

fn require_arg<'a>(args: &'a [String], idx: usize, flag: &str) -> Result<&'a str, String> {
    args.get(idx).map(String::as_str).ok_or_else(|| format!("{flag} 缺少参数"))
}

fn run_command(command: &TrackCommand, paths: &Paths) -> Result<(), String> {
    match command {
        TrackCommand::Save { name } => cmd_save(paths, name.clone()),
        TrackCommand::List => cmd_list(paths),
        TrackCommand::Diff { name } => cmd_diff(paths, name.as_deref()),
        TrackCommand::Delete { name } => cmd_delete(paths, name),
    }
}

fn run_miner(config: &Config) -> Result<RunResult, String> {
    let bun_cache_dir = config.shared_cache_dir.join("bun");
    let ts_cache_dir = config.shared_cache_dir.join("ts_trace");
    fs::create_dir_all(&bun_cache_dir).map_err(|e| format!("创建 bun 缓存目录失败: {e}"))?;
    fs::create_dir_all(&ts_cache_dir).map_err(|e| format!("创建 TS 缓存目录失败: {e}"))?;

    let mut cmd = Command::new("cargo");
    cmd.arg("run");
    if config.quiet {
        cmd.arg("--quiet");
    }
    cmd.args(["--release", "--features", "no_debug", "--bin", "tswn_case_miner", "--"]);
    cmd.arg("--library").arg(&config.library);
    cmd.arg("--md5-tool").arg(config.md5_tool.as_ref().unwrap());
    cmd.arg("--out-dir").arg(&config.out_dir);
    cmd.arg("--modes").arg(&config.modes);
    cmd.arg("--ffa-sizes").arg(&config.ffa_sizes);
    cmd.arg("--case-offset-per-mode").arg(config.case_offset_per_mode.to_string());
    cmd.arg("--max-cases-per-mode").arg(config.max_cases_per_mode.to_string());
    if config.keep_going {
        cmd.arg("--keep-going");
    }
    cmd.env("TSWN_CASE_MINER_TS_CACHE_DIR", &ts_cache_dir);
    cmd.env("TSWN_CASE_MINER_BUN_CACHE_DIR", &bun_cache_dir);

    if config.quiet {
        let output = cmd.output().map_err(|e| format!("运行 miner 失败: {e}"))?;
        Ok(RunResult {
            success: output.status.success(),
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    } else {
        let status = cmd.status().map_err(|e| format!("运行 miner 失败: {e}"))?;
        Ok(RunResult {
            success: status.success(),
            code: status.code(),
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

fn load_summary(path: &Path) -> Result<Value, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("找不到 summary.json: {} ({e})", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("解析 summary.json 失败: {e}"))
}

fn summarize_run(summary: &Value, config: &Config) -> Result<MinerRecords, String> {
    let mut failed_cases = BTreeMap::new();
    for case in summary.get("failed_cases").and_then(Value::as_array).into_iter().flatten() {
        let id = case.get("id").and_then(Value::as_str).unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        failed_cases.insert(
            id,
            FailedCaseRecord {
                mode: case.get("mode").and_then(Value::as_str).unwrap_or("?").to_string(),
                idx: case.get("first_mismatch_idx").and_then(Value::as_i64).unwrap_or(-1),
                diff_signature: case.get("diff_signature").and_then(Value::as_str).unwrap_or("").to_string(),
                input: case.get("input").and_then(Value::as_str).unwrap_or("").to_string(),
                ts: case.get("ts").and_then(Value::as_str).unwrap_or("").to_string(),
                rust: case.get("rust").and_then(Value::as_str).unwrap_or("").to_string(),
                diff: case.get("diff").and_then(Value::as_str).unwrap_or("").to_string(),
                meta: case.get("meta").and_then(Value::as_str).unwrap_or("").to_string(),
            },
        );
    }

    Ok(MinerRecords {
        time: Timestamp::now().display,
        config: MinerConfigRecord {
            library: Some(config.library.display().to_string()),
            md5_tool: config.md5_tool.as_ref().map(|path| path.display().to_string()),
            out_dir: config.out_dir.display().to_string(),
            shared_cache_dir: config.shared_cache_dir.display().to_string(),
            bun_cache_dir: config.shared_cache_dir.join("bun").display().to_string(),
            modes: config.modes.clone(),
            ffa_sizes: config.ffa_sizes.clone(),
            case_offset_per_mode: config.case_offset_per_mode,
            max_cases_per_mode: config.max_cases_per_mode,
            keep_going: config.keep_going,
        },
        summary: MinerSummaryRecord {
            total_generated: json_usize(summary, "total_generated"),
            unique_inputs: json_usize(summary, "unique_inputs"),
            executed: json_usize(summary, "executed"),
            ts_cache_hits: json_usize(summary, "ts_cache_hits"),
            ts_cache_misses: json_usize(summary, "ts_cache_misses"),
            bun_invocations: json_usize(summary, "bun_invocations"),
            ts_failures: json_usize(summary, "ts_failures"),
            rust_failures: json_usize(summary, "rust_failures"),
            diff_failures: json_usize(summary, "diff_failures"),
            deduped_diff_failures: json_usize(summary, "deduped_diff_failures"),
            per_mode_generated: json_usize_map(summary, "per_mode_generated"),
            per_mode_failures: json_usize_map(summary, "per_mode_failures"),
        },
        failed_cases,
    })
}

fn json_usize(value: &Value, key: &str) -> usize { value.get(key).and_then(Value::as_u64).unwrap_or(0) as usize }

fn json_usize_map(value: &Value, key: &str) -> BTreeMap<String, usize> {
    value
        .get(key)
        .and_then(Value::as_object)
        .map(|obj| {
            obj.iter()
                .map(|(key, value)| (key.clone(), value.as_u64().unwrap_or(0) as usize))
                .collect()
        })
        .unwrap_or_default()
}

fn find_scope_mismatches(current: &MinerRecords, previous: &MinerRecords) -> Vec<ScopeMismatch> {
    if previous.config.out_dir.is_empty() {
        return Vec::new();
    }
    let fields = [
        (
            "library",
            "号库",
            current.config.library.as_deref().unwrap_or(""),
            previous.config.library.as_deref().unwrap_or(""),
        ),
        (
            "md5_tool",
            "md5 工具",
            current.config.md5_tool.as_deref().unwrap_or(""),
            previous.config.md5_tool.as_deref().unwrap_or(""),
        ),
        ("modes", "modes", current.config.modes.as_str(), previous.config.modes.as_str()),
        (
            "ffa_sizes",
            "ffa sizes",
            current.config.ffa_sizes.as_str(),
            previous.config.ffa_sizes.as_str(),
        ),
    ];
    let mut mismatches = Vec::new();
    for (key, label, current_value, previous_value) in fields {
        if current_value != previous_value {
            mismatches.push(ScopeMismatch::new(key, label, current_value, previous_value));
        }
    }
    if current.config.case_offset_per_mode != previous.config.case_offset_per_mode {
        mismatches.push(ScopeMismatch::new(
            "case_offset_per_mode",
            "case offset per mode",
            &current.config.case_offset_per_mode.to_string(),
            &previous.config.case_offset_per_mode.to_string(),
        ));
    }
    if current.config.max_cases_per_mode != previous.config.max_cases_per_mode {
        mismatches.push(ScopeMismatch::new(
            "max_cases_per_mode",
            "max cases per mode",
            &current.config.max_cases_per_mode.to_string(),
            &previous.config.max_cases_per_mode.to_string(),
        ));
    }
    mismatches
}

struct ScopeMismatch {
    label: String,
    current: String,
    previous: String,
}

impl ScopeMismatch {
    fn new(_key: &str, label: &str, current: &str, previous: &str) -> Self {
        Self {
            label: label.to_string(),
            current: current.to_string(),
            previous: previous.to_string(),
        }
    }
}

fn print_scope_mismatches(mismatches: &[ScopeMismatch]) {
    println!("比较范围已变化，跳过 failed case 直接对比:");
    for item in mismatches {
        println!("  {}: {:?} -> {:?}", item.label, item.previous, item.current);
    }
}

fn compare_records(current: &MinerRecords, previous: &MinerRecords) -> Vec<Change> {
    let all_case_ids = current
        .failed_cases
        .keys()
        .chain(previous.failed_cases.keys())
        .cloned()
        .collect::<BTreeSet<String>>();
    let mut changes = Vec::new();
    for case_id in all_case_ids {
        let curr = current.failed_cases.get(&case_id);
        let prev = previous.failed_cases.get(&case_id);
        match (curr, prev) {
            (None, Some(prev)) => changes.push(Change {
                case_id,
                kind: "FIXED_CASE",
                mode: prev.mode.clone(),
                idx: -1,
                prev_idx: prev.idx,
            }),
            (Some(curr), None) => changes.push(Change {
                case_id,
                kind: "NEW_FAILED_CASE",
                mode: curr.mode.clone(),
                idx: curr.idx,
                prev_idx: -1,
            }),
            (Some(curr), Some(prev)) if curr.idx > prev.idx => changes.push(Change {
                case_id,
                kind: "IMPROVED",
                mode: curr.mode.clone(),
                idx: curr.idx,
                prev_idx: prev.idx,
            }),
            (Some(curr), Some(prev)) if curr.idx < prev.idx => changes.push(Change {
                case_id,
                kind: "REGRESSED",
                mode: curr.mode.clone(),
                idx: curr.idx,
                prev_idx: prev.idx,
            }),
            _ => {}
        }
    }
    changes
}

fn print_changes(changes: &[Change]) {
    for change in changes {
        match change.kind {
            "IMPROVED" => println!(
                "[改进] {} ({}): idx {} -> {}",
                change.case_id, change.mode, change.prev_idx, change.idx
            ),
            "REGRESSED" => println!(
                "[退步] {} ({}): idx {} -> {}",
                change.case_id, change.mode, change.prev_idx, change.idx
            ),
            "NEW_FAILED_CASE" => println!("[新失败] {} ({}): idx={}", change.case_id, change.mode, change.idx),
            "FIXED_CASE" => println!("[修复] {} ({}): 上次 idx={}", change.case_id, change.mode, change.prev_idx),
            _ => {}
        }
    }
}

fn print_conclusion(changes: &[Change]) {
    let any_improved = changes.iter().any(|c| c.kind == "IMPROVED" || c.kind == "FIXED_CASE");
    let any_regressed = changes.iter().any(|c| c.kind == "REGRESSED" || c.kind == "NEW_FAILED_CASE");
    if any_improved && !any_regressed {
        println!("结论: 修改有效 (有改进且无退步)，可进行 commit 提交");
    } else if any_regressed {
        println!("结论: 修改有问题 (存在退步)");
    } else {
        println!("结论: 无明显变化");
    }
}

fn print_checkpoint_comparison(paths: &Paths, current: &MinerRecords, quiet: bool) -> Result<(), String> {
    let Some(cp) = latest_checkpoint(&paths.checkpoint_dir)? else {
        return Ok(());
    };
    println!();
    println!("--- vs 存档点 \"{}\" ({}) ---", cp.name, cp.time);
    let scope_mismatches = find_scope_mismatches(current, &cp.records);
    if !scope_mismatches.is_empty() {
        print_scope_mismatches(&scope_mismatches);
        return Ok(());
    }
    let changes = compare_records(current, &cp.records);
    if !quiet {
        print_changes(&changes);
    }
    print_conclusion(&changes);
    Ok(())
}

fn print_current_status(records: &MinerRecords) {
    println!("当前 miner 失败状态:");
    println!("  ts_cache_hits={}", records.summary.ts_cache_hits);
    println!("  ts_cache_misses={}", records.summary.ts_cache_misses);
    println!("  bun_invocations={}", records.summary.bun_invocations);
    println!("  diff_failures={}", records.summary.diff_failures);
    println!("  deduped_diff_failures={}", records.summary.deduped_diff_failures);
    for (mode, count) in &records.summary.per_mode_failures {
        println!("  {mode}: {count}");
    }
    if !records.failed_cases.is_empty() {
        println!();
        println!("failed cases:");
        for (case_id, info) in &records.failed_cases {
            println!("  {case_id} => mode={} idx={}", info.mode, info.idx);
        }
    }
}

fn cmd_save(paths: &Paths, name: Option<String>) -> Result<(), String> {
    let records = load_records(&paths.record_file);
    let now = Timestamp::now();
    let name = name.unwrap_or_else(|| now.compact.clone());
    if let Some(existing) = find_checkpoint(&paths.checkpoint_dir, &name)? {
        println!("存档点 \"{name}\" 已存在，覆盖");
        fs::remove_file(existing).map_err(|e| format!("覆盖存档点失败: {e}"))?;
    }
    fs::create_dir_all(&paths.checkpoint_dir).map_err(|e| format!("创建存档点目录失败: {e}"))?;
    let checkpoint = Checkpoint {
        name: name.clone(),
        time: now.display.clone(),
        records,
    };
    let path = paths.checkpoint_dir.join(format!("{}_{}.json", now.compact, sanitize_filename(&name)));
    write_json(&path, &checkpoint)?;
    println!("存档点 \"{name}\" 已保存 ({})", now.short);
    println!("  包含 {} 个 failed case", checkpoint.records.failed_cases.len());
    Ok(())
}

fn cmd_list(paths: &Paths) -> Result<(), String> {
    let files = checkpoint_files(&paths.checkpoint_dir)?;
    if files.is_empty() {
        println!("没有存档点");
        return Ok(());
    }
    println!("存档点列表 ({} 个):", files.len());
    for file in files {
        let cp: Checkpoint = read_json(&file)?;
        println!("  {} ({}) - {} 个 failed case", cp.name, cp.time, cp.records.failed_cases.len());
    }
    Ok(())
}

fn cmd_diff(paths: &Paths, name: Option<&str>) -> Result<(), String> {
    let cp = if let Some(name) = name {
        let path = find_checkpoint(&paths.checkpoint_dir, name)?.ok_or_else(|| format!("存档点 \"{name}\" 不存在"))?;
        read_json::<Checkpoint>(&path)?
    } else {
        latest_checkpoint(&paths.checkpoint_dir)?.ok_or_else(|| "没有存档点".to_string())?
    };
    let current = load_records(&paths.record_file);
    println!("--- vs 存档点 \"{}\" ({}) ---", cp.name, cp.time);
    let scope_mismatches = find_scope_mismatches(&current, &cp.records);
    if !scope_mismatches.is_empty() {
        print_scope_mismatches(&scope_mismatches);
        return Ok(());
    }
    let changes = compare_records(&current, &cp.records);
    print_changes(&changes);
    print_conclusion(&changes);
    Ok(())
}

fn cmd_delete(paths: &Paths, name: &str) -> Result<(), String> {
    let path = find_checkpoint(&paths.checkpoint_dir, name)?.ok_or_else(|| format!("存档点 \"{name}\" 不存在"))?;
    fs::remove_file(path).map_err(|e| format!("删除存档点失败: {e}"))?;
    println!("存档点 \"{name}\" 已删除");
    Ok(())
}

fn empty_records() -> MinerRecords {
    MinerRecords {
        time: String::new(),
        config: MinerConfigRecord::default(),
        summary: MinerSummaryRecord::default(),
        failed_cases: BTreeMap::new(),
    }
}

fn load_records(path: &Path) -> MinerRecords { read_json(path).unwrap_or_else(|_| empty_records()) }

fn save_records(path: &Path, records: &MinerRecords) -> Result<(), String> { write_json(path, records) }

fn write_log(path: &Path, message: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建日志目录失败: {e}"))?;
    }
    let timestamp = Timestamp::now();
    let mut existing = fs::read_to_string(path).unwrap_or_default();
    existing.push_str(&format!("[{}] {message}\n", timestamp.display));
    fs::write(path, existing).map_err(|e| format!("写入日志失败: {e}"))
}

fn checkpoint_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut files = fs::read_dir(dir)
        .map_err(|e| format!("读取存档点目录失败: {e}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect::<Vec<PathBuf>>();
    files.sort();
    files.reverse();
    Ok(files)
}

fn latest_checkpoint(dir: &Path) -> Result<Option<Checkpoint>, String> {
    let Some(path) = checkpoint_files(dir)?.into_iter().next() else {
        return Ok(None);
    };
    Ok(Some(read_json(&path)?))
}

fn find_checkpoint(dir: &Path, name: &str) -> Result<Option<PathBuf>, String> {
    for file in checkpoint_files(dir)? {
        let cp: Checkpoint = read_json(&file)?;
        if cp.name == name {
            return Ok(Some(file));
        }
    }
    Ok(None)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("解析 {} 失败: {e}", path.display()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(value).map_err(|e| format!("序列化 JSON 失败: {e}"))?;
    fs::write(path, raw).map_err(|e| format!("写入 {} 失败: {e}", path.display()))
}

fn detect_default_md5_tool() -> Option<PathBuf> {
    let root = env::current_dir().ok()?;
    let candidates = [
        root.parent()?.join("fast-namerena").join("branch").join("latest").join("out_md5.ts"),
        root.join("fast-namerena").join("branch").join("latest").join("out_md5.ts"),
    ];
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .map(|path| path.canonicalize().unwrap_or(path))
}

fn resolve_runtime_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
    }
}

struct Timestamp {
    compact: String,
    display: String,
    short: String,
}

impl Timestamp {
    fn now() -> Self {
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|dur| dur.as_secs()).unwrap_or(0);
        Self {
            compact: secs.to_string(),
            display: format!("unix:{secs}"),
            short: format!("unix:{secs}"),
        }
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect()
}

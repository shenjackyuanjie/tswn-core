use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use tswn_core::Runner;
use tswn_core::engine::update::{RunUpdate, UpdateType};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum CaseMode {
    OneVsOne,
    TwoVsTwo,
    ThreeVsThreeVsThree,
    FreeForAll(usize),
}

impl CaseMode {
    fn label(self) -> String {
        match self {
            Self::OneVsOne => "1v1".to_string(),
            Self::TwoVsTwo => "2v2".to_string(),
            Self::ThreeVsThreeVsThree => "3v3v3".to_string(),
            Self::FreeForAll(size) => format!("ffa_{size}"),
        }
    }

    fn total_players(self) -> usize {
        match self {
            Self::OneVsOne => 2,
            Self::TwoVsTwo => 4,
            Self::ThreeVsThreeVsThree => 9,
            Self::FreeForAll(size) => size,
        }
    }

    fn build_input(self, players: &[String]) -> String {
        match self {
            Self::OneVsOne | Self::FreeForAll(_) => players.join("\n"),
            Self::TwoVsTwo => format!("{}\n\n{}", players[..2].join("\n"), players[2..4].join("\n")),
            Self::ThreeVsThreeVsThree => format!(
                "{}\n\n{}\n\n{}",
                players[..3].join("\n"),
                players[3..6].join("\n"),
                players[6..9].join("\n")
            ),
        }
    }
}

#[derive(Debug)]
struct Config {
    library: PathBuf,
    md5_tool: PathBuf,
    out_dir: PathBuf,
    modes: Vec<CaseMode>,
    case_offset_per_mode: usize,
    max_cases_per_mode: usize,
    shuffle_seed: u64,
    keep_going: bool,
    save_all: bool,
}

#[derive(Clone, Debug)]
struct GeneratedCase {
    mode: CaseMode,
    players: Vec<String>,
    input: String,
    input_hash: u64,
}

#[derive(Debug)]
struct TsTraceResult {
    input_hash: u64,
    output: Result<String, String>,
    cache_hit: bool,
}

#[derive(Debug, Default)]
struct TsTraceCollection {
    outputs: HashMap<u64, Result<String, String>>,
    cache_hits: usize,
    cache_misses: usize,
}

#[derive(Debug)]
struct CaseRecord {
    id: String,
    mode: String,
    players: Vec<String>,
    input_hash: u64,
    input_path: PathBuf,
    ts_path: PathBuf,
    rust_path: PathBuf,
    diff_path: PathBuf,
    meta_path: PathBuf,
    first_mismatch_idx: usize,
    ts_line_count: usize,
    rust_line_count: usize,
    diff_signature: String,
}

#[derive(Debug)]
struct TsEmptyRecord {
    id: String,
    mode: String,
    players: Vec<String>,
    input_hash: u64,
    input_path: PathBuf,
    ts_path: PathBuf,
    rust_path: PathBuf,
    meta_path: PathBuf,
    ts_line_count: usize,
    rust_line_count: usize,
}

#[derive(Debug, Default)]
struct Summary {
    total_generated: usize,
    unique_inputs: usize,
    executed: usize,
    skipped_duplicate_inputs: usize,
    skipped_insufficient_library: usize,
    ts_cache_hits: usize,
    ts_cache_misses: usize,
    bun_invocations: usize,
    ts_failures: usize,
    rust_failures: usize,
    ts_empty_outputs: usize,
    diff_failures: usize,
    deduped_diff_failures: usize,
    saved_pass_cases: usize,
    saved_failed_cases: usize,
    saved_ts_empty_cases: usize,
    per_mode_generated: BTreeMap<String, usize>,
    per_mode_failures: BTreeMap<String, usize>,
    per_mode_ts_empty_outputs: BTreeMap<String, usize>,
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
    reset_run_output(&config.out_dir)?;

    let library = load_library(&config.library)?;
    if library.len() < 2 {
        return Err(format!("号库有效玩家数不足: {}，至少需要 2 个名字", library.len()));
    }

    let mut names = library;
    deterministic_shuffle(&mut names, config.shuffle_seed);

    let temp_root = config.out_dir.join(".tmp");
    fs::create_dir_all(&temp_root).map_err(|e| format!("创建临时目录失败: {e}"))?;
    let ts_cache_dir = detect_default_ts_cache_dir(&config.out_dir);
    fs::create_dir_all(&ts_cache_dir).map_err(|e| format!("创建 TS 缓存目录失败: {e}"))?;
    let bun_cache_dir = detect_default_bun_cache_dir(&config.out_dir);
    fs::create_dir_all(&bun_cache_dir).map_err(|e| format!("创建 bun 缓存目录失败: {e}"))?;
    let md5_tool_signature = file_signature(&config.md5_tool)?;

    let mut summary = Summary::default();
    let mut seen_inputs = HashSet::new();
    let mut seen_signatures = HashSet::new();
    let mut records = Vec::new();
    let mut ts_empty_records = Vec::new();
    let mut unique_cases = Vec::new();

    for mode in &config.modes {
        let generated = generate_cases_for_mode(
            &names,
            *mode,
            config.case_offset_per_mode,
            config.max_cases_per_mode,
            &mut summary,
        );
        for case in generated {
            summary.total_generated += 1;
            *summary.per_mode_generated.entry(case.mode.label()).or_insert(0) += 1;

            if !seen_inputs.insert(case.input_hash) {
                summary.skipped_duplicate_inputs += 1;
                continue;
            }
            summary.unique_inputs += 1;
            summary.executed += 1;
            unique_cases.push(case);
        }
    }

    println!(
        "阶段 1/3: case 已生成，准备处理 TS trace 缓存 (唯一输入: {})",
        summary.unique_inputs
    );

    let ts_traces = collect_ts_traces(
        &config.md5_tool,
        md5_tool_signature,
        &ts_cache_dir,
        &bun_cache_dir,
        &temp_root,
        &unique_cases,
        8,
    );
    summary.ts_cache_hits = ts_traces.cache_hits;
    summary.ts_cache_misses = ts_traces.cache_misses;
    summary.bun_invocations = ts_traces.cache_misses;

    println!(
        "阶段 2/3: TS trace 缓存完成 (hit={}, miss={}, bun调用={})",
        summary.ts_cache_hits, summary.ts_cache_misses, summary.bun_invocations
    );
    println!("阶段 3/3: 开始执行 Rust trace 并比对输出...");

    for case in unique_cases {
        let ts_output = match ts_traces.outputs.get(&case.input_hash) {
            Some(Ok(output)) => output.clone(),
            Some(Err(err)) => {
                summary.ts_failures += 1;
                if !config.keep_going {
                    return Err(format!("TS 执行失败({}): {err}", case.mode.label()));
                }
                continue;
            }
            None => {
                summary.ts_failures += 1;
                if !config.keep_going {
                    return Err(format!("TS 执行失败({}): 缺少执行结果", case.mode.label()));
                }
                continue;
            }
        };

        let rust_output = match run_rust_trace(&case.input) {
            Ok(output) => output,
            Err(err) => {
                summary.rust_failures += 1;
                if !config.keep_going {
                    return Err(format!("Rust 执行失败({}): {err}", case.mode.label()));
                }
                continue;
            }
        };

        let ts_lines = split_output_lines(&ts_output);
        let rust_lines = split_output_lines(&rust_output);

        if ts_lines.is_empty() {
            summary.ts_empty_outputs += 1;
            *summary.per_mode_ts_empty_outputs.entry(case.mode.label()).or_insert(0) += 1;
            let record = save_ts_empty_case(&config.out_dir, &config.md5_tool, &case, &ts_output, &rust_output)?;
            summary.saved_ts_empty_cases += 1;
            ts_empty_records.push(record);
            continue;
        }

        if ts_lines == rust_lines {
            if config.save_all {
                save_passing_case(&config.out_dir, &case, &ts_output, &rust_output)?;
                summary.saved_pass_cases += 1;
            }
            continue;
        }

        summary.diff_failures += 1;
        *summary.per_mode_failures.entry(case.mode.label()).or_insert(0) += 1;

        let mismatch_idx = first_mismatch_idx(&ts_lines, &rust_lines);
        let signature = diff_signature(&ts_lines, &rust_lines, mismatch_idx);
        if seen_signatures.insert(signature.clone()) {
            summary.deduped_diff_failures += 1;
        }

        let record = save_failed_case(
            &config.out_dir,
            &config.md5_tool,
            &case,
            &ts_output,
            &rust_output,
            mismatch_idx,
            &signature,
        )?;
        summary.saved_failed_cases += 1;
        records.push(record);
    }

    write_summary(&config, &summary, &records, &ts_empty_records)?;
    write_report(&config, &summary, &records, &ts_empty_records)?;

    println!("号库: {}", config.library.display());
    println!("md5 工具: {}", config.md5_tool.display());
    println!("输出目录: {}", config.out_dir.display());
    println!("生成 case: {}", summary.total_generated);
    println!("唯一输入: {}", summary.unique_inputs);
    println!("执行成功: {}", summary.executed - summary.ts_failures - summary.rust_failures);
    println!("TS cache hit: {}", summary.ts_cache_hits);
    println!("TS cache miss: {}", summary.ts_cache_misses);
    println!("bun 调用次数: {}", summary.bun_invocations);
    println!("TS 执行失败: {}", summary.ts_failures);
    println!("Rust 执行失败: {}", summary.rust_failures);
    println!("TS 空输出: {}", summary.ts_empty_outputs);
    println!("failed case: {}", summary.diff_failures);
    println!("去重后失败类别: {}", summary.deduped_diff_failures);

    Ok(())
}

fn parse_args() -> Result<Config, String> {
    let mut library = None;
    let mut md5_tool = None;
    let mut out_dir = None;
    let mut modes = None;
    let mut include_ffa_from_modes = false;
    let mut ffa_sizes = vec![4usize, 6, 8];
    let mut case_offset_per_mode = 0usize;
    let mut max_cases_per_mode = 64usize;
    let mut shuffle_seed = 0x5EED_2026_u64;
    let mut keep_going = false;
    let mut save_all = false;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            "--library" => {
                idx += 1;
                library = Some(PathBuf::from(require_arg(&args, idx, "--library")?));
            }
            "--md5-tool" => {
                idx += 1;
                md5_tool = Some(PathBuf::from(require_arg(&args, idx, "--md5-tool")?));
            }
            "--out-dir" => {
                idx += 1;
                out_dir = Some(PathBuf::from(require_arg(&args, idx, "--out-dir")?));
            }
            "--modes" => {
                idx += 1;
                let (parsed_modes, include_ffa) = parse_mode_names(require_arg(&args, idx, "--modes")?)?;
                modes = Some(parsed_modes);
                include_ffa_from_modes = include_ffa;
            }
            "--ffa-sizes" => {
                idx += 1;
                ffa_sizes = parse_usize_csv(require_arg(&args, idx, "--ffa-sizes")?, "--ffa-sizes")?;
            }
            "--case-offset-per-mode" => {
                idx += 1;
                case_offset_per_mode = require_arg(&args, idx, "--case-offset-per-mode")?
                    .parse::<usize>()
                    .map_err(|e| format!("解析 --case-offset-per-mode 失败: {e}"))?;
            }
            "--max-cases-per-mode" => {
                idx += 1;
                max_cases_per_mode = require_arg(&args, idx, "--max-cases-per-mode")?
                    .parse::<usize>()
                    .map_err(|e| format!("解析 --max-cases-per-mode 失败: {e}"))?;
                if max_cases_per_mode == 0 {
                    return Err("--max-cases-per-mode 必须大于 0".to_string());
                }
            }
            "--shuffle-seed" => {
                idx += 1;
                shuffle_seed = require_arg(&args, idx, "--shuffle-seed")?
                    .parse::<u64>()
                    .map_err(|e| format!("解析 --shuffle-seed 失败: {e}"))?;
            }
            "--keep-going" => keep_going = true,
            "--save-all" => save_all = true,
            other => return Err(format!("未知参数: {other}")),
        }
        idx += 1;
    }

    let library = match library {
        Some(path) => path,
        None => detect_default_library()
            .ok_or_else(|| "缺少 --library <path>，且无法自动推导共享号库 tests/sqp6000.txt".to_string())?,
    };
    let md5_tool = match md5_tool {
        Some(path) => path,
        None => detect_default_md5_tool()
            .ok_or_else(|| "缺少 --md5-tool <path>，且无法自动推导 fast-namerena/branch/latest/out_md5.ts".to_string())?,
    };
    let out_dir = out_dir.unwrap_or_else(|| PathBuf::from("target").join("ts_diff_cases"));

    if !library.is_file() {
        return Err(format!("号库文件不存在: {}", library.display()));
    }
    if !md5_tool.is_file() {
        return Err(format!("md5 工具文件不存在: {}", md5_tool.display()));
    }

    let mut modes = modes.unwrap_or_else(|| vec![CaseMode::OneVsOne, CaseMode::TwoVsTwo, CaseMode::ThreeVsThreeVsThree]);
    let include_ffa = if args.iter().any(|arg| arg == "--modes") {
        include_ffa_from_modes
    } else {
        true
    };
    if include_ffa {
        modes.extend(ffa_sizes.iter().copied().map(CaseMode::FreeForAll));
    }

    Ok(Config {
        library,
        md5_tool,
        out_dir,
        modes,
        case_offset_per_mode,
        max_cases_per_mode,
        shuffle_seed,
        keep_going,
        save_all,
    })
}

fn print_usage() {
    println!(
        r#"用法:
    tswn_case_miner [选项]

选项:
    --library <path>          号库文件；默认共享仓库 tests/sqp6000.txt
    --md5-tool <path>         TS 基准工具路径；默认自动推导 fast-namerena/branch/latest/out_md5.ts
  --out-dir <path>          failed case 输出目录（默认 target/ts_diff_cases）
  --modes <csv>             生成模式，默认 1v1,2v2,3v3v3,ffa
  --ffa-sizes <csv>         自由混战人数，默认 4,6,8
  --case-offset-per-mode <N> 每种模式按稳定顺序跳过前 N 个唯一 case，默认 0
  --max-cases-per-mode <N>  每种模式最多生成多少 case，默认 64
  --shuffle-seed <N>        固定采样顺序，默认 1592597030
  --keep-going              个别 case 执行失败时继续
  --save-all                连通过 case 也一起保存
"#
    );
}

fn require_arg<'a>(args: &'a [String], idx: usize, flag: &str) -> Result<&'a str, String> {
    args.get(idx).map(String::as_str).ok_or_else(|| format!("{flag} 缺少参数"))
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
        let value = token.parse::<usize>().map_err(|e| format!("解析 {flag} 中的值失败({token}): {e}"))?;
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

fn strip_utf8_bom(s: &str) -> &str { s.strip_prefix('\u{feff}').unwrap_or(s) }

fn load_library(path: &Path) -> Result<Vec<String>, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("读取号库失败: {e}"))?;
    let raw = strip_utf8_bom(&raw);
    let mut seen = HashSet::new();
    let mut names = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            names.push(trimmed.to_string());
        }
    }
    Ok(names)
}

fn deterministic_shuffle(values: &mut [String], seed: u64) {
    if values.len() < 2 {
        return;
    }
    let mut state = seed ^ 0x9E37_79B9_7F4A_7C15;
    for idx in (1..values.len()).rev() {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let swap_idx = (state % ((idx + 1) as u64)) as usize;
        values.swap(idx, swap_idx);
    }
}

fn generate_cases_for_mode(
    names: &[String],
    mode: CaseMode,
    case_offset: usize,
    max_cases: usize,
    summary: &mut Summary,
) -> Vec<GeneratedCase> {
    let total = mode.total_players();
    if names.len() < total {
        summary.skipped_insufficient_library += 1;
        return Vec::new();
    }

    let mut cases = Vec::new();
    let mut seen_hashes = HashSet::new();
    let mut skipped = 0usize;

    // 单一连续窗口最多只能产出 names.len() 个唯一 case。
    // 这里改成“多组互质步长 + 滚动起点”，在保持确定性的前提下，
    // 能从同一份号库中稳定扩出更多唯一输入，满足 500-case 基线。
    for step in candidate_steps(names.len()) {
        for offset_idx in 0..names.len() {
            if cases.len() >= max_cases {
                return cases;
            }
            let start = (offset_idx * step) % names.len();
            let players = sample_unique_window(names, start, total, step);
            let input = mode.build_input(&players);
            let input_hash = stable_hash(&input);
            if !seen_hashes.insert(input_hash) {
                continue;
            }
            if skipped < case_offset {
                skipped += 1;
                continue;
            }
            cases.push(GeneratedCase {
                mode,
                players,
                input,
                input_hash,
            });
        }
    }

    cases
}

fn candidate_steps(len: usize) -> Vec<usize> {
    let mut steps = Vec::new();
    for candidate in [1usize, 7, 11, 13, 17, 19, 23, 29, 31] {
        if candidate < len && gcd(candidate, len) == 1 {
            steps.push(candidate);
        }
    }
    if steps.is_empty() {
        steps.push(1);
    }
    steps
}

fn gcd(mut lhs: usize, mut rhs: usize) -> usize {
    while rhs != 0 {
        let rem = lhs % rhs;
        lhs = rhs;
        rhs = rem;
    }
    lhs
}

fn sample_unique_window(names: &[String], start: usize, total: usize, step: usize) -> Vec<String> {
    let mut players = Vec::with_capacity(total);
    for idx in 0..total {
        players.push(names[(start + idx * step) % names.len()].clone());
    }
    players
}

fn detect_default_library() -> Option<PathBuf> {
    let shared_repo_root = detect_shared_repo_root()?;
    let candidate = shared_repo_root.join("tests").join("sqp6000.txt");
    if candidate.is_file() { Some(candidate) } else { None }
}

fn detect_default_md5_tool() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let repo_root = find_repo_root(&cwd).unwrap_or(cwd);
    let shared_repo_root = detect_shared_repo_root().unwrap_or_else(|| repo_root.clone());

    let mut candidates = Vec::new();
    for root in [repo_root, shared_repo_root] {
        if let Some(parent) = root.parent() {
            candidates.push(parent.join("fast-namerena").join("branch").join("latest").join("out_md5.ts"));
        }
        candidates.push(root.join("fast-namerena").join("branch").join("latest").join("out_md5.ts"));
    }

    candidates.into_iter().find(|path| path.is_file())
}

fn detect_default_ts_cache_dir(out_dir: &Path) -> PathBuf {
    path_from_env("TSWN_CASE_MINER_TS_CACHE_DIR")
        .or_else(|| detect_shared_cache_root().map(|root| root.join("ts_trace")))
        .unwrap_or_else(|| out_dir.join(".ts_cache"))
}

fn detect_default_bun_cache_dir(out_dir: &Path) -> PathBuf {
    path_from_env("TSWN_CASE_MINER_BUN_CACHE_DIR")
        .or_else(|| detect_shared_cache_root().map(|root| root.join("bun")))
        .unwrap_or_else(|| out_dir.join(".bun_cache"))
}

fn path_from_env(name: &str) -> Option<PathBuf> { std::env::var_os(name).map(PathBuf::from) }

fn detect_shared_cache_root() -> Option<PathBuf> {
    let shared_repo_root = detect_shared_repo_root()?;
    Some(shared_repo_root.join("target").join("tswn_case_miner_cache"))
}

fn detect_shared_repo_root() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let repo_root = find_repo_root(&cwd)?;
    let common_dir = detect_git_common_dir_from_repo_root(&repo_root)?;
    if common_dir.file_name().and_then(|name| name.to_str()) == Some(".git") {
        common_dir.parent().map(Path::to_path_buf)
    } else {
        Some(repo_root)
    }
}

fn find_repo_root(start: &Path) -> Option<PathBuf> {
    for candidate in start.ancestors() {
        let git_entry = candidate.join(".git");
        if git_entry.is_dir() || git_entry.is_file() {
            return Some(candidate.to_path_buf());
        }
    }
    None
}

fn detect_git_common_dir_from_repo_root(repo_root: &Path) -> Option<PathBuf> {
    let git_entry = repo_root.join(".git");
    if git_entry.is_dir() {
        return Some(normalize_existing_path(git_entry));
    }

    let git_dir = parse_gitdir_file(&git_entry)?;
    let common_dir_file = git_dir.join("commondir");
    if common_dir_file.is_file() {
        let common_dir = PathBuf::from(read_first_line(&common_dir_file)?);
        let resolved = if common_dir.is_absolute() {
            common_dir
        } else {
            git_dir.join(common_dir)
        };
        return Some(normalize_existing_path(resolved));
    }

    Some(normalize_existing_path(git_dir))
}

fn parse_gitdir_file(git_entry: &Path) -> Option<PathBuf> {
    let raw = read_first_line(git_entry)?;
    let git_dir = PathBuf::from(raw.strip_prefix("gitdir:")?.trim());
    let resolved = if git_dir.is_absolute() {
        git_dir
    } else {
        git_entry.parent()?.join(git_dir)
    };
    Some(normalize_existing_path(resolved))
}

fn read_first_line(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let raw = strip_utf8_bom(&raw);
    raw.lines().next().map(str::trim).map(str::to_string).filter(|line| !line.is_empty())
}

fn normalize_existing_path(path: PathBuf) -> PathBuf { path.canonicalize().unwrap_or(path) }

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_names(count: usize) -> Vec<String> { (0..count).map(|idx| format!("name_{idx}")).collect() }

    #[test]
    fn generate_cases_for_mode_can_expand_beyond_library_len() {
        let names = sample_names(329);
        let mut summary = Summary::default();

        let cases = generate_cases_for_mode(&names, CaseMode::OneVsOne, 0, 500, &mut summary);
        let unique = cases.iter().map(|case| case.input_hash).collect::<HashSet<_>>();

        assert_eq!(cases.len(), 500);
        assert_eq!(unique.len(), 500);
    }

    #[test]
    fn generated_cases_keep_players_unique_within_case() {
        let names = sample_names(64);
        let mut summary = Summary::default();

        let cases = generate_cases_for_mode(&names, CaseMode::FreeForAll(8), 0, 128, &mut summary);

        assert_eq!(cases.len(), 128);
        for case in cases {
            let unique = case.players.iter().collect::<HashSet<_>>();
            assert_eq!(unique.len(), case.players.len());
        }
    }

    #[test]
    fn generate_cases_for_mode_offset_matches_stable_slice() {
        let names = sample_names(329);
        let mut full_summary = Summary::default();
        let mut offset_summary = Summary::default();

        let full = generate_cases_for_mode(&names, CaseMode::TwoVsTwo, 0, 40, &mut full_summary);
        let offset = generate_cases_for_mode(&names, CaseMode::TwoVsTwo, 12, 10, &mut offset_summary);

        let expected = full[12..22].iter().map(|case| case.input_hash).collect::<Vec<_>>();
        let actual = offset.iter().map(|case| case.input_hash).collect::<Vec<_>>();

        assert_eq!(actual, expected);
    }
}

fn run_ts_trace(md5_tool: &Path, bun_cache_dir: &Path, temp_root: &Path, case: &GeneratedCase) -> Result<String, String> {
    let temp_input = temp_root.join(format!("input-{:016x}.txt", case.input_hash));
    fs::write(&temp_input, &case.input).map_err(|e| format!("写入 TS 临时输入失败: {e}"))?;
    let bun_install_cache_dir = bun_cache_dir.join("install");
    fs::create_dir_all(&bun_install_cache_dir).map_err(|e| format!("创建 bun 安装缓存目录失败: {e}"))?;
    let bun_runtime_cache_path = bun_cache_dir.join("runtime_transpiler.cache");

    let output = Command::new("bun")
        .env("BUN_INSTALL_CACHE_DIR", &bun_install_cache_dir)
        .env("BUN_RUNTIME_TRANSPILER_CACHE_PATH", &bun_runtime_cache_path)
        .arg(md5_tool)
        .arg(&temp_input)
        .output()
        .map_err(|e| format!("调用 bun 失败: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "bun 退出码 {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(normalize_command_output(&String::from_utf8_lossy(&output.stdout)))
}

fn collect_ts_traces(
    md5_tool: &Path,
    md5_tool_signature: u64,
    cache_dir: &Path,
    bun_cache_dir: &Path,
    temp_root: &Path,
    cases: &[GeneratedCase],
    max_jobs: usize,
) -> TsTraceCollection {
    let jobs = max_jobs.clamp(1, 8);
    let cases = Arc::new(cases.to_vec());
    let index = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = mpsc::channel::<TsTraceResult>();
    let md5_tool = md5_tool.to_path_buf();
    let cache_dir = cache_dir.to_path_buf();
    let bun_cache_dir = bun_cache_dir.to_path_buf();
    let temp_root = temp_root.to_path_buf();

    let mut handles = Vec::new();
    for _ in 0..jobs {
        let tx = tx.clone();
        let cases = Arc::clone(&cases);
        let index = Arc::clone(&index);
        let md5_tool = md5_tool.clone();
        let cache_dir = cache_dir.clone();
        let bun_cache_dir = bun_cache_dir.clone();
        let temp_root = temp_root.clone();
        handles.push(thread::spawn(move || {
            loop {
                let next = index.fetch_add(1, Ordering::Relaxed);
                let Some(case) = cases.get(next) else {
                    break;
                };
                let (output, cache_hit) =
                    load_or_run_ts_trace(&md5_tool, md5_tool_signature, &cache_dir, &bun_cache_dir, &temp_root, case);
                let _ = tx.send(TsTraceResult {
                    input_hash: case.input_hash,
                    output,
                    cache_hit,
                });
            }
        }));
    }
    drop(tx);

    let mut results = HashMap::with_capacity(cases.len());
    let mut cache_hits = 0usize;
    let mut cache_misses = 0usize;
    for result in rx {
        if result.cache_hit {
            cache_hits += 1;
        } else {
            cache_misses += 1;
        }
        results.insert(result.input_hash, result.output);
    }

    for handle in handles {
        let _ = handle.join();
    }

    TsTraceCollection {
        outputs: results,
        cache_hits,
        cache_misses,
    }
}

fn load_or_run_ts_trace(
    md5_tool: &Path,
    md5_tool_signature: u64,
    cache_dir: &Path,
    bun_cache_dir: &Path,
    temp_root: &Path,
    case: &GeneratedCase,
) -> (Result<String, String>, bool) {
    let cache_path = cache_dir.join(format!("{md5_tool_signature:016x}-{:016x}.txt", case.input_hash));
    if let Ok(cached) = fs::read_to_string(&cache_path) {
        return (Ok(cached), true);
    }

    let output = match run_ts_trace(md5_tool, bun_cache_dir, temp_root, case) {
        Ok(output) => output,
        Err(err) => return (Err(err), false),
    };
    if let Err(err) = fs::write(&cache_path, &output) {
        return (Err(format!("写入 TS 缓存失败: {err}")), false);
    }
    (Ok(output), false)
}

fn file_signature(path: &Path) -> Result<u64, String> {
    let bytes = fs::read(path).map_err(|e| format!("读取文件失败({}): {e}", path.display()))?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    Ok(hasher.finish())
}

fn run_rust_trace(input: &str) -> Result<String, String> {
    let mut runner = Runner::new_from_namerena_raw(input.to_string()).map_err(|e| format!("构建对局失败: {e}"))?;
    let lines = collect_fight_raw_lines(&mut runner);
    Ok(lines.join("\n"))
}

fn normalize_command_output(raw: &str) -> String {
    raw.replace("\r\n", "\n").replace('\r', "\n").trim_end_matches('\n').to_string()
}

fn split_output_lines(output: &str) -> Vec<String> {
    if output.is_empty() {
        Vec::new()
    } else {
        output.split('\n').map(|line| line.to_string()).collect()
    }
}

fn first_mismatch_idx(lhs: &[String], rhs: &[String]) -> usize {
    let min_len = lhs.len().min(rhs.len());
    lhs.iter().zip(rhs.iter()).position(|(a, b)| a != b).unwrap_or(min_len)
}

fn diff_signature(ts_lines: &[String], rust_lines: &[String], mismatch_idx: usize) -> String {
    let start = mismatch_idx.saturating_sub(2);
    let end = (mismatch_idx + 3).min(ts_lines.len().max(rust_lines.len()));
    let mut buf = format!("idx={mismatch_idx};ts_len={};rust_len={};", ts_lines.len(), rust_lines.len());
    for idx in start..end {
        let _ = write!(
            &mut buf,
            "ts[{idx}]={:?};rust[{idx}]={:?};",
            ts_lines.get(idx),
            rust_lines.get(idx)
        );
    }
    format!("{:016x}", stable_hash(&buf))
}

fn stable_hash<T: Hash>(value: &T) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn case_id(mode: CaseMode, input_hash: u64) -> String { format!("{}-{:016x}", mode.label(), input_hash) }

fn reset_run_output(out_dir: &Path) -> Result<(), String> {
    for name in ["failed", "passed", "ts_empty", ".tmp"] {
        let path = out_dir.join(name);
        if path.exists() {
            fs::remove_dir_all(&path).map_err(|e| format!("清理 {} 失败: {e}", path.display()))?;
        }
    }
    for name in ["summary.json", "report.md"] {
        let path = out_dir.join(name);
        if path.exists() {
            fs::remove_file(&path).map_err(|e| format!("清理 {} 失败: {e}", path.display()))?;
        }
    }
    Ok(())
}

fn save_passing_case(out_dir: &Path, case: &GeneratedCase, ts_output: &str, rust_output: &str) -> Result<(), String> {
    let case_dir = out_dir.join("passed").join(case_id(case.mode, case.input_hash));
    fs::create_dir_all(&case_dir).map_err(|e| format!("创建通过样本目录失败: {e}"))?;
    fs::write(case_dir.join("input.txt"), &case.input).map_err(|e| format!("写入 input.txt 失败: {e}"))?;
    fs::write(case_dir.join("ts.txt"), ts_output).map_err(|e| format!("写入 ts.txt 失败: {e}"))?;
    fs::write(case_dir.join("rust.txt"), rust_output).map_err(|e| format!("写入 rust.txt 失败: {e}"))?;
    Ok(())
}

fn save_ts_empty_case(
    out_dir: &Path,
    md5_tool: &Path,
    case: &GeneratedCase,
    ts_output: &str,
    rust_output: &str,
) -> Result<TsEmptyRecord, String> {
    let id = case_id(case.mode, case.input_hash);
    let case_dir = out_dir.join("ts_empty").join(&id);
    fs::create_dir_all(&case_dir).map_err(|e| format!("创建 ts_empty case 目录失败: {e}"))?;

    let input_path = case_dir.join("input.txt");
    let ts_path = case_dir.join("ts.txt");
    let rust_path = case_dir.join("rust.txt");
    let meta_path = case_dir.join("meta.json");

    fs::write(&input_path, &case.input).map_err(|e| format!("写入 input.txt 失败: {e}"))?;
    fs::write(&ts_path, ts_output).map_err(|e| format!("写入 ts.txt 失败: {e}"))?;
    fs::write(&rust_path, rust_output).map_err(|e| format!("写入 rust.txt 失败: {e}"))?;

    let ts_line_count = split_output_lines(ts_output).len();
    let rust_line_count = split_output_lines(rust_output).len();
    let meta = format!(
        "{{\n  \"id\": \"{}\",\n  \"mode\": \"{}\",\n  \"players\": {},\n  \"input_hash\": \"{:016x}\",\n  \"reason\": \"ts_output_empty\",\n  \"ts_line_count\": {},\n  \"rust_line_count\": {},\n  \"md5_tool_path\": \"{}\"\n}}\n",
        json_escape(&id),
        json_escape(&case.mode.label()),
        json_string_array(&case.players),
        case.input_hash,
        ts_line_count,
        rust_line_count,
        json_escape(&md5_tool.display().to_string()),
    );
    fs::write(&meta_path, meta).map_err(|e| format!("写入 meta.json 失败: {e}"))?;

    Ok(TsEmptyRecord {
        id,
        mode: case.mode.label(),
        players: case.players.clone(),
        input_hash: case.input_hash,
        input_path,
        ts_path,
        rust_path,
        meta_path,
        ts_line_count,
        rust_line_count,
    })
}

fn save_failed_case(
    out_dir: &Path,
    md5_tool: &Path,
    case: &GeneratedCase,
    ts_output: &str,
    rust_output: &str,
    mismatch_idx: usize,
    signature: &str,
) -> Result<CaseRecord, String> {
    let id = case_id(case.mode, case.input_hash);
    let case_dir = out_dir.join("failed").join(&id);
    fs::create_dir_all(&case_dir).map_err(|e| format!("创建 failed case 目录失败: {e}"))?;

    let input_path = case_dir.join("input.txt");
    let ts_path = case_dir.join("ts.txt");
    let rust_path = case_dir.join("rust.txt");
    let diff_path = case_dir.join("diff.txt");
    let meta_path = case_dir.join("meta.json");

    fs::write(&input_path, &case.input).map_err(|e| format!("写入 input.txt 失败: {e}"))?;
    fs::write(&ts_path, ts_output).map_err(|e| format!("写入 ts.txt 失败: {e}"))?;
    fs::write(&rust_path, rust_output).map_err(|e| format!("写入 rust.txt 失败: {e}"))?;
    fs::write(&diff_path, build_failed_diff(ts_output, rust_output)).map_err(|e| format!("写入 diff.txt 失败: {e}"))?;

    let meta = format!(
        "{{\n  \"id\": \"{}\",\n  \"mode\": \"{}\",\n  \"players\": {},\n  \"input_hash\": \"{:016x}\",\n  \"first_mismatch_idx\": {},\n  \"ts_line_count\": {},\n  \"rust_line_count\": {},\n  \"diff_signature\": \"{}\",\n  \"md5_tool_path\": \"{}\"\n}}\n",
        json_escape(&id),
        json_escape(&case.mode.label()),
        json_string_array(&case.players),
        case.input_hash,
        mismatch_idx,
        split_output_lines(ts_output).len(),
        split_output_lines(rust_output).len(),
        json_escape(signature),
        json_escape(&md5_tool.display().to_string()),
    );
    fs::write(&meta_path, meta).map_err(|e| format!("写入 meta.json 失败: {e}"))?;

    Ok(CaseRecord {
        id,
        mode: case.mode.label(),
        players: case.players.clone(),
        input_hash: case.input_hash,
        input_path,
        ts_path,
        rust_path,
        diff_path,
        meta_path,
        first_mismatch_idx: mismatch_idx,
        ts_line_count: split_output_lines(ts_output).len(),
        rust_line_count: split_output_lines(rust_output).len(),
        diff_signature: signature.to_string(),
    })
}

fn write_summary(
    config: &Config,
    summary: &Summary,
    records: &[CaseRecord],
    ts_empty_records: &[TsEmptyRecord],
) -> Result<(), String> {
    let mut json = String::new();
    let _ = writeln!(&mut json, "{{");
    let _ = writeln!(
        &mut json,
        "  \"library\": \"{}\",",
        json_escape(&config.library.display().to_string())
    );
    let _ = writeln!(
        &mut json,
        "  \"md5_tool\": \"{}\",",
        json_escape(&config.md5_tool.display().to_string())
    );
    let _ = writeln!(
        &mut json,
        "  \"out_dir\": \"{}\",",
        json_escape(&config.out_dir.display().to_string())
    );
    let _ = writeln!(&mut json, "  \"total_generated\": {},", summary.total_generated);
    let _ = writeln!(&mut json, "  \"unique_inputs\": {},", summary.unique_inputs);
    let _ = writeln!(&mut json, "  \"executed\": {},", summary.executed);
    let _ = writeln!(&mut json, "  \"ts_cache_hits\": {},", summary.ts_cache_hits);
    let _ = writeln!(&mut json, "  \"ts_cache_misses\": {},", summary.ts_cache_misses);
    let _ = writeln!(&mut json, "  \"bun_invocations\": {},", summary.bun_invocations);
    let _ = writeln!(
        &mut json,
        "  \"skipped_duplicate_inputs\": {},",
        summary.skipped_duplicate_inputs
    );
    let _ = writeln!(
        &mut json,
        "  \"skipped_insufficient_library\": {},",
        summary.skipped_insufficient_library
    );
    let _ = writeln!(&mut json, "  \"ts_failures\": {},", summary.ts_failures);
    let _ = writeln!(&mut json, "  \"rust_failures\": {},", summary.rust_failures);
    let _ = writeln!(&mut json, "  \"ts_empty_outputs\": {},", summary.ts_empty_outputs);
    let _ = writeln!(&mut json, "  \"diff_failures\": {},", summary.diff_failures);
    let _ = writeln!(&mut json, "  \"deduped_diff_failures\": {},", summary.deduped_diff_failures);
    let _ = writeln!(&mut json, "  \"saved_pass_cases\": {},", summary.saved_pass_cases);
    let _ = writeln!(&mut json, "  \"saved_failed_cases\": {},", summary.saved_failed_cases);
    let _ = writeln!(&mut json, "  \"saved_ts_empty_cases\": {},", summary.saved_ts_empty_cases);
    let _ = writeln!(
        &mut json,
        "  \"per_mode_generated\": {},",
        json_btreemap(&summary.per_mode_generated)
    );
    let _ = writeln!(
        &mut json,
        "  \"per_mode_failures\": {},",
        json_btreemap(&summary.per_mode_failures)
    );
    let _ = writeln!(
        &mut json,
        "  \"per_mode_ts_empty_outputs\": {},",
        json_btreemap(&summary.per_mode_ts_empty_outputs)
    );
    let _ = writeln!(&mut json, "  \"failed_cases\": [");
    for (idx, record) in records.iter().enumerate() {
        let comma = if idx + 1 == records.len() { "" } else { "," };
        let _ = writeln!(
            &mut json,
            "    {{\"id\":\"{}\",\"mode\":\"{}\",\"first_mismatch_idx\":{},\"input_hash\":\"{:016x}\",\"diff_signature\":\"{}\",\"input\":\"{}\",\"ts\":\"{}\",\"rust\":\"{}\",\"diff\":\"{}\",\"meta\":\"{}\"}}{}",
            json_escape(&record.id),
            json_escape(&record.mode),
            record.first_mismatch_idx,
            record.input_hash,
            json_escape(&record.diff_signature),
            json_escape(&record.input_path.display().to_string()),
            json_escape(&record.ts_path.display().to_string()),
            json_escape(&record.rust_path.display().to_string()),
            json_escape(&record.diff_path.display().to_string()),
            json_escape(&record.meta_path.display().to_string()),
            comma
        );
    }
    let _ = writeln!(&mut json, "  ],");
    let _ = writeln!(&mut json, "  \"ts_empty_cases\": [");
    for (idx, record) in ts_empty_records.iter().enumerate() {
        let comma = if idx + 1 == ts_empty_records.len() { "" } else { "," };
        let _ = writeln!(
            &mut json,
            "    {{\"id\":\"{}\",\"mode\":\"{}\",\"input_hash\":\"{:016x}\",\"ts_line_count\":{},\"rust_line_count\":{},\"input\":\"{}\",\"ts\":\"{}\",\"rust\":\"{}\",\"meta\":\"{}\"}}{}",
            json_escape(&record.id),
            json_escape(&record.mode),
            record.input_hash,
            record.ts_line_count,
            record.rust_line_count,
            json_escape(&record.input_path.display().to_string()),
            json_escape(&record.ts_path.display().to_string()),
            json_escape(&record.rust_path.display().to_string()),
            json_escape(&record.meta_path.display().to_string()),
            comma
        );
    }
    let _ = writeln!(&mut json, "  ]");
    let _ = writeln!(&mut json, "}}");

    fs::write(config.out_dir.join("summary.json"), json).map_err(|e| format!("写入 summary.json 失败: {e}"))
}

fn write_report(
    config: &Config,
    summary: &Summary,
    records: &[CaseRecord],
    ts_empty_records: &[TsEmptyRecord],
) -> Result<(), String> {
    let mut report = String::new();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|dur| dur.as_secs()).unwrap_or(0);
    let _ = writeln!(&mut report, "# TS/Rust Failed Case Report");
    let _ = writeln!(&mut report);
    let _ = writeln!(&mut report, "- generated_at_unix: {now}");
    let _ = writeln!(&mut report, "- library: `{}`", config.library.display());
    let _ = writeln!(&mut report, "- md5_tool: `{}`", config.md5_tool.display());
    let _ = writeln!(&mut report, "- total_generated: {}", summary.total_generated);
    let _ = writeln!(&mut report, "- unique_inputs: {}", summary.unique_inputs);
    let _ = writeln!(&mut report, "- ts_failures: {}", summary.ts_failures);
    let _ = writeln!(&mut report, "- rust_failures: {}", summary.rust_failures);
    let _ = writeln!(&mut report, "- ts_empty_outputs: {}", summary.ts_empty_outputs);
    let _ = writeln!(&mut report, "- diff_failures: {}", summary.diff_failures);
    let _ = writeln!(&mut report, "- deduped_diff_failures: {}", summary.deduped_diff_failures);
    let _ = writeln!(&mut report);
    let _ = writeln!(&mut report, "## Failed Cases");
    let _ = writeln!(&mut report);

    for record in records {
        let _ = writeln!(
            &mut report,
            "- `{}` mode=`{}` mismatch_idx={} ts_lines={} rust_lines={} players={}",
            record.id,
            record.mode,
            record.first_mismatch_idx,
            record.ts_line_count,
            record.rust_line_count,
            record.players.join(", ")
        );
        let _ = writeln!(&mut report, "  input: `{}`", record.input_path.display());
        let _ = writeln!(&mut report, "  ts: `{}`", record.ts_path.display());
        let _ = writeln!(&mut report, "  rust: `{}`", record.rust_path.display());
        let _ = writeln!(&mut report, "  diff: `{}`", record.diff_path.display());
        let _ = writeln!(&mut report, "  meta: `{}`", record.meta_path.display());
    }

    let _ = writeln!(&mut report);
    let _ = writeln!(&mut report, "## TS Empty Output Cases");
    let _ = writeln!(&mut report);

    for record in ts_empty_records {
        let _ = writeln!(
            &mut report,
            "- `{}` mode=`{}` ts_lines={} rust_lines={} players={}",
            record.id,
            record.mode,
            record.ts_line_count,
            record.rust_line_count,
            record.players.join(", ")
        );
        let _ = writeln!(&mut report, "  input: `{}`", record.input_path.display());
        let _ = writeln!(&mut report, "  ts: `{}`", record.ts_path.display());
        let _ = writeln!(&mut report, "  rust: `{}`", record.rust_path.display());
        let _ = writeln!(&mut report, "  meta: `{}`", record.meta_path.display());
    }

    fs::write(config.out_dir.join("report.md"), report).map_err(|e| format!("写入 report.md 失败: {e}"))
}

fn json_escape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 8);
    for ch in raw.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => {
                let _ = write!(&mut out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out
}

fn build_failed_diff(ts_output: &str, rust_output: &str) -> String {
    let ts_lines = split_output_lines(ts_output);
    let rust_lines = split_output_lines(rust_output);
    let mismatch_idx = first_mismatch_idx(&ts_lines, &rust_lines);
    let start = mismatch_idx.saturating_sub(3);
    let end = (mismatch_idx + 4).max(start + 1);

    let mut out = String::new();
    let _ = writeln!(&mut out, "--- expected(ts)");
    let _ = writeln!(&mut out, "+++ actual(rust)");
    let _ = writeln!(&mut out, "@@ mismatch_idx={} @@", mismatch_idx);

    for idx in start..end {
        let ts_line = ts_lines.get(idx);
        let rust_line = rust_lines.get(idx);
        match (ts_line, rust_line) {
            (Some(left), Some(right)) if left == right => {
                let _ = writeln!(&mut out, " {:>4} {}", idx, left);
            }
            (Some(left), Some(right)) => {
                let _ = writeln!(&mut out, "-{:>4} {}", idx, left);
                let _ = writeln!(&mut out, "+{:>4} {}", idx, right);
            }
            (Some(left), None) => {
                let _ = writeln!(&mut out, "-{:>4} {}", idx, left);
                let _ = writeln!(&mut out, "+{:>4} <EOF>", idx);
            }
            (None, Some(right)) => {
                let _ = writeln!(&mut out, "-{:>4} <EOF>", idx);
                let _ = writeln!(&mut out, "+{:>4} {}", idx, right);
            }
            (None, None) => break,
        }
    }
    out
}

fn json_string_array(values: &[String]) -> String {
    let items = values.iter().map(|value| format!("\"{}\"", json_escape(value))).collect::<Vec<String>>();
    format!("[{}]", items.join(", "))
}

fn json_btreemap(values: &BTreeMap<String, usize>) -> String {
    let items = values
        .iter()
        .map(|(key, value)| format!("\"{}\": {}", json_escape(key), value))
        .collect::<Vec<String>>();
    format!("{{{}}}", items.join(", "))
}

#[derive(Default)]
struct TraceNameState {
    assigned: std::collections::HashMap<usize, String>,
    next_index: std::collections::HashMap<usize, usize>,
    /// Blood sacrifice summon name cache, keyed by direct caster entity ID (owner_id).
    /// When the same caster recasts blood sacrifice, the summon reuses the same trace name.
    summon_name: std::collections::HashMap<usize, String>,
}

fn root_trace_owner_id(storage: &tswn_core::engine::storage::Storage, start_id: usize) -> usize {
    use tswn_core::player::skill::act::minion::MinionRuntimeState;

    let mut current = start_id;
    loop {
        let Some(plr) = storage.get_player_or_pending(&current) else {
            return current;
        };
        let Some(minion) = plr.get_state::<MinionRuntimeState>() else {
            return current;
        };
        let Some(owner) = minion.owner else {
            return current;
        };
        current = owner;
    }
}

fn format_trace_minion_name(owner: &tswn_core::player::Player, index: usize) -> String {
    let base = format!("{}?{}", owner.id_name(), index);
    let team = owner.clan_name();
    if !team.is_empty() && team != owner.id_name() {
        format!("{base}@{team}")
    } else {
        base
    }
}

fn alloc_trace_minion_name(trace_names: &mut TraceNameState, root_owner_id: usize, owner: &tswn_core::player::Player) -> String {
    let index = trace_names.next_index.entry(root_owner_id).or_insert(0);
    let name = format_trace_minion_name(owner, *index);
    *index += 1;
    name
}

fn plr_name_raw(runner: &Runner, id: usize, trace_names: &mut TraceNameState) -> String {
    if let Some(name) = trace_names.assigned.get(&id) {
        return name.clone();
    }

    let Some(plr) = runner.storage.get_player_or_pending(&id) else {
        return format!("#{id}");
    };

    use tswn_core::player::PlayerType;
    use tswn_core::player::skill::act::minion::{MinionKind, MinionRuntimeState};

    let name = if plr.player_type() == PlayerType::Boss {
        plr.display_name()
    } else if let Some(minion) = plr.get_state::<MinionRuntimeState>() {
        if let Some(owner_id) = minion.owner {
            let root_owner_id = root_trace_owner_id(&runner.storage, owner_id);
            if let Some(owner) = runner.storage.get_player_or_pending(&root_owner_id) {
                if minion.kind == MinionKind::Summon {
                    // Blood sacrifice summon: cache by direct caster (owner_id).
                    // Same caster recasting reuses the same trace name;
                    // a different caster under the same root owner allocates a new name.
                    if let Some(name) = trace_names.summon_name.get(&owner_id) {
                        name.clone()
                    } else {
                        let name = alloc_trace_minion_name(trace_names, root_owner_id, owner);
                        trace_names.summon_name.insert(owner_id, name.clone());
                        name
                    }
                } else {
                    alloc_trace_minion_name(trace_names, root_owner_id, owner)
                }
            } else {
                plr.id_key_name()
            }
        } else {
            plr.id_key_name()
        }
    } else {
        plr.id_key_name()
    };

    trace_names.assigned.insert(id, name.clone());
    name
}

fn fmt_update_raw_with_state(runner: &Runner, update: &RunUpdate, trace_names: &mut TraceNameState) -> String {
    let caster = plr_name_raw(runner, update.caster, trace_names);
    let mut target = plr_name_raw(runner, update.target, trace_names);
    let targets = if let Some(p) = update.param {
        p.to_string()
    } else if update.targets.is_empty() {
        update.score.to_string()
    } else {
        update
            .targets
            .iter()
            .map(|id| plr_name_raw(runner, *id, trace_names))
            .collect::<Vec<String>>()
            .join(",")
    };

    if update.message == "召唤出幻影" {
        use tswn_core::player::skill::act::minion::{MinionKind, MinionRuntimeState};

        let root_owner_id = root_trace_owner_id(&runner.storage, update.caster);
        let pending_id = runner
            .storage
            .all_player_ids()
            .into_iter()
            .chain(runner.storage.pending_spawn_ids_for_owner(update.caster))
            .find(|id| {
                !trace_names.assigned.contains_key(id)
                    && runner
                        .storage
                        .get_player_or_pending(id)
                        .and_then(|plr| plr.get_state::<MinionRuntimeState>())
                        .map(|state| {
                            state.kind == MinionKind::Shadow
                                && root_trace_owner_id(&runner.storage, state.owner.unwrap_or(*id)) == root_owner_id
                        })
                        .unwrap_or(false)
            });
        if let Some(pending_id) = pending_id {
            target = plr_name_raw(runner, pending_id, trace_names);
        }
        return format!("召唤出{target}");
    }

    let mut msg = update.message.to_string();
    msg = msg.replace("[0]", &caster);
    msg = msg.replace("[1]", &target);
    msg.replace("[2]", &targets)
}

fn sanitize_output_line(line: &str) -> String {
    let filtered = line
        .chars()
        .filter(|ch| !ch.is_control() && !matches!(*ch, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}'))
        .collect::<String>();

    let mut normalized = String::with_capacity(filtered.len());
    let mut prev_space = false;
    for ch in filtered.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                normalized.push(' ');
                prev_space = true;
            }
        } else {
            normalized.push(ch);
            prev_space = false;
        }
    }
    normalized.trim().to_string()
}

fn normalize_trace_line(line: String) -> String {
    let mut normalized = line
        .replace("[s_counter]", "")
        .replace("[s_dmg160]", "")
        .replace("[s_dmg120]", "")
        .replace("[s_dmg0]", "")
        .replace(' ', "")
        .replace('！', "!")
        .replace('？', "?")
        .replace('，', ",")
        .replace('：', ":")
        .replace('；', ";")
        .replace('（', "(")
        .replace('）', ")")
        .replace('²', "2");

    for (from, to) in [
        ("[回避]", "回避"),
        ("[反击]", "反击"),
        ("[吸血攻击]", "吸血攻击"),
        ("[聚气]", "聚气"),
        ("[潜行]", "潜行"),
        ("[背刺]", "背刺"),
        ("[狂暴攻击]", "狂暴攻击"),
        ("[狂暴术]", "狂暴术"),
        ("[狂暴]", "狂暴"),
        ("[蓄力]", "蓄力"),
        ("[隐匿]", "隐匿"),
        ("[魅惑]", "魅惑"),
        ("[防御]", "防御"),
        ("[吞噬]", "吞噬"),
        ("[分身]", "分身"),
        ("[会心一击]", "会心一击"),
        ("[伤害反弹]", "伤害反弹"),
        ("[净化]", "净化"),
        ("[护身符]", "护身符"),
        ("[诅咒]", "诅咒"),
        ("[守护]", "守护"),
        ("[生命之轮]", "生命之轮"),
        ("[垂死]", "垂死"),
        ("[火球术]", "火球术"),
        ("[瘟疫]", "瘟疫"),
        ("[加速术]", "加速术"),
        ("[疾走]", "疾走"),
        ("[治愈魔法]", "治愈魔法"),
        ("[迟缓]", "迟缓"),
        ("[中毒]", "中毒"),
        ("[冰冻术]", "冰冻术"),
        ("[冰冻]", "冰冻"),
        ("[铁壁]", "铁壁"),
        ("[投毒]", "投毒"),
        ("[毒性发作]", "毒性发作"),
        ("[附体]", "附体"),
        ("[地裂术]", "地裂术"),
        ("[连击]", "连击"),
        ("[苏生术]", "苏生术"),
        ("[复活]", "复活"),
        ("[幻术]", "幻术"),
        ("[减速术]", "减速术"),
        ("[雷击术]", "雷击术"),
        ("[血祭]", "血祭"),
        ("[召唤亡灵]", "召唤亡灵"),
        ("[自爆]", "自爆"),
    ] {
        normalized = normalized.replace(from, to);
    }

    sanitize_output_line(&normalized)
}

fn is_action_line(line: &str) -> bool {
    line.contains("发起攻击")
        || (line.contains("使用") && !line.contains("护身符抵挡了一次死亡"))
        || line.contains("做出垂死抗争")
        || line.contains("连击")
        || line.contains("从疾走中解除")
}

fn emit_current_turn(output_lines: &mut Vec<String>, pending_action_line: &mut String, pending_misc_lines: &mut Vec<String>) {
    if !pending_action_line.is_empty() {
        output_lines.push(std::mem::take(pending_action_line));
        output_lines.push(String::new());
        pending_misc_lines.clear();
        return;
    }
    if !pending_misc_lines.is_empty() {
        output_lines.push(pending_misc_lines.join(", "));
        output_lines.push(String::new());
        pending_misc_lines.clear();
    }
}

fn collect_fight_raw_lines(runner: &mut Runner) -> Vec<String> {
    let mut output_lines = Vec::new();
    let mut pending_action_line = String::new();
    let mut pending_misc_lines = Vec::new();
    let mut trace_names = TraceNameState::default();

    let mut round = 1usize;
    let mut idle_rounds = 0usize;
    while !runner.have_winner() && round <= 100_000 {
        let updates = runner.main_round();
        if updates.updates.is_empty() {
            idle_rounds += 1;
            if idle_rounds > 16 {
                break;
            }
            continue;
        }
        idle_rounds = 0;

        for update in updates.updates {
            if matches!(update.update_type, UpdateType::NextLine) {
                emit_current_turn(&mut output_lines, &mut pending_action_line, &mut pending_misc_lines);
                continue;
            }

            let line = normalize_trace_line(fmt_update_raw_with_state(runner, &update, &mut trace_names));
            if line.is_empty() {
                continue;
            }

            if is_action_line(&line) {
                emit_current_turn(&mut output_lines, &mut pending_action_line, &mut pending_misc_lines);
                pending_action_line = line;
                continue;
            }

            if pending_action_line.is_empty() {
                pending_misc_lines.push(line);
            } else {
                pending_action_line.push_str(", ");
                pending_action_line.push_str(&line);
            }
        }
        round += 1;
    }

    emit_current_turn(&mut output_lines, &mut pending_action_line, &mut pending_misc_lines);
    while matches!(output_lines.last(), Some(line) if line.is_empty()) {
        output_lines.pop();
    }
    output_lines
}

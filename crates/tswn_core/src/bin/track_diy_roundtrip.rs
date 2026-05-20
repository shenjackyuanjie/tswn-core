use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tswn_core::case_gen::{CaseMode, case_id, deterministic_shuffle, generate_cases_for_mode, load_library};
use tswn_core::engine::storage::Storage;
use tswn_core::engine::update::{RunUpdate, UpdateType};
use tswn_core::player::Player;
use tswn_core::{Runner, engine};

#[derive(Debug)]
struct Config {
    cases_dir: Option<PathBuf>,
    library: PathBuf,
    out_dir: PathBuf,
    modes: Vec<CaseMode>,
    case_offset_per_mode: usize,
    max_cases_per_mode: usize,
    shuffle_seed: u64,
    skip_fight: bool,
    max_rounds: usize,
    keep_going: bool,
    quiet: bool,
}

#[derive(Clone, Debug)]
struct TrackCase {
    id: String,
    mode: String,
    players: Vec<String>,
    input: String,
}

#[derive(Clone, Debug)]
struct PlayerStatus {
    name: String,
    id: usize,
    hp: i64,
    max_hp: i64,
    move_point: i64,
    atk: i64,
    def: i64,
    spd: i64,
    agi: i64,
    mag: i64,
    mp: i64,
    mdf: i64,
    itl: i64,
    all_sum: i64,
    name_factor: f64,
}

#[derive(Debug)]
struct CaseResult {
    case_id: String,
    mode: String,
    success: bool,
    error: Option<String>,
    warnings: Vec<String>,
    diffs: Vec<String>,
    player_count: usize,
}

#[derive(Default)]
struct Summary {
    total: usize,
    passed: usize,
    failed: usize,
    errors: usize,
    warnings: usize,
    per_mode_total: BTreeMap<String, usize>,
    per_mode_failed: BTreeMap<String, usize>,
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

    let cases = load_cases(&config)?;
    if cases.is_empty() {
        return Err("没有可用的 case".to_string());
    }

    println!("共 {} 个 case 待测试", cases.len());
    let mut summary = Summary::default();
    let mut results = Vec::new();

    for (idx, case) in cases.iter().enumerate() {
        if !config.quiet {
            println!("\n[{}/{}] {} mode={}", idx + 1, cases.len(), case.id, case.mode);
        }

        let result = run_case(case, &config)?;
        summary.total += 1;
        *summary.per_mode_total.entry(result.mode.clone()).or_insert(0) += 1;
        summary.warnings += result.warnings.len();

        if let Some(err) = &result.error {
            summary.errors += 1;
            if !config.quiet {
                println!("  错误: {err}");
            }
            if !config.keep_going {
                results.push(result);
                break;
            }
        } else if result.success {
            summary.passed += 1;
            if !config.quiet {
                println!("  通过 ({} 玩家一致)", result.player_count);
            }
        } else {
            summary.failed += 1;
            *summary.per_mode_failed.entry(result.mode.clone()).or_insert(0) += 1;
            if !config.quiet {
                println!("  失败 ({} 处差异)", result.diffs.len());
                for diff in result.diffs.iter().take(6) {
                    println!("{diff}");
                }
            }
            if !config.keep_going {
                results.push(result);
                break;
            }
        }
        results.push(result);
    }

    write_summary(&config, &summary, &results)?;

    println!("\n==================================================");
    println!(
        "总计: {} | 通过: {} | 失败: {} | 错误: {} | 警告: {}",
        summary.total, summary.passed, summary.failed, summary.errors, summary.warnings
    );
    println!("Summary: {}", config.out_dir.join("summary.json").display());

    if summary.failed == 0 && summary.errors == 0 {
        Ok(())
    } else {
        Err("存在失败或错误 case".to_string())
    }
}

fn parse_args() -> Result<Config, String> {
    let mut cases_dir = None;
    let mut library = None;
    let mut out_dir = PathBuf::from("target").join("diy_roundtrip");
    let mut modes: Option<Vec<CaseMode>> = None;
    let mut include_ffa_from_modes = false;
    let mut ffa_sizes = vec![4usize, 6, 8];
    let mut case_offset_per_mode = 0usize;
    let mut max_cases_per_mode = 64usize;
    let mut shuffle_seed = 0x5EED_2026_u64;
    let mut skip_fight = false;
    let mut max_rounds = 100_000usize;
    let mut keep_going = false;
    let mut quiet = false;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            "--cases-dir" => {
                idx += 1;
                cases_dir = Some(PathBuf::from(require_arg(&args, idx, "--cases-dir")?));
            }
            "--library" => {
                idx += 1;
                library = Some(PathBuf::from(require_arg(&args, idx, "--library")?));
            }
            "--out-dir" => {
                idx += 1;
                out_dir = PathBuf::from(require_arg(&args, idx, "--out-dir")?);
            }
            "--mode" => {
                idx += 1;
                let mode = parse_single_mode(require_arg(&args, idx, "--mode")?)?;
                modes = Some(vec![mode]);
                include_ffa_from_modes = false;
            }
            "--modes" => {
                idx += 1;
                let (parsed, include_ffa) = parse_mode_names(require_arg(&args, idx, "--modes")?)?;
                modes = Some(parsed);
                include_ffa_from_modes = include_ffa;
            }
            "--ffa-sizes" => {
                idx += 1;
                ffa_sizes = parse_usize_csv(require_arg(&args, idx, "--ffa-sizes")?, "--ffa-sizes")?;
            }
            "--case-offset" | "--case-offset-per-mode" => {
                idx += 1;
                case_offset_per_mode = require_arg(&args, idx, args[idx - 1].as_str())?
                    .parse::<usize>()
                    .map_err(|e| format!("解析 {} 失败: {e}", args[idx - 1]))?;
            }
            "--max-cases" | "--max-cases-per-mode" => {
                idx += 1;
                max_cases_per_mode = require_arg(&args, idx, args[idx - 1].as_str())?
                    .parse::<usize>()
                    .map_err(|e| format!("解析 {} 失败: {e}", args[idx - 1]))?;
                if max_cases_per_mode == 0 {
                    return Err(format!("{} 必须大于 0", args[idx - 1]));
                }
            }
            "--shuffle-seed" => {
                idx += 1;
                shuffle_seed = require_arg(&args, idx, "--shuffle-seed")?
                    .parse::<u64>()
                    .map_err(|e| format!("解析 --shuffle-seed 失败: {e}"))?;
            }
            "--skip-fight" => skip_fight = true,
            "--fight-timeout" => {
                idx += 1;
                let seconds = require_arg(&args, idx, "--fight-timeout")?
                    .parse::<usize>()
                    .map_err(|e| format!("解析 --fight-timeout 失败: {e}"))?;
                max_rounds = seconds.saturating_mul(2_000).max(1);
            }
            "--max-rounds" => {
                idx += 1;
                max_rounds = require_arg(&args, idx, "--max-rounds")?
                    .parse::<usize>()
                    .map_err(|e| format!("解析 --max-rounds 失败: {e}"))?;
                if max_rounds == 0 {
                    return Err("--max-rounds 必须大于 0".to_string());
                }
            }
            "--keep-going" => keep_going = true,
            "--quiet" | "-q" => quiet = true,
            other => return Err(format!("未知参数: {other}")),
        }
        idx += 1;
    }

    let mut modes = modes.unwrap_or_else(|| vec![CaseMode::OneVsOne, CaseMode::TwoVsTwo, CaseMode::ThreeVsThreeVsThree]);
    let include_ffa = if args.iter().any(|arg| arg == "--modes") {
        include_ffa_from_modes
    } else if args.iter().any(|arg| arg == "--mode") {
        false
    } else {
        true
    };
    if include_ffa {
        modes.extend(ffa_sizes.iter().copied().map(CaseMode::FreeForAll));
    }

    Ok(Config {
        cases_dir,
        library: library.unwrap_or_else(|| PathBuf::from("tests").join("sqp6000.txt")),
        out_dir,
        modes,
        case_offset_per_mode,
        max_cases_per_mode,
        shuffle_seed,
        skip_fight,
        max_rounds,
        keep_going,
        quiet,
    })
}

fn print_usage() {
    println!(
        r#"用法:
    track_diy_roundtrip [选项]

选项:
  --cases-dir <dir>          从已有 case 目录读取 input.txt
  --library <path>           号库文件，默认 tests/sqp6000.txt
  --out-dir <path>           失败输出目录，默认 target/diy_roundtrip
  --modes <csv>              生成模式，默认 1v1,2v2,3v3v3,ffa
  --mode <mode>              兼容旧脚本，单一模式: 1v1/2v2/3v3v3/ffa
  --ffa-sizes <csv>          ffa 人数，默认 4,6,8
  --case-offset-per-mode <N> 每种模式跳过前 N 个 case，默认 0
  --max-cases-per-mode <N>   每种模式最多生成多少 case，默认 64
  --case-offset <N>          兼容旧脚本，等同 --case-offset-per-mode
  --max-cases <N>            兼容旧脚本，等同 --max-cases-per-mode
  --shuffle-seed <N>         固定采样顺序，默认 1592597030
  --skip-fight               只比对初始状态，跳过完整战斗日志
  --fight-timeout <seconds>  兼容旧脚本，换算成 max rounds
  --max-rounds <N>           战斗比对最大轮次，默认 100000
  --keep-going               遇到失败/错误继续
  -q, --quiet                安静模式
"#
    );
}

fn require_arg<'a>(args: &'a [String], idx: usize, flag: &str) -> Result<&'a str, String> {
    args.get(idx).map(String::as_str).ok_or_else(|| format!("{flag} 缺少参数"))
}

fn parse_single_mode(raw: &str) -> Result<CaseMode, String> {
    match raw {
        "1v1" => Ok(CaseMode::OneVsOne),
        "2v2" => Ok(CaseMode::TwoVsTwo),
        "3v3v3" => Ok(CaseMode::ThreeVsThreeVsThree),
        "ffa" => Ok(CaseMode::FreeForAll(6)),
        other => Err(format!("未知模式: {other}")),
    }
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

fn load_cases(config: &Config) -> Result<Vec<TrackCase>, String> {
    if let Some(cases_dir) = &config.cases_dir {
        return load_cases_dir(cases_dir);
    }

    let mut names = load_library(&config.library)?;
    deterministic_shuffle(&mut names, config.shuffle_seed);

    let mut cases = Vec::new();
    let mut seen_inputs = HashSet::new();
    for mode in &config.modes {
        for case in generate_cases_for_mode(&names, *mode, config.case_offset_per_mode, config.max_cases_per_mode) {
            if seen_inputs.insert(case.input_hash) {
                cases.push(TrackCase {
                    id: case_id(case.mode, case.input_hash),
                    mode: case.mode.label(),
                    players: case.players,
                    input: case.input,
                });
            }
        }
    }
    Ok(cases)
}

fn load_cases_dir(cases_dir: &Path) -> Result<Vec<TrackCase>, String> {
    if !cases_dir.is_dir() {
        return Err(format!("case 目录不存在: {}", cases_dir.display()));
    }

    let mut cases = Vec::new();
    let mut dirs = fs::read_dir(cases_dir)
        .map_err(|e| format!("读取 case 目录失败: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取 case 目录失败: {e}"))?;
    dirs.sort_by_key(|entry| entry.file_name());

    for entry in dirs {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let input_path = path.join("input.txt");
        if !input_path.is_file() {
            continue;
        }
        let input = fs::read_to_string(&input_path).map_err(|e| format!("读取 {} 失败: {e}", input_path.display()))?;
        let id = entry.file_name().to_string_lossy().to_string();
        cases.push(TrackCase {
            id,
            mode: "existing".to_string(),
            players: parse_players_from_input(&input).into_iter().flatten().collect(),
            input,
        });
    }
    Ok(cases)
}

fn run_case(case: &TrackCase, config: &Config) -> Result<CaseResult, String> {
    let mut result = CaseResult {
        case_id: case.id.clone(),
        mode: case.mode.clone(),
        success: false,
        error: None,
        warnings: Vec::new(),
        diffs: Vec::new(),
        player_count: 0,
    };

    let (diy_input, _name_map, warnings) = build_diy_input(&case.input);
    result.warnings = warnings;

    let orig_statuses = match get_player_statuses(&case.input) {
        Ok(statuses) => statuses,
        Err(err) => {
            result.error = Some(format!("原始对局构建失败: {err}"));
            write_failure_files(&config.out_dir, case, &diy_input, &[], &[], &[], None)?;
            return Ok(result);
        }
    };
    let diy_statuses = match get_player_statuses(&diy_input) {
        Ok(statuses) => statuses,
        Err(err) => {
            result.error = Some(format!("DIY 对局构建失败: {err}"));
            write_failure_files(&config.out_dir, case, &diy_input, &orig_statuses, &[], &[], None)?;
            return Ok(result);
        }
    };

    let mut diffs = compare_statuses(&orig_statuses, &diy_statuses);
    let mut fight_detail = None;
    if !config.skip_fight {
        let orig_lines = collect_fight_lines(&case.input, config.max_rounds)?;
        let diy_lines = collect_fight_lines(&diy_input, config.max_rounds)?;
        let (fight_diffs, detail) = compare_fight_lines(&orig_lines, &diy_lines, 2);
        if !fight_diffs.is_empty() {
            diffs.extend(fight_diffs);
            fight_detail = Some(FightDetail {
                orig_lines,
                diy_lines,
                diff_lines: detail,
            });
        }
    }

    result.player_count = orig_statuses.len();
    result.success = diffs.is_empty() && result.error.is_none();
    result.diffs = diffs;

    if !result.success || result.error.is_some() {
        write_failure_files(
            &config.out_dir,
            case,
            &diy_input,
            &orig_statuses,
            &diy_statuses,
            &result.diffs,
            fight_detail.as_ref(),
        )?;
    }

    Ok(result)
}

fn parse_players_from_input(input_text: &str) -> Vec<Vec<String>> {
    let mut groups = Vec::new();
    let mut current = Vec::new();
    for line in input_text.trim().lines() {
        let line = line.trim();
        if line.is_empty() {
            if !current.is_empty() {
                groups.push(std::mem::take(&mut current));
            }
        } else {
            current.push(line.to_string());
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

fn build_diy_input(original_input: &str) -> (String, HashMap<String, String>, Vec<String>) {
    let groups = parse_players_from_input(original_input);
    let mut name_map = HashMap::new();
    let mut warnings = Vec::new();
    let storage = Storage::new_arc();

    let diy_groups = groups
        .iter()
        .map(|group| {
            group
                .iter()
                .map(|player_raw| match get_diy_name(player_raw, storage.clone()) {
                    Ok(diy_name) => {
                        name_map.insert(player_raw.clone(), diy_name.clone());
                        diy_name
                    }
                    Err(err) => {
                        warnings.push(format!("无法转换: {} ({err})", truncate(player_raw, 80)));
                        player_raw.clone()
                    }
                })
                .collect::<Vec<String>>()
        })
        .collect::<Vec<Vec<String>>>();

    let diy_input = diy_groups.iter().map(|group| group.join("\n")).collect::<Vec<String>>().join("\n\n");
    (diy_input, name_map, warnings)
}

fn get_diy_name(player_raw: &str, storage: std::sync::Arc<Storage>) -> Result<String, String> {
    let mut player = Player::new_from_namerena_raw(player_raw.to_string(), storage).map_err(|e| e.to_string())?;
    player.build();
    Ok(player.to_ol_json())
}

fn get_player_statuses(input_text: &str) -> Result<Vec<PlayerStatus>, String> {
    let runner = Runner::new_from_namerena_raw(input_text.to_string()).map_err(|e| e.to_string())?;
    let mut statuses = Vec::new();
    for id in runner.storage.all_player_ids() {
        if let Some(plr) = runner.storage.get_player(&id) {
            let status = plr.get_status();
            statuses.push(PlayerStatus {
                name: plr.display_name(),
                id,
                hp: status.hp as i64,
                max_hp: status.max_hp as i64,
                move_point: status.move_point as i64,
                atk: status.attack as i64,
                def: status.defense as i64,
                spd: status.speed as i64,
                agi: status.agility as i64,
                mag: status.magic as i64,
                mp: status.magic_point as i64,
                mdf: status.resistance as i64,
                itl: status.wisdom as i64,
                all_sum: status.all_sum as i64,
                name_factor: plr.get_name_factor(),
            });
        }
    }
    Ok(statuses)
}

fn compare_statuses(orig_statuses: &[PlayerStatus], diy_statuses: &[PlayerStatus]) -> Vec<String> {
    let mut diffs = Vec::new();
    if orig_statuses.len() != diy_statuses.len() {
        diffs.push(format!(
            "玩家数量不同: orig={}, diy={}",
            orig_statuses.len(),
            diy_statuses.len()
        ));
        return diffs;
    }

    let orig_by_id = index_by_id(orig_statuses);
    let diy_by_id = index_by_id(diy_statuses);
    if orig_by_id.is_none() || diy_by_id.is_none() {
        diffs.push("玩家 id 缺失或重复，无法按 id 对齐".to_string());
        return diffs;
    }
    let orig_by_id = orig_by_id.unwrap();
    let diy_by_id = diy_by_id.unwrap();

    let orig_ids = orig_by_id.keys().copied().collect::<Vec<usize>>();
    let diy_ids = diy_by_id.keys().copied().collect::<Vec<usize>>();
    if orig_ids != diy_ids {
        diffs.push(format!("玩家 id 集合不同: orig={orig_ids:?}, diy={diy_ids:?}"));
        return diffs;
    }

    for id in orig_ids {
        let orig = orig_by_id.get(&id).unwrap();
        let diy = diy_by_id.get(&id).unwrap();
        compare_i64(&mut diffs, id, "hp", orig.hp, diy.hp);
        compare_i64(&mut diffs, id, "max_hp", orig.max_hp, diy.max_hp);
        compare_i64(&mut diffs, id, "atk", orig.atk, diy.atk);
        compare_i64(&mut diffs, id, "def", orig.def, diy.def);
        compare_i64(&mut diffs, id, "spd", orig.spd, diy.spd);
        compare_i64(&mut diffs, id, "agi", orig.agi, diy.agi);
        compare_i64(&mut diffs, id, "mag", orig.mag, diy.mag);
        compare_i64(&mut diffs, id, "mdf", orig.mdf, diy.mdf);
        compare_i64(&mut diffs, id, "itl", orig.itl, diy.itl);
        compare_i64(&mut diffs, id, "all_sum", orig.all_sum, diy.all_sum);
        if (orig.name_factor - diy.name_factor).abs() > 0.001 {
            diffs.push(format!(
                "  player[id={id}] name_factor: orig={:.6}, diy={:.6}",
                orig.name_factor, diy.name_factor
            ));
        }
    }

    diffs
}

fn index_by_id(statuses: &[PlayerStatus]) -> Option<BTreeMap<usize, &PlayerStatus>> {
    let mut indexed = BTreeMap::new();
    for status in statuses {
        if indexed.insert(status.id, status).is_some() {
            return None;
        }
    }
    Some(indexed)
}

fn compare_i64(diffs: &mut Vec<String>, id: usize, field: &str, orig: i64, diy: i64) {
    if orig != diy {
        diffs.push(format!("  player[id={id}] {field}: orig={orig}, diy={diy}"));
    }
}

fn collect_fight_lines(input_text: &str, max_rounds: usize) -> Result<Vec<String>, String> {
    let mut runner = Runner::new_from_namerena_raw(input_text.to_string()).map_err(|e| e.to_string())?;
    let input_player_ids = runner.input_groups.iter().flat_map(|group| group.iter().copied()).collect::<Vec<usize>>();
    let mut output_lines = collect_fight_raw_lines(&mut runner, max_rounds);
    if let Some(win_idx_line) = fmt_winner_input_indices(&runner, &input_player_ids) {
        output_lines.push(win_idx_line);
    }
    Ok(output_lines)
}

fn compare_fight_lines(orig_lines: &[String], diy_lines: &[String], context: usize) -> (Vec<String>, Vec<String>) {
    if orig_lines == diy_lines {
        return (Vec::new(), Vec::new());
    }

    let max_len = orig_lines.len().max(diy_lines.len());
    let mut mismatch_idx = 0usize;
    for idx in 0..max_len {
        let orig = orig_lines.get(idx);
        let diy = diy_lines.get(idx);
        if orig != diy {
            mismatch_idx = idx;
            break;
        }
    }

    let summary = vec![format!(
        "  fight: mismatch at line {mismatch_idx} (orig_lines={}, diy_lines={})",
        orig_lines.len(),
        diy_lines.len()
    )];

    let mut detail_lines = vec![
        format!("orig_lines={}", orig_lines.len()),
        format!("diy_lines={}", diy_lines.len()),
        format!("first_mismatch={mismatch_idx}"),
        String::new(),
    ];
    let start = mismatch_idx.saturating_sub(context);
    let end = max_len.min(mismatch_idx + context + 1);
    for idx in start..end {
        let orig = orig_lines.get(idx).map(String::as_str).unwrap_or("<EOF>");
        let diy = diy_lines.get(idx).map(String::as_str).unwrap_or("<EOF>");
        let prefix = if idx == mismatch_idx { ">>" } else { "  " };
        detail_lines.push(format!("{prefix} [{idx}] orig: {orig}"));
        detail_lines.push(format!("{prefix} [{idx}] diy : {diy}"));
    }

    (summary, detail_lines)
}

struct FightDetail {
    orig_lines: Vec<String>,
    diy_lines: Vec<String>,
    diff_lines: Vec<String>,
}

fn write_failure_files(
    out_dir: &Path,
    case: &TrackCase,
    diy_input: &str,
    orig_statuses: &[PlayerStatus],
    diy_statuses: &[PlayerStatus],
    diffs: &[String],
    fight_detail: Option<&FightDetail>,
) -> Result<(), String> {
    let case_dir = out_dir.join("failed").join(sanitize_path_segment(&case.id));
    fs::create_dir_all(&case_dir).map_err(|e| format!("创建失败 case 目录失败: {e}"))?;
    fs::write(case_dir.join("input_orig.txt"), &case.input).map_err(|e| format!("写入 input_orig.txt 失败: {e}"))?;
    fs::write(case_dir.join("input_diy.txt"), diy_input).map_err(|e| format!("写入 input_diy.txt 失败: {e}"))?;
    fs::write(case_dir.join("players.json"), json_string_array(&case.players))
        .map_err(|e| format!("写入 players.json 失败: {e}"))?;
    fs::write(case_dir.join("status_orig.json"), statuses_json(orig_statuses))
        .map_err(|e| format!("写入 status_orig.json 失败: {e}"))?;
    fs::write(case_dir.join("status_diy.json"), statuses_json(diy_statuses))
        .map_err(|e| format!("写入 status_diy.json 失败: {e}"))?;
    if !diffs.is_empty() {
        fs::write(case_dir.join("diff.txt"), diffs.join("\n")).map_err(|e| format!("写入 diff.txt 失败: {e}"))?;
    }
    if let Some(detail) = fight_detail {
        fs::write(case_dir.join("fight_orig.txt"), detail.orig_lines.join("\n"))
            .map_err(|e| format!("写入 fight_orig.txt 失败: {e}"))?;
        fs::write(case_dir.join("fight_diy.txt"), detail.diy_lines.join("\n"))
            .map_err(|e| format!("写入 fight_diy.txt 失败: {e}"))?;
        fs::write(case_dir.join("fight_diff.txt"), detail.diff_lines.join("\n"))
            .map_err(|e| format!("写入 fight_diff.txt 失败: {e}"))?;
    }
    Ok(())
}

fn write_summary(config: &Config, summary: &Summary, results: &[CaseResult]) -> Result<(), String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|dur| dur.as_secs()).unwrap_or(0);
    let failed_cases = results
        .iter()
        .filter(|r| !r.success && r.error.is_none())
        .map(|r| {
            format!(
                "{{\"case_id\":\"{}\",\"mode\":\"{}\",\"diffs\":{}}}",
                json_escape(&r.case_id),
                json_escape(&r.mode),
                json_string_array(&r.diffs)
            )
        })
        .collect::<Vec<String>>();
    let error_cases = results
        .iter()
        .filter_map(|r| {
            r.error.as_ref().map(|err| {
                format!(
                    "{{\"case_id\":\"{}\",\"mode\":\"{}\",\"error\":\"{}\"}}",
                    json_escape(&r.case_id),
                    json_escape(&r.mode),
                    json_escape(err)
                )
            })
        })
        .collect::<Vec<String>>();

    let mut json = String::new();
    let _ = writeln!(&mut json, "{{");
    let _ = writeln!(&mut json, "  \"generated_at_unix\": {now},");
    let _ = writeln!(
        &mut json,
        "  \"library\": \"{}\",",
        json_escape(&config.library.display().to_string())
    );
    let _ = writeln!(
        &mut json,
        "  \"out_dir\": \"{}\",",
        json_escape(&config.out_dir.display().to_string())
    );
    let _ = writeln!(
        &mut json,
        "  \"results\": {{\"total\": {}, \"passed\": {}, \"failed\": {}, \"errors\": {}, \"warnings\": {}}},",
        summary.total, summary.passed, summary.failed, summary.errors, summary.warnings
    );
    let _ = writeln!(&mut json, "  \"per_mode_total\": {},", json_btreemap(&summary.per_mode_total));
    let _ = writeln!(&mut json, "  \"per_mode_failed\": {},", json_btreemap(&summary.per_mode_failed));
    let _ = writeln!(&mut json, "  \"failed_cases\": [{}],", failed_cases.join(","));
    let _ = writeln!(&mut json, "  \"error_cases\": [{}]", error_cases.join(","));
    let _ = writeln!(&mut json, "}}");

    fs::write(config.out_dir.join("summary.json"), json).map_err(|e| format!("写入 summary.json 失败: {e}"))
}

fn statuses_json(statuses: &[PlayerStatus]) -> String {
    let mut json = String::new();
    json.push('[');
    for (idx, status) in statuses.iter().enumerate() {
        if idx > 0 {
            json.push(',');
        }
        let _ = write!(
            &mut json,
            "{{\"name\":\"{}\",\"id\":{},\"hp\":{},\"max_hp\":{},\"move_point\":{},\"atk\":{},\"def\":{},\"spd\":{},\"agi\":{},\"mag\":{},\"mp\":{},\"mdf\":{},\"itl\":{},\"all_sum\":{},\"name_factor\":{:.6}}}",
            json_escape(&status.name),
            status.id,
            status.hp,
            status.max_hp,
            status.move_point,
            status.atk,
            status.def,
            status.spd,
            status.agi,
            status.mag,
            status.mp,
            status.mdf,
            status.itl,
            status.all_sum,
            status.name_factor
        );
    }
    json.push(']');
    json
}

fn json_string_array(values: &[String]) -> String {
    let items = values.iter().map(|value| format!("\"{}\"", json_escape(value))).collect::<Vec<String>>();
    format!("[{}]", items.join(","))
}

fn json_btreemap(values: &BTreeMap<String, usize>) -> String {
    let items = values
        .iter()
        .map(|(key, value)| format!("\"{}\":{}", json_escape(key), value))
        .collect::<Vec<String>>();
    format!("{{{}}}", items.join(","))
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

fn sanitize_path_segment(raw: &str) -> String {
    raw.chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect()
}

fn truncate(raw: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in raw.chars().enumerate() {
        if idx >= max_chars {
            out.push_str("...");
            return out;
        }
        out.push(ch);
    }
    out
}

#[derive(Default)]
struct TraceNameState {
    assigned: HashMap<usize, String>,
    next_index: HashMap<usize, usize>,
    summon_name: HashMap<usize, String>,
}

fn root_trace_owner_id(storage: &engine::storage::Storage, start_id: usize) -> usize {
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

fn format_trace_minion_name(owner: &Player, index: usize) -> String {
    let base = format!("{}?{}", owner.id_name(), index);
    let team = owner.clan_name();
    if !team.is_empty() && team != owner.id_name() {
        format!("{base}@{team}")
    } else {
        base
    }
}

fn alloc_trace_minion_name(trace_names: &mut TraceNameState, root_owner_id: usize, owner: &Player) -> String {
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

fn collect_fight_raw_lines(runner: &mut Runner, max_rounds: usize) -> Vec<String> {
    let mut output_lines = Vec::new();
    let mut pending_action_line = String::new();
    let mut pending_misc_lines = Vec::new();
    let mut trace_names = TraceNameState::default();

    let mut round = 1usize;
    let mut idle_rounds = 0usize;
    while !runner.have_winner() && round <= max_rounds {
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

fn fmt_winner_input_indices(runner: &Runner, input_player_ids: &[usize]) -> Option<String> {
    let winners = runner.world.winner.as_ref()?;
    let indices = winners
        .iter()
        .filter_map(|winner| input_player_ids.iter().position(|id| id == winner))
        .map(|idx| idx.to_string())
        .collect::<Vec<String>>();
    if indices.is_empty() {
        None
    } else {
        Some(format!("win_idx={}", indices.join(",")))
    }
}

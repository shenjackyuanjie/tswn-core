use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const DEFAULT_FILTER: &str = "large large_full small_seed fight_multi";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct TestRecord {
    status: String,
    idx: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Checkpoint {
    name: String,
    time: String,
    records: BTreeMap<String, TestRecord>,
}

#[derive(Debug)]
struct Config {
    filter: String,
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
    test: String,
    kind: &'static str,
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
    let config = parse_args()?;
    let paths = Paths::new();

    if let Some(command) = &config.command {
        run_command(command, &paths)?;
        return Ok(());
    }

    if !config.quiet {
        println!("========================================");
        println!("  测试回归追踪工具");
        println!("========================================");
        println!();
    }

    let previous_records = load_records(&paths.record_file);

    if config.reset {
        if paths.record_file.exists() {
            fs::remove_file(&paths.record_file).map_err(|e| format!("删除历史记录失败: {e}"))?;
        }
        save_records(&paths.record_file, &BTreeMap::new())?;
        println!("重置模式：清除历史记录");
        return Ok(());
    }

    if config.show {
        println!("当前失败状态:");
        for (test, info) in &previous_records {
            if info.status == "FAILED" {
                println!("  {test} => idx={}", info.idx);
            }
        }
        return Ok(());
    }

    if config.quiet {
        println!("[track_test] 运行测试: {}", config.filter);
    } else {
        println!("运行测试: {}", config.filter);
        println!();
    }

    let output = run_cargo_test(&config.filter)?;
    let current_records = parse_cargo_test_output(&output);
    let has_failure = current_records.values().any(|record| record.status == "FAILED" && record.idx >= 0);

    if !has_failure {
        if !config.quiet {
            println!("所有测试通过！");
            let passing_tests = current_records
                .iter()
                .filter(|(_, record)| record.status != "FAILED")
                .map(|(name, _)| name)
                .collect::<Vec<_>>();
            if !passing_tests.is_empty() {
                println!();
                println!("通过的测试:");
                for test in passing_tests {
                    println!("  - {test}");
                }
            }
        } else {
            println!("所有测试通过！");
        }
        print_checkpoint_comparison(&paths, &BTreeMap::new(), config.quiet)?;
        save_records(&paths.record_file, &BTreeMap::new())?;
        write_log(&paths.log_file, "所有测试通过")?;
        return Ok(());
    }

    if config.quiet {
        println!("测试失败，分析中...");
    } else {
        println!("测试失败，分析中...");
        println!();
    }

    println!("--- vs 上次运行 ---");
    let changes = compare_records(&current_records, &previous_records);
    let (improved_count, regressed_count, new_fail_count, fixed_count) = print_changes_summary(&changes, config.quiet);

    if !config.quiet {
        println!();
        println!("========================================");
        println!("  汇总");
        println!("========================================");
        println!("改进: {improved_count}");
        println!("退步: {regressed_count}");
        println!("新失败: {new_fail_count}");
        println!("修复: {fixed_count}");
    }

    println!();
    print_conclusion(&changes, false);
    print_checkpoint_comparison(&paths, &current_records, config.quiet)?;

    save_records(&paths.record_file, &current_records)?;
    write_log(
        &paths.log_file,
        &format!("改进:{improved_count}, 退步:{regressed_count}, 新失败:{new_fail_count}, 修复:{fixed_count}"),
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
            record_file: PathBuf::from("target").join("test_regression.json"),
            log_file: PathBuf::from("target").join("test_regression.log"),
            checkpoint_dir: PathBuf::from("target").join("test_checkpoints"),
        }
    }
}

fn parse_args() -> Result<Config, String> {
    let args = std::env::args().skip(1).collect::<Vec<String>>();
    let mut filter = DEFAULT_FILTER.to_string();
    let mut show = false;
    let mut reset = false;
    let mut quiet = false;
    let mut command = None;
    let mut idx = 0usize;

    while idx < args.len() {
        match args[idx].as_str() {
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            "-f" | "--filter" => {
                idx += 1;
                filter = args.get(idx).cloned().ok_or_else(|| "--filter 缺少参数".to_string())?;
            }
            "-s" | "--show" => show = true,
            "-r" | "--reset" => reset = true,
            "-q" | "--quiet" => quiet = true,
            "save" => {
                let name = args.get(idx + 1).cloned();
                command = Some(TrackCommand::Save { name });
                break;
            }
            "list" => {
                command = Some(TrackCommand::List);
                break;
            }
            "diff" => {
                let name = args.get(idx + 1).cloned();
                command = Some(TrackCommand::Diff { name });
                break;
            }
            "delete" => {
                let name = args.get(idx + 1).cloned().ok_or_else(|| "delete 缺少存档点名称".to_string())?;
                command = Some(TrackCommand::Delete { name });
                break;
            }
            other => return Err(format!("未知参数: {other}")),
        }
        idx += 1;
    }

    Ok(Config {
        filter,
        show,
        reset,
        quiet,
        command,
    })
}

fn print_usage() {
    println!(
        r#"用法:
  track_test [选项]
  track_test save [name]
  track_test list
  track_test diff [name]
  track_test delete <name>

选项:
  -f, --filter <expr>  测试过滤表达式，默认: large large_full small_seed fight_multi
  -s, --show           只显示当前失败状态，不运行测试
  -r, --reset          重置历史记录
  -q, --quiet          安静模式
"#
    );
}

fn run_command(command: &TrackCommand, paths: &Paths) -> Result<(), String> {
    match command {
        TrackCommand::Save { name } => cmd_save(paths, name.clone()),
        TrackCommand::List => cmd_list(paths),
        TrackCommand::Diff { name } => cmd_diff(paths, name.as_deref()),
        TrackCommand::Delete { name } => cmd_delete(paths, name),
    }
}

fn run_cargo_test(filter: &str) -> Result<String, String> {
    let mut cmd = Command::new("cargo");
    cmd.arg("test").arg("--");
    for arg in filter.split_whitespace() {
        cmd.arg(arg);
    }
    let output = cmd.output().map_err(|e| format!("运行 cargo test 失败: {e}"))?;
    let mut text = String::new();
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    text.push('\n');
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok(text)
}

fn parse_cargo_test_output(output: &str) -> BTreeMap<String, TestRecord> {
    let lines = output.lines().collect::<Vec<&str>>();
    let mut results = BTreeMap::new();

    for line in &lines {
        if let Some((test_name, status)) = parse_test_status_line(line) {
            results.insert(
                test_name,
                TestRecord {
                    status: if status == "FAILED" { "FAILED" } else { "PASSED" }.to_string(),
                    idx: -1,
                },
            );
        }
    }

    let mut case_to_test = BTreeMap::new();
    for test_name in results.keys() {
        if let Some(case_key) = first_contains_case_key(test_name) {
            case_to_test.insert(case_key, test_name.clone());
        }
        if let Some(idx) = suffix_large_idx(test_name) {
            case_to_test.insert(format!("sampled_large_case_{idx}"), test_name.clone());
            case_to_test.insert(format!("large_{idx}"), test_name.clone());
        }
        if test_name.ends_with("::large_full") {
            case_to_test.insert("fight_large".to_string(), test_name.clone());
            case_to_test.insert("large_full".to_string(), test_name.clone());
        }
    }
    for direct in ["small_seed", "simple_fight"] {
        if results.contains_key(direct) {
            case_to_test.insert(direct.to_string(), direct.to_string());
        }
    }

    let mut current_thread: Option<String> = None;
    for line in &lines {
        if let Some(thread) = parse_stdout_header(line) {
            current_thread = Some(thread);
        }
        if let Some(thread) = parse_thread_name(line) {
            current_thread = Some(thread);
        }
        let Some(idx_value) = parse_mismatch_idx(line) else {
            continue;
        };

        if let Some(inline_thread) = parse_thread_name(line)
            && let Some(record) = results.get_mut(&inline_thread)
        {
            record.idx = idx_value;
            continue;
        }
        if let Some(thread) = &current_thread
            && let Some(record) = results.get_mut(thread)
        {
            record.idx = idx_value;
            continue;
        }
        if let Some(case_key) = parse_case_key_from_line(line) {
            if let Some(test_name) = case_to_test.get(&case_key)
                && let Some(record) = results.get_mut(test_name)
            {
                record.idx = idx_value;
                continue;
            }
        }
        for direct in ["small_seed", "simple_fight"] {
            if line.contains(direct)
                && let Some(record) = results.get_mut(direct)
            {
                record.idx = idx_value;
                break;
            }
        }
    }

    results
}

fn parse_test_status_line(line: &str) -> Option<(String, &str)> {
    let rest = line.strip_prefix("test ")?;
    let (test_name, status) = rest.split_once(" ... ")?;
    if status == "FAILED" || status == "ok" || status == "ignored" {
        Some((test_name.to_string(), status))
    } else {
        None
    }
}

fn first_contains_case_key(test_name: &str) -> Option<String> {
    if test_name.contains("fight_large") {
        return Some("fight_large".to_string());
    }
    let marker = "sampled_large_case_";
    let pos = test_name.find(marker)?;
    let tail = &test_name[pos..];
    let key = tail.chars().take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_').collect::<String>();
    if key.is_empty() { None } else { Some(key) }
}

fn suffix_large_idx(test_name: &str) -> Option<String> {
    let (_, tail) = test_name.rsplit_once("::large_")?;
    if tail.len() == 2 && tail.chars().all(|ch| ch.is_ascii_digit()) {
        Some(tail.to_string())
    } else {
        None
    }
}

fn parse_stdout_header(line: &str) -> Option<String> {
    let rest = line.strip_prefix("---- ")?;
    let name = rest.strip_suffix(" stdout ----")?;
    Some(name.to_string())
}

fn parse_thread_name(line: &str) -> Option<String> {
    let start = line.find("thread '")? + "thread '".len();
    let end = line[start..].find('\'')? + start;
    Some(line[start..end].to_string())
}

fn parse_mismatch_idx(line: &str) -> Option<i64> {
    let marker = "mismatch at idx=";
    let start = line.find(marker)? + marker.len();
    let digits = line[start..].chars().take_while(|ch| ch.is_ascii_digit()).collect::<String>();
    digits.parse::<i64>().ok()
}

fn parse_case_key_from_line(line: &str) -> Option<String> {
    for key in ["fight_large", "large_full"] {
        if line.contains(key) {
            return Some(key.to_string());
        }
    }
    if let Some(pos) = line.find("large_") {
        let key = line[pos..]
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .collect::<String>();
        if key.len() == "large_00".len() {
            return Some(key);
        }
    }
    if let Some(pos) = line.find("sampled case") {
        let tail = &line[pos + "sampled ".len()..];
        let normalized = tail
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
            .collect::<String>()
            .replace("case-", "case_");
        if !normalized.is_empty() {
            return Some(format!("sampled_large_{normalized}"));
        }
    }
    None
}

fn compare_records(current: &BTreeMap<String, TestRecord>, previous: &BTreeMap<String, TestRecord>) -> Vec<Change> {
    let all_tests = current.keys().chain(previous.keys()).cloned().collect::<BTreeSet<String>>();
    let mut changes = Vec::new();

    for test in all_tests {
        let curr = current.get(&test);
        let prev = previous.get(&test);
        let (Some(curr), Some(prev)) = (curr, prev) else {
            continue;
        };

        if prev.status == "FAILED" && curr.status != "FAILED" {
            changes.push(Change {
                test: test.clone(),
                kind: "NEW_PASS",
                idx: curr.idx,
                prev_idx: prev.idx,
            });
        } else if prev.status != "FAILED" && curr.status == "FAILED" {
            changes.push(Change {
                test: test.clone(),
                kind: "NEW_FAIL",
                idx: curr.idx,
                prev_idx: prev.idx,
            });
        }

        if curr.idx >= 0 && prev.idx >= 0 {
            if curr.idx > prev.idx {
                changes.push(Change {
                    test: test.clone(),
                    kind: "IMPROVED",
                    idx: curr.idx,
                    prev_idx: prev.idx,
                });
            } else if curr.idx < prev.idx {
                changes.push(Change {
                    test,
                    kind: "REGRESSED",
                    idx: curr.idx,
                    prev_idx: prev.idx,
                });
            }
        }
    }

    changes
}

fn print_changes(changes: &[Change]) {
    for change in changes {
        match change.kind {
            "IMPROVED" => println!("[改进] {}: idx {} -> {}", change.test, change.prev_idx, change.idx),
            "REGRESSED" => println!("[退步] {}: idx {} -> {}", change.test, change.prev_idx, change.idx),
            "NEW_FAIL" => println!("[新失败] {}: idx={}", change.test, change.idx),
            "NEW_PASS" => println!("[修复] {}: 从失败变为通过", change.test),
            _ => {}
        }
    }
}

fn print_changes_summary(changes: &[Change], quiet: bool) -> (usize, usize, usize, usize) {
    let mut improved = 0usize;
    let mut regressed = 0usize;
    let mut new_fail = 0usize;
    let mut fixed = 0usize;
    for change in changes {
        match change.kind {
            "IMPROVED" => {
                improved += 1;
                if !quiet {
                    println!("[改进] {}", change.test);
                    println!("       分叉点延后 (idx: {} -> {})", change.prev_idx, change.idx);
                }
            }
            "REGRESSED" => {
                regressed += 1;
                if !quiet {
                    println!("[退步] {}", change.test);
                    println!("       分叉点提前 (idx: {} -> {})", change.prev_idx, change.idx);
                }
            }
            "NEW_FAIL" => {
                new_fail += 1;
                if !quiet {
                    println!("[新失败] {}", change.test);
                    println!("         idx={}", change.idx);
                }
            }
            "NEW_PASS" => {
                fixed += 1;
                if !quiet {
                    println!("[修复] {}", change.test);
                    println!("       从失败变为通过");
                }
            }
            _ => {}
        }
    }
    (improved, regressed, new_fail, fixed)
}

fn print_conclusion(changes: &[Change], commit_wording: bool) {
    let any_improved = changes.iter().any(|c| c.kind == "IMPROVED" || c.kind == "NEW_PASS");
    let any_regressed = changes.iter().any(|c| c.kind == "REGRESSED");
    if any_improved && !any_regressed {
        if commit_wording {
            println!("结论: 修改有效 (有改进且无退步)，可进行 commit 提交");
        } else {
            println!("结论: 修改有效 (有改进且无退步)");
        }
    } else if any_regressed {
        println!("结论: 修改有问题 (存在退步)");
    } else {
        println!("结论: 无明显变化");
    }
}

fn print_checkpoint_comparison(paths: &Paths, current_records: &BTreeMap<String, TestRecord>, quiet: bool) -> Result<(), String> {
    let Some(cp) = latest_checkpoint(&paths.checkpoint_dir)? else {
        return Ok(());
    };
    let changes = compare_records(current_records, &cp.records);
    println!();
    println!("--- vs 存档点 \"{}\" ({}) ---", cp.name, cp.time);
    if !quiet {
        print_changes(&changes);
    }
    print_conclusion(&changes, false);
    Ok(())
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

    let failed = checkpoint
        .records
        .values()
        .filter(|record| record.status == "FAILED" && record.idx >= 0)
        .count();
    println!("存档点 \"{name}\" 已保存 ({})", now.short);
    if failed == 0 {
        println!("  当前所有测试通过");
    } else {
        println!("  包含 {failed} 个失败测试");
    }
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
        let failed = cp.records.values().filter(|record| record.status == "FAILED" && record.idx >= 0).count();
        if failed == 0 {
            println!("  {} ({}) - 全部通过", cp.name, cp.time);
        } else {
            println!("  {} ({}) - {} 个失败", cp.name, cp.time, failed);
        }
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
    let changes = compare_records(&current, &cp.records);
    println!("--- vs 存档点 \"{}\" ({}) ---", cp.name, cp.time);
    print_changes(&changes);
    print_conclusion(&changes, false);
    Ok(())
}

fn cmd_delete(paths: &Paths, name: &str) -> Result<(), String> {
    let path = find_checkpoint(&paths.checkpoint_dir, name)?.ok_or_else(|| format!("存档点 \"{name}\" 不存在"))?;
    fs::remove_file(path).map_err(|e| format!("删除存档点失败: {e}"))?;
    println!("存档点 \"{name}\" 已删除");
    Ok(())
}

fn load_records(path: &Path) -> BTreeMap<String, TestRecord> { read_json(path).unwrap_or_default() }

fn save_records(path: &Path, records: &BTreeMap<String, TestRecord>) -> Result<(), String> { write_json(path, records) }

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

//! benchmark 输出、文件交互与文本格式化 helper。
//!
//! 这些逻辑本身并不决定“跑什么 benchmark”，但决定了用户最终看到什么、文件怎么落地：
//! - perf 行如何打印；
//! - 分组如何显示；
//! - 输出文件已存在时怎样交互确认；
//! - JSONL / log / pure 三种记录格式如何编码。

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write as _};
use std::path::Path;
use std::time::Duration;

use tswn_core::engine::storage::Storage;
use tswn_core::player::Player;
use tswn_core::win_rate::WinRateTiming;

/// 打印 total/init/fight 的耗时拆分。
pub(super) fn print_perf_lines(total_elapsed: Duration, timing: WinRateTiming, total: usize) {
    let total_f = total.max(1) as f64;
    let total_secs = total_elapsed.as_secs_f64();
    let throughput = if total_secs > 0.0 { total_f / total_secs } else { 0.0 };
    println!("─────────────────────────────────");
    println!(
        "total :  {:.3}s  ({:.1}µs/场, {:.0} 场/s)",
        total_secs,
        total_elapsed.as_micros() as f64 / total_f,
        throughput
    );
    println!(
        "init  :  {:.3}s  ({:.1}µs/场)",
        timing.init_nanos as f64 / 1e9,
        timing.init_nanos as f64 / 1e3 / total_f
    );
    println!(
        "fight :  {:.3}s  ({:.1}µs/场)",
        timing.fight_nanos as f64 / 1e9,
        timing.fight_nanos as f64 / 1e3 / total_f
    );
}

/// 把一组 raw 名字打印成更适合终端展示的逗号分隔格式。
pub(super) fn display_group(raw: &str) -> String {
    raw.lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>().join(", ")
}

/// 检查一次 matchup 中是否出现了同名玩家。
///
/// 比较时会先转成 id_name，因此 overlay 后缀不会绕过重复号检查。
pub(super) fn first_duplicate_name_in_matchup(groups: &[&str]) -> Option<String> {
    let mut seen = HashSet::new();
    for group in groups {
        for name in group.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let id_name = Player::raw_namerena_to_idname(name);
            if !seen.insert(id_name.clone()) {
                return Some(id_name);
            }
        }
    }
    None
}

/// 文件已存在时用户可选的动作。
enum ExistingFileAction {
    Overwrite,
    Append,
}

/// 打开 batch-rate / pair 的输出文件。
pub(super) fn open_batch_rate_output(path: &Path, force: bool) -> io::Result<File> {
    if path.file_name().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("输出路径必须包含文件名: {}", path.display()),
        ));
    }
    if path.exists() && path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("输出路径不能是目录: {}", path.display()),
        ));
    }

    ensure_batch_rate_output_parent(path)?;

    if path.exists() {
        match resolve_existing_file_action(path, force)? {
            ExistingFileAction::Overwrite => OpenOptions::new().write(true).truncate(true).open(path),
            ExistingFileAction::Append => OpenOptions::new().append(true).open(path),
        }
    } else {
        OpenOptions::new().write(true).create_new(true).open(path)
    }
}

/// 校验输出目录存在且是目录。
fn ensure_batch_rate_output_parent(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("输出目录不存在: {}", parent.display()),
        ));
    }
    if !parent.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("输出目录不是目录: {}", parent.display()),
        ));
    }
    Ok(())
}

/// 当文件已存在时，决定覆盖还是追加。
fn resolve_existing_file_action(path: &Path, force: bool) -> io::Result<ExistingFileAction> {
    if force {
        return Ok(ExistingFileAction::Overwrite);
    }
    if prompt_yes_no(&format!("输出文件已存在: {}\n是否覆盖? [y/N]: ", path.display()))? {
        return Ok(ExistingFileAction::Overwrite);
    }
    if prompt_yes_no("是否追加? [y/N]: ")? {
        return Ok(ExistingFileAction::Append);
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("输出文件已存在，且未选择覆盖或追加: {}", path.display()),
    ))
}

/// 用 yes/no 方式询问用户。
fn prompt_yes_no(prompt: &str) -> io::Result<bool> {
    let mut stderr = io::stderr().lock();
    stderr.write_all(prompt.as_bytes())?;
    stderr.flush()?;

    let mut line = String::new();
    open_prompt_reader()?.read_line(&mut line)?;
    let answer = line.trim();
    Ok(answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes"))
}

/// 尝试打开用于交互确认的 TTY 输入。
fn open_prompt_reader() -> io::Result<BufReader<File>> {
    open_tty_for_read().map(BufReader::new).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("输出文件已存在，但当前无法交互确认: {err}；请改用 --force/-f"),
        )
    })
}

#[cfg(windows)]
fn open_tty_for_read() -> io::Result<File> { File::open("CONIN$") }

#[cfg(not(windows))]
fn open_tty_for_read() -> io::Result<File> { File::open("/dev/tty") }

/// 以单行文本的形式写入一条 batch 记录。
pub(super) fn write_batch_rate_record(file: &mut File, line: &str) -> io::Result<()> {
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()
}

/// 构造 batch-rate JSONL 记录。
#[allow(clippy::too_many_arguments)]
pub(super) fn format_batch_rate_record(
    label: &str,
    avg_rate: f64,
    aggregate_rate: f64,
    wins: usize,
    total: usize,
    elapsed: Duration,
    throughput: f64,
    valid_matchups: usize,
    skipped_matchups: usize,
    wr_precision: usize,
) -> String {
    format!(
        "{{\"label\":\"{}\",\"avg_win_rate\":{},\"aggregate_win_rate\":{},\"wins\":{wins},\"total\":{total},\"valid_matchups\":{valid_matchups},\"skipped_matchups\":{skipped_matchups},\"elapsed_s\":{:.3},\"us_per_battle\":{:.1},\"battles_per_s\":{throughput:.0}}}",
        escape_json_string(label),
        format_rate(avg_rate, wr_precision),
        format_rate(aggregate_rate, wr_precision),
        elapsed.as_secs_f64(),
        elapsed.as_micros() as f64 / total.max(1) as f64,
    )
}

/// 构造默认 `winrate<space>label` 输出格式。
pub(super) fn format_batch_rate_log_record(label: &str, avg_rate: f64, wr_precision: usize) -> String {
    format!("{} {label}", format_rate(avg_rate, wr_precision))
}

/// 只输出 label 的极简格式。
pub(super) fn format_batch_rate_pure_record(label: &str) -> String { label.to_string() }

/// 构造 pair JSONL 记录。
#[allow(clippy::too_many_arguments)]
pub(super) fn format_pair_rate_record(
    label: &str,
    final_score: f64,
    selected_count: usize,
    head: usize,
    pair_rates: &[(f64, String)],
    aggregate_rate: f64,
    wins: usize,
    total: usize,
    elapsed: Duration,
    throughput: f64,
    valid_matchups: usize,
    skipped_matchups: usize,
    wr_precision: usize,
) -> String {
    let top_pairs = pair_rates
        .iter()
        .take(selected_count)
        .map(|(rate, teammate)| {
            format!(
                "{{\"teammate\":\"{}\",\"batch_rate\":{}}}",
                escape_json_string(teammate),
                format_rate(*rate, wr_precision)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"label\":\"{}\",\"score\":{},\"head\":{head},\"selected\":{selected_count},\"top_pairs\":[{top_pairs}],\"aggregate_win_rate\":{},\"wins\":{wins},\"total\":{total},\"valid_matchups\":{valid_matchups},\"skipped_matchups\":{skipped_matchups},\"elapsed_s\":{:.3},\"us_per_battle\":{:.1},\"battles_per_s\":{throughput:.0}}}",
        escape_json_string(label),
        format_rate(final_score, wr_precision),
        format_rate(aggregate_rate, wr_precision),
        elapsed.as_secs_f64(),
        elapsed.as_micros() as f64 / total.max(1) as f64,
    )
}

/// 统一处理小数位数和负零问题。
pub(super) fn format_rate(value: f64, precision: usize) -> String {
    let value = if value.abs() < 0.5_f64 * 10_f64.powi(-(precision as i32)) {
        0.0
    } else {
        value
    };
    format!("{value:.precision$}")
}

/// 将 player-list 里的普通名字转换成 `+ol`，保持 pair 组合输入稳定。
pub(super) fn player_to_ol_or_exit(raw: &str) -> String {
    if raw.contains("+diy[") || raw.contains("+ol:") {
        return raw.to_string();
    }
    let storage = Storage::new_arc();
    let mut player = match Player::new_from_namerena_raw(raw.to_string(), storage) {
        Ok(player) => player,
        Err(err) => {
            eprintln!("转换 player-list 名字为 +ol 失败: {raw}: {err}");
            std::process::exit(1);
        }
    };
    player.build();
    player.to_ol_json()
}

/// 最小 JSON 字符串转义。
fn escape_json_string(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_name_check_detects_cross_group_duplicate() {
        assert_eq!(
            first_duplicate_name_in_matchup(&["alice\nbob", "carol\nalice"]),
            Some("alice".to_string())
        );
    }

    #[test]
    fn duplicate_name_check_ignores_overlay_suffix() {
        let base = "涵虚不等式 PFVKEUPBU@TigerStar";
        let overlay = r#"涵虚不等式 PFVKEUPBU@TigerStar+ol:{"attrs":[89,85,88,77,48,96,97,327]}"#;
        assert_eq!(first_duplicate_name_in_matchup(&[base, overlay]), Some(base.to_string()));
    }

    #[test]
    fn duplicate_name_check_allows_distinct_names() {
        assert_eq!(first_duplicate_name_in_matchup(&["alice\nbob", "carol\ndave"]), None);
    }
}
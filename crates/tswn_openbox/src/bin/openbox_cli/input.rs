//! `openbox-cli` 的输入读取与参数校验 helper。
//!
//! 与 `tswn_cli` 的同名 helper 是两套独立实现：openbox-cli 属于 GUI 适配 crate，
//! 不能反向依赖 core 的 bin 内部模块；`--metric` 语法与行为刻意保持一致。

use std::fs;
use std::path::{Path, PathBuf};

use clap::CommandFactory;
use clap::error::ErrorKind;
use tswn_openbox::backend::NamerPfMetric;

use super::args::Cli;
use super::plan::MetricSpec;

/// 从文件读取完整文本，统一去掉 UTF-8 BOM。
pub(super) fn read_file(path: &Path) -> Result<String, clap::Error> {
    let content = fs::read_to_string(path).map_err(|err| cli_error(format!("读取文件失败: {err}")))?;
    Ok(content.strip_prefix('\u{feff}').unwrap_or(&content).to_string())
}

/// 把命令行里的字面量 `\n` 还原成真实换行。
pub(super) fn decode_raw(raw: &str) -> String { raw.replace("\\n", "\n") }

pub(super) fn parse_thread_count(raw: &str) -> Result<usize, String> {
    let value = raw.parse::<usize>().map_err(|_| "线程数必须是正整数".to_string())?;
    if value == 0 {
        Err("线程数必须大于 0".to_string())
    } else {
        Ok(value)
    }
}

/// 解析并校验百分比阈值参数 (0~100)。
pub(super) fn parse_percent_0_100(raw: &str) -> Result<f64, String> {
    let value = raw.parse::<f64>().map_err(|_| "阈值必须是 0~100 之间的数字".to_string())?;
    if !(0.0..=100.0).contains(&value) {
        Err("阈值必须在 0~100 之间".to_string())
    } else {
        Ok(value)
    }
}

/// 解析非负浮点数。
pub(super) fn parse_non_negative_f64(raw: &str) -> Result<f64, String> {
    let value = raw.parse::<f64>().map_err(|_| "阈值必须是非负数字".to_string())?;
    if value < 0.0 {
        Err("阈值必须不小于 0".to_string())
    } else {
        Ok(value)
    }
}

/// 解析胜率/分数输出的小数位数 (0~9)。
pub(super) fn parse_wr_precision(raw: &str) -> Result<usize, String> {
    let value = raw.parse::<usize>().map_err(|_| "小数位数必须是 0~9 之间的整数".to_string())?;
    if value > 9 {
        Err("小数位数必须在 0~9 之间".to_string())
    } else {
        Ok(value)
    }
}

/// 构造统一风格的 CLI 参数校验错误。
pub(super) fn cli_error(message: impl Into<String>) -> clap::Error {
    Cli::command().error(ErrorKind::ValueValidation, message.into())
}

/// 解析 `namer-pf --metric` 的单项规格。
///
/// 语法 `NAME[:MIN_SCREEN[:FILE[:MIN_FILE]]]`，空段表示跳过该可选项，
/// 例如 `pp::pp.txt` 表示无屏幕阈值但写入 `pp.txt`。
///
/// FILE 段按“最后一段是可解析数字才算 MIN_FILE”的规则回切，因此 Windows 的
/// `C:\dir\out.txt` 这类带盘符冒号的路径不会被切碎；其余含冒号路径直接报错，
/// 避免和语法混淆（ADS 等合法含冒号路径请改用绝对/相对无冒号形式）。
pub(super) fn parse_metric_spec(raw: &str) -> Result<MetricSpec, String> {
    let mut segments = raw.split(':');
    let name = segments.next().unwrap_or_default().trim();
    let metric = match name.to_ascii_lowercase().as_str() {
        "pp" => NamerPfMetric::Pp,
        "pd" => NamerPfMetric::Pd,
        "qp" => NamerPfMetric::Qp,
        "qd" => NamerPfMetric::Qd,
        "sum" => NamerPfMetric::Sum,
        _ => return Err(format!("评分项 NAME 必须是 pp/pd/qp/qd/sum 之一，当前为: {name}")),
    };
    let min_screen = match segments.next() {
        Some(segment) if !segment.trim().is_empty() => Some(parse_non_negative_f64(segment.trim())?),
        _ => None,
    };
    let mut rest = segments.collect::<Vec<_>>();
    if rest.len() > 1 && rest.last().is_some_and(|segment| segment.trim().is_empty()) {
        rest.pop();
    }
    let min_file = if rest.len() >= 2 && rest.last().is_some_and(|segment| segment.trim().parse::<f64>().is_ok()) {
        let raw_min_file = rest.pop().expect("至少两段时才回切");
        Some(parse_non_negative_f64(raw_min_file.trim())?)
    } else {
        None
    };
    let output_file = (!rest.is_empty()).then(|| PathBuf::from(rest.join(":").trim()));
    if let Some(path) = output_file.as_ref()
        && path_has_illegal_colon(path)
    {
        return Err(format!(
            "输出文件路径不能包含 ':'（Windows 盘符前缀如 C:\\ 除外）: {}",
            path.display()
        ));
    }
    Ok(MetricSpec {
        metric,
        min_screen,
        output_file,
        min_file,
    })
}

/// 判断路径里的 `:` 是否非法：唯一例外是 Windows 盘符前缀（`C:\dir\out.txt`）。
fn path_has_illegal_colon(path: &Path) -> bool {
    let raw = path.to_string_lossy();
    let mut chars = raw.chars();
    let is_drive = matches!((chars.next(), chars.next()), (Some(letter), Some(':')) if letter.is_ascii_alphabetic());
    !is_drive && raw.contains(':')
}

#[cfg(test)]
mod tests {
    use super::*;
    use tswn_openbox::backend::NamerPfMetric;

    #[test]
    fn metric_spec_parses_all_four_segments() {
        let spec = parse_metric_spec("sum:30000:out.txt:25000").unwrap();
        assert_eq!(spec.metric, NamerPfMetric::Sum);
        assert_eq!(spec.min_screen, Some(30000.0));
        assert_eq!(spec.output_file, Some(PathBuf::from("out.txt")));
        assert_eq!(spec.min_file, Some(25000.0));
    }

    #[test]
    fn metric_spec_allows_empty_middle_segment() {
        let spec = parse_metric_spec("qp::qp.txt").unwrap();
        assert_eq!(spec.metric, NamerPfMetric::Qp);
        assert_eq!(spec.min_screen, None);
        assert_eq!(spec.output_file, Some(PathBuf::from("qp.txt")));
        assert_eq!(spec.min_file, None);
    }

    #[test]
    fn metric_spec_keeps_windows_drive_letter_in_file() {
        let spec = parse_metric_spec(r"pd:8000:C:\scores\pd.txt:7000").unwrap();
        assert_eq!(spec.min_screen, Some(8000.0));
        assert_eq!(spec.output_file, Some(PathBuf::from(r"C:\scores\pd.txt")));
        assert_eq!(spec.min_file, Some(7000.0));
    }

    #[test]
    fn metric_spec_rejects_invalid_names_and_thresholds() {
        assert!(parse_metric_spec("xp").is_err());
        assert!(parse_metric_spec("pp:abc").is_err());
        assert!(parse_metric_spec("pp:8000:out.txt:abc").is_err());
        assert!(parse_metric_spec(r"pp:8000:weird:name.txt").is_err());
    }

    #[test]
    fn read_file_strips_bom() {
        let dir = std::env::temp_dir().join(format!("tswn_openbox_cli_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bom.txt");
        std::fs::write(&path, b"\xef\xbb\xbfmario\nluigi").unwrap();
        assert_eq!(read_file(&path).unwrap(), "mario\nluigi");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn decode_raw_restores_newlines() {
        assert_eq!(decode_raw("mario\\nluigi"), "mario\nluigi");
    }
}

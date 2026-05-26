//! 输入读取、参数校验和文本切分 helper。
//!
//! 这些函数看起来都很小，但它们决定了 CLI 在三个关键边界上的行为：
//! - 用户到底是从命令行、文件还是 stdin 提供数据；
//! - 哪些参数组合应该尽早报错；
//! - 文本文件中的每一行如何被解释成“一个名字”或“一整组名字”。
//!
//! 把这一层从命令树定义里拆开后，后续调整输入规则时就不需要在几百行 `clap`
//! 声明之间来回定位辅助函数。

use std::fs;
use std::io::{self, Read};
use std::path::Path;

use clap::CommandFactory;
use clap::error::ErrorKind;

use super::cli::Cli;

/// 从标准输入读取完整的 namerena 原始输入。
///
/// CLI 的 `fight` / `bench auto` / `namer-pf` 都允许省略 `--raw` 和 `--file`，
/// 此时这里就是最终兜底。读取完成后会统一去掉 UTF-8 BOM，并拒绝纯空白输入。
pub(super) fn read_stdin() -> Result<String, clap::Error> {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .map_err(|err| cli_error(format!("读取 stdin 失败: {err}")))?;
    let raw = strip_utf8_bom(&raw).to_string();
    if raw.trim().is_empty() {
        return Err(cli_error("未提供 raw_namerena 输入"));
    }
    Ok(raw)
}

/// 从指定文件读取完整文本输入，并统一处理 BOM。
pub(super) fn read_file(path: &Path) -> Result<String, clap::Error> {
    let content = fs::read_to_string(path).map_err(|err| cli_error(format!("读取文件失败: {err}")))?;
    Ok(strip_utf8_bom(&content).to_string())
}

/// 将命令行里的字面量 `\n` 还原成真实换行。
///
/// 这允许用户在 shell 中直接写一条短 raw，而不必专门准备文件。
pub(super) fn decode_raw(raw: &str) -> String { raw.replace("\\n", "\n") }

/// 解析并校验 benchmark 线程数参数。
pub(super) fn parse_thread_count(raw: &str) -> Result<usize, String> {
    let value = raw.parse::<usize>().map_err(|_| "线程数必须是正整数".to_string())?;
    if value == 0 {
        Err("线程数必须大于 0".to_string())
    } else {
        Ok(value)
    }
}

/// 解析严格大于 0 的整数。
pub(super) fn parse_positive_usize(raw: &str) -> Result<usize, String> {
    let value = raw.parse::<usize>().map_err(|_| "参数必须是正整数".to_string())?;
    if value == 0 {
        Err("参数必须大于 0".to_string())
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

/// 解析胜率输出的小数位数。
pub(super) fn parse_wr_precision(raw: &str) -> Result<usize, String> {
    let value = raw.parse::<usize>().map_err(|_| "小数位数必须是 0~9 之间的整数".to_string())?;
    if value > 9 {
        Err("小数位数必须在 0~9 之间".to_string())
    } else {
        Ok(value)
    }
}

/// 构造统一风格的 CLI 参数校验错误。
///
/// 把错误构造集中在这里，是为了保证所有手写校验都以和 `clap` 原生错误一致的形式输出。
pub(super) fn cli_error(message: impl Into<String>) -> clap::Error {
    Cli::command().error(ErrorKind::ValueValidation, message.into())
}

/// 去除 UTF-8 BOM (U+FEFF) 前缀。
pub(super) fn strip_utf8_bom(s: &str) -> &str { s.strip_prefix('\u{feff}').unwrap_or(s) }

/// 解析 `to-diy --file` 输入。
///
/// 文件模式按“每行一个名字”解释；空行会被忽略，但若最后没有任何有效名字则直接报错。
pub(super) fn parse_to_diy_file_names(content: &str) -> Result<Vec<String>, clap::Error> {
    let names = parse_line_list(content);
    if names.is_empty() {
        Err(cli_error("to-diy 文件中没有有效名字"))
    } else {
        Ok(names)
    }
}

/// 解析“每个非空行是一项”的纯行列表。
pub(super) fn parse_line_list(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// 解析“每个非空行都是一个 `+` 分隔组”的文件内容。
///
/// 返回转换后的 namerena 组字符串列表，组内成员之间用 `\n` 分隔。
/// 这是 `bench batch-rate` / `bench pair` 的靶子列表和部分玩家列表的标准格式。
pub(super) fn parse_plus_separated_groups(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.split('+').map(str::trim).collect::<Vec<_>>().join("\n"))
        .collect()
}

/// 与 `parse_plus_separated_groups` 相同，但会额外保留每行原始文本作为展示标签。
///
/// `double_plus=true` 时，组内分隔符改成 `++`，用来避免把玩家名字里的 `+diy[...]`
/// 或 `+ol:...` 误切开。这里刻意只在 player-list 上启用，因为靶子列表不需要兼容这类格式。
pub(super) fn parse_player_groups_with_labels(content: &str, double_plus: bool) -> (Vec<String>, Vec<String>) {
    let mut groups = Vec::new();
    let mut labels = Vec::new();
    for line in content.lines().map(str::trim).filter(|line| !line.is_empty()) {
        labels.push(line.to_string());
        let separator = if double_plus { "++" } else { "+" };
        groups.push(line.split(separator).map(str::trim).collect::<Vec<_>>().join("\n"));
    }
    (groups, labels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_groups_default_split_uses_single_plus() {
        let (groups, labels) = parse_player_groups_with_labels("aaaa+bbbb", false);
        assert_eq!(groups, vec!["aaaa\nbbbb".to_string()]);
        assert_eq!(labels, vec!["aaaa+bbbb".to_string()]);
    }

    #[test]
    fn player_groups_double_plus_keeps_diy_plus_inside_player() {
        let diy = r#"aaaa+diy[58,87,82,78,89,93,99,343]{"skldefend":13}"#;
        let raw = format!("{diy}++bbbb");
        let (groups, labels) = parse_player_groups_with_labels(&raw, true);

        assert_eq!(groups, vec![format!("{diy}\nbbbb")]);
        assert_eq!(labels, vec![raw]);
    }

    #[test]
    fn to_diy_file_names_skip_blank_lines() {
        assert_eq!(
            parse_to_diy_file_names("aaaa\n\n bbbb@team \r\n").unwrap(),
            vec!["aaaa".to_string(), "bbbb@team".to_string()]
        );
    }

    #[test]
    fn to_diy_file_names_reject_empty_file() {
        assert!(parse_to_diy_file_names("\n \r\n").is_err());
    }
}

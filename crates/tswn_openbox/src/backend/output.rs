//! 文件创建与结果排序；独立于计算和屏幕输出。

use std::cmp::Ordering as CmpOrdering;
use std::fs::{self, File};
use std::io::Write as _;
use std::path::Path;

use super::types::OutputMode;

pub(super) fn create_output_file(path: &Path) -> Result<File, String> {
    if path.file_name().is_none() {
        return Err(format!("输出路径必须包含文件名: {}", path.display()));
    }
    if path.exists() && path.is_dir() {
        return Err(format!("输出路径不能是目录: {}", path.display()));
    }
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty())
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|err| format!("创建输出目录失败: {}: {err}", parent.display()))?;
    }
    File::create(path).map_err(|err| format!("打开输出文件失败: {}: {err}", path.display()))
}

pub(super) fn finalize_sorted_output_file(mut output: Option<File>, path: Option<&Path>, mode: OutputMode) -> Result<(), String> {
    let Some(path) = path else {
        return Ok(());
    };
    if let Some(file) = output.as_mut() {
        file.flush().map_err(|err| format!("刷新输出文件失败: {}: {err}", path.display()))?;
    }
    drop(output);
    sort_score_output_file(path, mode)
}

fn sort_score_output_file(path: &Path, mode: OutputMode) -> Result<(), String> {
    if mode == OutputMode::Pure {
        return Ok(());
    }
    let content = fs::read_to_string(path).map_err(|err| format!("读取输出文件失败: {}: {err}", path.display()))?;
    let lines = sorted_score_lines(&content, mode);
    let file = create_output_file(path)?;
    let mut writer = std::io::BufWriter::new(file);
    for line in lines {
        writeln!(writer, "{line}").map_err(|err| format!("写入排序输出文件失败: {}: {err}", path.display()))?;
    }
    writer.flush().map_err(|err| format!("刷新排序输出文件失败: {}: {err}", path.display()))
}

fn sorted_score_lines(content: &str, mode: OutputMode) -> Vec<&str> {
    // 每行只解析一次；JSONL 不再在 O(N log N) 次比较中重复分配并解析 JSON。
    let mut lines: Vec<_> = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| (line, score_output_line_value(line, mode)))
        .collect();
    lines.sort_unstable_by(|(left, left_score), (right, right_score)| match (left_score, right_score) {
        (Some(a), Some(b)) => b.total_cmp(a).then_with(|| left.cmp(right)),
        (Some(_), None) => CmpOrdering::Less,
        (None, Some(_)) => CmpOrdering::Greater,
        (None, None) => left.cmp(right),
    });
    lines.into_iter().map(|(line, _)| line).collect()
}

fn score_output_line_value(line: &str, mode: OutputMode) -> Option<f64> {
    match mode {
        OutputMode::Log => line.split_whitespace().next()?.parse().ok(),
        OutputMode::Jsonl => {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            value
                .get("avg_win_rate")
                .or_else(|| value.get("score"))
                .and_then(serde_json::Value::as_f64)
        }
        OutputMode::Pure => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonl_sort_keeps_ties_invalid_lines_and_both_score_keys() {
        let lines = sorted_score_lines(
            "{\"score\":2,\"label\":\"b\"}\n{\"score\":2,\"label\":\"a\"}\n\n{\"avg_win_rate\":3}\n{}\nbad\n",
            OutputMode::Jsonl,
        );
        assert_eq!(
            lines,
            vec![
                r#"{"avg_win_rate":3}"#,
                r#"{"score":2,"label":"a"}"#,
                r#"{"score":2,"label":"b"}"#,
                "bad",
                "{}",
            ]
        );
    }
    #[test]
    fn log_output_lines_sort_by_score_descending() {
        let lines = sorted_score_lines("12.000 beta\n99.500 alpha\nbad line\n99.500 gamma\n", OutputMode::Log);
        assert_eq!(lines, vec!["99.500 alpha", "99.500 gamma", "12.000 beta", "bad line"]);
    }

    #[test]
    fn jsonl_output_line_score_accepts_batch_and_pair_keys() {
        assert_eq!(
            score_output_line_value(r#"{"label":"a","avg_win_rate":64.25}"#, OutputMode::Jsonl),
            Some(64.25)
        );
        assert_eq!(
            score_output_line_value(r#"{"label":"a","score":300.0}"#, OutputMode::Jsonl),
            Some(300.0)
        );
    }
}

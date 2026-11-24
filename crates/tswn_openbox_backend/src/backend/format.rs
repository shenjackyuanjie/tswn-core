//! 批量结果的纯文本与 JSONL 格式化，不负责文件写入或对局执行。
//!
//! 输入分数保留内部精度，precision 仅影响显示；标签按展示规则剥离 overlay。
//! 屏幕明细与文件记录分别生成，Pure 仅输出名字；格式化结果为独立 String，
//! 不借用任务状态，也不会按屏幕阈值重新筛选已经传入的结果。

use std::fmt::Write as _;

use super::types::{OutputMode, PairDetailMode};

/// 清洗名字标签：去掉 ol: 与 diy[ 前缀的成员和空成员，再用 + 连接。
pub fn clean_name_label(raw: &str) -> String {
    raw.split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty() && !part.starts_with("ol:") && !part.starts_with("diy["))
        .collect::<Vec<_>>()
        .join("+")
}

/// 清洗分组标签：逐行清洗名字后用 + 连接。
pub fn clean_group_label(raw: &str) -> String {
    raw.lines()
        .map(clean_name_label)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("+")
}

/// 按输出模式（Log/Pure/Jsonl）格式化一条批量结果记录。
pub fn format_batch_file_record(mode: OutputMode, label: &str, avg: f64, precision: usize) -> String {
    let label = clean_name_label(label);
    match mode {
        OutputMode::Log => format!("{} {label}", format_rate(avg, precision)),
        OutputMode::Pure => label,
        OutputMode::Jsonl => format!(
            "{{\"label\":\"{}\",\"avg_win_rate\":{}}}",
            escape_json_string(&label),
            format_rate(avg, precision),
        ),
    }
}

/// 格式化批量结果的屏幕日志：平均分加各靶子明细。
pub fn format_batch_screen_log(label: &str, avg: f64, detail_rates: &[(f64, String)], precision: usize) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{} {}", format_rate(avg, precision), clean_name_label(label));
    for (rate, target) in detail_rates {
        let _ = writeln!(out, "  {} {}", format_rate(*rate, precision), clean_group_label(target));
    }
    out
}

/// 按输出模式格式化一条配对结果记录。
pub fn format_pair_file_record(
    mode: OutputMode,
    label: &str,
    final_score: f64,
    selected_count: usize,
    head: usize,
    pair_rates: &[(f64, String)],
    precision: usize,
) -> String {
    let label = clean_name_label(label);
    match mode {
        OutputMode::Log => format!("{} {label}", format_rate(final_score, precision)),
        OutputMode::Pure => label,
        OutputMode::Jsonl => {
            let top_pairs = pair_rates
                .iter()
                .take(selected_count)
                .map(|(rate, teammate)| {
                    format!(
                        "{{\"teammate\":\"{}\",\"cqp\":{}}}",
                        escape_json_string(&clean_name_label(teammate)),
                        format_rate(*rate, precision)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"label\":\"{}\",\"score\":{},\"head\":{},\"selected\":{},\"top_pairs\":[{}]}}",
                escape_json_string(&label),
                format_rate(final_score, precision),
                head,
                selected_count,
                top_pairs,
            )
        }
    }
}

/// 格式化配对结果的屏幕日志，按 detail_mode 控制明细层级。
pub fn format_pair_screen_log(
    label: &str,
    final_score: f64,
    selected_count: usize,
    pair_rates: &[(f64, String)],
    detail_mode: PairDetailMode,
    detail_min: Option<f64>,
    precision: usize,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{} {}", format_rate(final_score, precision), clean_name_label(label));
    match detail_mode {
        PairDetailMode::None => {}
        PairDetailMode::Top => {
            for (rate, teammate) in pair_rates.iter().take(selected_count) {
                let _ = writeln!(out, "  {} {}", format_rate(*rate, precision), clean_name_label(teammate));
            }
        }
        PairDetailMode::Every => {
            for (rate, teammate) in pair_rates {
                if detail_min.is_none_or(|limit| *rate >= limit) {
                    let _ = writeln!(out, "  {} {}", format_rate(*rate, precision), clean_name_label(teammate));
                }
            }
        }
    }
    out
}

/// 把多行分组格式化为逗号连接的单行（测试辅助）。
#[cfg(test)]
pub fn display_group(raw: &str) -> String {
    raw.lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>().join(", ")
}

/// 按精度格式化比率；小于半个精度格的值归零。
pub fn format_rate(value: f64, precision: usize) -> String {
    let value = if value.abs() < 0.5_f64 * 10_f64.powi(-(precision as i32)) {
        0.0
    } else {
        value
    };
    format!("{value:.precision$}")
}

/// 转义 JSON 字符串中的引号、反斜杠与控制字符。
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

/// 判断分数是否达到 屏幕阈值 + 高亮增量。
pub(super) fn should_highlight(score: f64, min_screen: Option<f64>, highlight_delta: Option<f64>) -> bool {
    highlight_delta.is_some_and(|delta| score >= min_screen.unwrap_or(0.0) + delta)
}

/// 技能 id 到中文名的对照表，顺序与 `skill_name_to_id` 一一对应：
/// 火球 冰冻 雷击 地裂 吸血 投毒 连击 会心 瘟疫 命轮 狂暴 魅惑 加速 减速 诅咒
/// 治愈 苏生 净化 铁壁 蓄力 聚气 潜行 血祭 分身 幻术 防御 守护 反弹 护符 护盾
/// 反击 吞噬 召灵 垂死 隐匿
pub(super) const SKILL_CN_NAMES: [&str; 35] = [
    "火球", "冰冻", "雷击", "地裂", "吸血", "投毒", "连击", "会心", "瘟疫", "命轮", "狂暴", "魅惑", "加速", "减速", "诅咒",
    "治愈", "苏生", "净化", "铁壁", "蓄力", "聚气", "潜行", "血祭", "分身", "幻术", "防御", "守护", "反弹", "护符", "护盾",
    "反击", "吞噬", "召灵", "垂死", "隐匿",
];

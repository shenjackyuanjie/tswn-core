//! 批量结果文本格式化。

use std::fmt::Write as _;
use std::time::Duration;

use super::types::{OutputMode, PairDetailMode};

pub fn clean_name_label(raw: &str) -> String {
    raw.split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty() && !part.starts_with("ol:") && !part.starts_with("diy["))
        .collect::<Vec<_>>()
        .join("+")
}

pub fn clean_group_label(raw: &str) -> String {
    raw.lines()
        .map(clean_name_label)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("+")
}

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

pub fn format_batch_screen_log(
    label: &str,
    avg: f64,
    matchup_rates: &[(f64, String)],
    show_matchups: bool,
    precision: usize,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{} {}", format_rate(avg, precision), clean_name_label(label));
    if show_matchups {
        for (rate, target) in matchup_rates {
            let _ = writeln!(out, "  {} {}", format_rate(*rate, precision), clean_group_label(target));
        }
    }
    out
}

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

pub fn display_group(raw: &str) -> String {
    raw.lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>().join(", ")
}

pub fn format_rate(value: f64, precision: usize) -> String {
    let value = if value.abs() < 0.5_f64 * 10_f64.powi(-(precision as i32)) {
        0.0
    } else {
        value
    };
    format!("{value:.precision$}")
}

pub fn _throughput(total: usize, elapsed: Duration) -> f64 {
    let secs = elapsed.as_secs_f64();
    if secs > 0.0 { total as f64 / secs } else { 0.0 }
}

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

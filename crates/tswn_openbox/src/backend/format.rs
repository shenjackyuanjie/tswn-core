use std::fmt::Write as _;
use std::time::Duration;

use tswn_core::win_rate::WinRateTiming;

use super::types::OutputMode;

pub(crate) fn format_batch_file_record(
    mode: OutputMode,
    label: &str,
    avg: f64,
    aggregate_rate: f64,
    wins: usize,
    total: usize,
    valid_matchups: usize,
    skipped_matchups: usize,
    elapsed: Duration,
    precision: usize,
) -> String {
    match mode {
        OutputMode::Log => format!("{} {label}", format_rate(avg, precision)),
        OutputMode::Pure => label.to_string(),
        OutputMode::Jsonl => format!(
            "{{\"label\":\"{}\",\"avg_win_rate\":{},\"aggregate_win_rate\":{},\"wins\":{},\"total\":{},\"valid_matchups\":{},\"skipped_matchups\":{},\"elapsed_s\":{:.3},\"us_per_battle\":{:.1},\"battles_per_s\":{:.0}}}",
            escape_json_string(label),
            format_rate(avg, precision),
            format_rate(aggregate_rate, precision),
            wins,
            total,
            valid_matchups,
            skipped_matchups,
            elapsed.as_secs_f64(),
            elapsed.as_micros() as f64 / total.max(1) as f64,
            throughput(total, elapsed)
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn format_batch_screen_log(
    index: usize,
    total_players: usize,
    label: &str,
    avg: f64,
    aggregate_rate: f64,
    wins: usize,
    total: usize,
    valid_matchups: usize,
    skipped_matchups: usize,
    elapsed: Duration,
    timing: WinRateTiming,
    precision: usize,
    verbose: bool,
    verbose_text: &str,
    perf: bool,
) -> String {
    let mut out = String::new();
    if verbose && !verbose_text.trim().is_empty() {
        let _ = writeln!(out, "{verbose_text}");
    }
    let _ = writeln!(
        out,
        "[{index}/{total_players}] {label}\t平均胜率: {}%\t汇总: {}% ({wins}/{total})\t有效: {valid_matchups}\t跳过重复: {skipped_matchups}\t用时: {:.3}s",
        format_rate(avg, precision),
        format_rate(aggregate_rate, precision),
        elapsed.as_secs_f64()
    );
    if perf {
        out.push_str(&format_perf_lines(elapsed, timing, total));
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn format_pair_file_record(
    mode: OutputMode,
    label: &str,
    final_score: f64,
    selected_count: usize,
    head: usize,
    pair_rates: &[(f64, String)],
    aggregate_rate: f64,
    total_wins: usize,
    total_battles: usize,
    valid_matchups: usize,
    skipped_matchups: usize,
    elapsed: Duration,
    precision: usize,
) -> String {
    match mode {
        OutputMode::Log => format!("{} {label}", format_rate(final_score, precision)),
        OutputMode::Pure => label.to_string(),
        OutputMode::Jsonl => {
            let top_pairs = pair_rates
                .iter()
                .take(selected_count)
                .map(|(rate, teammate)| {
                    format!(
                        "{{\"teammate\":\"{}\",\"batch_rate\":{}}}",
                        escape_json_string(teammate),
                        format_rate(*rate, precision)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"label\":\"{}\",\"score\":{},\"head\":{},\"selected\":{},\"top_pairs\":[{}],\"aggregate_win_rate\":{},\"wins\":{},\"total\":{},\"valid_matchups\":{},\"skipped_matchups\":{},\"elapsed_s\":{:.3},\"battles_per_s\":{:.0}}}",
                escape_json_string(label),
                format_rate(final_score, precision),
                head,
                selected_count,
                top_pairs,
                format_rate(aggregate_rate, precision),
                total_wins,
                total_battles,
                valid_matchups,
                skipped_matchups,
                elapsed.as_secs_f64(),
                throughput(total_battles, elapsed)
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn format_pair_screen_log(
    index: usize,
    total_players: usize,
    label: &str,
    final_score: f64,
    selected_count: usize,
    head: usize,
    pair_rates: &[(f64, String)],
    aggregate_rate: f64,
    total_wins: usize,
    total_battles: usize,
    valid_matchups: usize,
    skipped_matchups: usize,
    elapsed: Duration,
    timing: WinRateTiming,
    precision: usize,
    verbose: bool,
    verbose_text: &str,
    perf: bool,
) -> String {
    let mut out = String::new();
    if verbose && !verbose_text.trim().is_empty() {
        let _ = writeln!(out, "{verbose_text}");
    }
    let _ = writeln!(
        out,
        "[{index}/{total_players}] {label}\t最终分数: {}\ttop: {selected_count}/{head}",
        format_rate(final_score, precision)
    );
    for (rank, (rate, teammate)) in pair_rates.iter().take(selected_count).enumerate() {
        let _ = writeln!(out, "  #{} {}% {}", rank + 1, format_rate(*rate, precision), teammate);
    }
    let _ = writeln!(
        out,
        "  汇总胜率: {}% ({total_wins}/{total_battles})  有效靶子: {valid_matchups}  跳过重复: {skipped_matchups}  用时: {:.3}s",
        format_rate(aggregate_rate, precision),
        elapsed.as_secs_f64()
    );
    if perf {
        out.push_str(&format_perf_lines(elapsed, timing, total_battles));
    }
    out
}

pub(crate) fn format_perf_lines(total_elapsed: Duration, timing: WinRateTiming, total: usize) -> String {
    let total_f = total.max(1) as f64;
    let total_secs = total_elapsed.as_secs_f64();
    let throughput = if total_secs > 0.0 { total_f / total_secs } else { 0.0 };
    format!(
        "─────────────────────────────────\ntotal :  {:.3}s  ({:.1}µs/场, {:.0} 场/s)\ninit  :  {:.3}s  ({:.1}µs/场)\nfight :  {:.3}s  ({:.1}µs/场)",
        total_secs,
        total_elapsed.as_micros() as f64 / total_f,
        throughput,
        timing.init_nanos as f64 / 1e9,
        timing.init_nanos as f64 / 1e3 / total_f,
        timing.fight_nanos as f64 / 1e9,
        timing.fight_nanos as f64 / 1e3 / total_f,
    )
}

pub(crate) fn display_group(raw: &str) -> String {
    raw.lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>().join(", ")
}

pub(crate) fn format_rate(value: f64, precision: usize) -> String {
    let value = if value.abs() < 0.5_f64 * 10_f64.powi(-(precision as i32)) {
        0.0
    } else {
        value
    };
    format!("{value:.precision$}")
}

fn throughput(total: usize, elapsed: Duration) -> f64 {
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

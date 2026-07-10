//! Shared replay test suite.
//!
//! The files in this module are engine-agnostic fixtures. They only consume
//! raw input, rendered update snapshots, winners, and scores.

use std::collections::HashMap;

use crate::{CoreEngine, EngineAdapter, EventSnapshot, SnapshotKind};
use tswn_core::engine::update::{RunUpdate, UpdateType};

pub mod fight_large;
pub mod fight_multi_1;
pub mod fight_multi_2;
pub mod fight_multi_3;
pub mod fight_multi_4;
pub mod fight_multi_5;
pub mod fight_multi_6;
pub mod fight_multi_7;
pub mod fight_multi_8;
pub mod large_01_10;
pub mod large_11_17;
pub mod large_18_22;
pub mod large_23_30;
pub mod large_31_40;
pub mod large_41_45;
pub mod large_46_50;
pub mod large_51_55;
pub mod large_56_61;
pub mod large_62_65;
pub mod large_66_70;
pub mod large_71_80;
pub mod simple;
pub mod small;

fn format_core_update_message(runner: &tswn_core::Runner, update: &RunUpdate) -> String {
    let caster = runner
        .storage
        .get_player(&update.caster)
        .map(|plr| plr.display_name())
        .unwrap_or_else(|| format!("#{}", update.caster));
    let target = runner
        .storage
        .get_player(&update.target)
        .map(|plr| plr.display_name())
        .unwrap_or_else(|| format!("#{}", update.target));
    let mut msg = update.message.to_string();
    msg = msg.replace("[0]", &caster);
    msg = msg.replace("[1]", &target);
    let param = if let Some(p) = update.param {
        p.to_string()
    } else if update.targets.is_empty() {
        update.score.to_string()
    } else {
        update
            .targets
            .iter()
            .map(|id| {
                runner
                    .storage
                    .get_player(id)
                    .map(|plr| plr.display_name())
                    .unwrap_or_else(|| format!("#{id}"))
            })
            .collect::<Vec<String>>()
            .join(",")
    };
    msg.replace("[2]", &param)
}

fn core_caster_name(runner: &tswn_core::Runner, update: &RunUpdate) -> String {
    runner
        .storage
        .get_player(&update.caster)
        .map(|plr| plr.display_name())
        .unwrap_or_else(|| format!("#{}", update.caster))
}

impl EngineAdapter for CoreEngine {
    type Runner = tswn_core::Runner;

    fn new_from_raw(raw: String) -> Result<Self::Runner, String> {
        tswn_core::Runner::new_from_namerena_raw(raw).map_err(|err| err.to_string())
    }

    fn main_round(runner: &mut Self::Runner) -> Vec<EventSnapshot> {
        runner
            .main_round()
            .updates
            .into_iter()
            .map(|update| {
                let kind = match update.update_type {
                    UpdateType::Win => SnapshotKind::Win,
                    UpdateType::NextLine => SnapshotKind::NextLine,
                    UpdateType::None => SnapshotKind::Event,
                };
                EventSnapshot {
                    message: format_core_update_message(runner, &update),
                    caster_name: core_caster_name(runner, &update),
                    score: update.score,
                    kind,
                }
            })
            .collect()
    }

    fn have_winner(runner: &Self::Runner) -> bool { runner.have_winner() }

    fn winner_names(runner: &Self::Runner) -> Vec<String> {
        runner
            .world
            .winner
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|id| {
                runner
                    .storage
                    .get_player(&id)
                    .map(|plr| plr.id_name())
                    .unwrap_or_else(|| format!("#{id}"))
            })
            .collect::<Vec<String>>()
    }

    fn rc4_state(runner: &Self::Runner) -> Option<(usize, usize)> {
        Some((runner.randomer.i as usize, runner.randomer.j as usize))
    }
}

fn normalize_trace_line(line: String) -> String {
    line.replace("[s_counter]", "")
        .replace("[s_dmg160]", "")
        .replace("[s_dmg120]", "")
        .replace("[s_dmg0]", "")
        .replace(['[', ']'], "")
        .replace(' ', "")
        .trim()
        .to_string()
}

fn collect_replay_events<E: EngineAdapter>(
    runner: &mut E::Runner,
    max_rounds: usize,
    normalize: bool,
) -> (Vec<String>, usize, u64) {
    let mut events = Vec::new();
    let mut guard = 0usize;
    let mut total_score = 0u64;
    while !E::have_winner(runner) && guard < max_rounds {
        let updates = E::main_round(runner);
        for update in updates {
            if matches!(update.kind, SnapshotKind::NextLine) {
                continue;
            }
            if update.score > 0 {
                total_score += update.score as u64;
            }
            let mut msg = update.message;
            if normalize {
                msg = normalize_trace_line(msg);
                if msg.is_empty() {
                    continue;
                }
            }
            events.push(msg);
        }
        guard += 1;
    }
    (events, guard, total_score)
}

pub fn collect_replay_lines<E: EngineAdapter>(
    runner: &mut E::Runner,
    max_rounds: usize,
    normalize: bool,
) -> (Vec<String>, usize, u64) {
    let mut lines = Vec::new();
    let mut guard = 0usize;
    let mut total_score = 0u64;
    let track_rc4 = std::env::var_os("TSWN_TRACK_RC4").is_some();
    let track_rc4_range: Option<(usize, usize)> = std::env::var("TSWN_TRACK_RC4").ok().and_then(|v| {
        let parts: Vec<&str> = v.split(',').collect();
        if parts.len() == 2 {
            Some((parts[0].parse().ok()?, parts[1].parse().ok()?))
        } else {
            None
        }
    });
    while !E::have_winner(runner) && guard < max_rounds {
        let in_range = track_rc4_range
            .map(|(lo, hi)| lines.len() >= lo && lines.len() <= hi)
            .unwrap_or(track_rc4 && lines.len() >= 250 && lines.len() <= 260);
        if in_range {
            if let Some((i, j)) = E::rc4_state(runner) {
                eprintln!("[rc4_track] before_round line_count={} rc4=({}, {})", lines.len(), i, j);
            }
        }
        let updates = E::main_round(runner);
        let mut parts = Vec::new();
        for update in updates {
            if matches!(update.kind, SnapshotKind::NextLine) {
                if !parts.is_empty() {
                    lines.push(parts.join(", "));
                    parts.clear();
                }
                continue;
            }
            if update.score > 0 {
                total_score += update.score as u64;
            }
            let mut msg = update.message;
            if normalize {
                msg = normalize_trace_line(msg);
            }
            if !msg.is_empty() {
                parts.push(msg);
            }
        }
        if !parts.is_empty() {
            lines.push(parts.join(", "));
        }
        guard += 1;
    }
    (lines, guard, total_score)
}

fn collect_battle_scores<E: EngineAdapter>(runner: &mut E::Runner, max_rounds: usize) -> (u64, HashMap<String, u64>) {
    let mut total_score = 0u64;
    let mut score_by_name: HashMap<String, u64> = HashMap::new();
    let mut guard = 0usize;
    while !E::have_winner(runner) && guard < max_rounds {
        let updates = E::main_round(runner);
        for update in updates {
            if matches!(update.kind, SnapshotKind::NextLine) || update.score == 0 {
                continue;
            }
            total_score += update.score as u64;
            *score_by_name.entry(update.caster_name).or_insert(0) += update.score as u64;
        }
        guard += 1;
    }
    (total_score, score_by_name)
}

fn should_dump_full_trace(case_name: &str) -> bool {
    let Some(raw) = std::env::var_os("TSWN_DUMP_TRACE") else {
        return false;
    };
    let raw = raw.to_string_lossy();
    let filter = raw.trim();
    filter.is_empty() || filter == "1" || case_name.contains(filter)
}

fn dump_full_trace(label: &str, lines: &[String]) {
    eprintln!("{label} full trace (len={}):", lines.len());
    for (idx, line) in lines.iter().enumerate() {
        eprintln!("  idx={idx}: {line}");
    }
}

fn parse_embedded_fight_case(case_text: &str, split_err: &str, empty_err: &str) -> (String, Vec<String>) {
    let fight_text = case_text.replace("\r\n", "\n").replace('\r', "\n");
    let (raw_input, expected_part) = fight_text.split_once("\n\n\n").expect(split_err);
    let expected_lines = expected_part
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| normalize_trace_line(line.to_string()))
        .filter(|line| !line.is_empty())
        .collect::<Vec<String>>();
    assert!(!expected_lines.is_empty(), "{empty_err}");
    (raw_input.trim_end().to_string(), expected_lines)
}

pub fn winner_names<E: EngineAdapter>(runner: &E::Runner) -> Vec<String> { E::winner_names(runner) }

fn assert_trace_with_context(case_name: &str, actual_lines: &[String], expected_lines: &[String]) {
    if actual_lines == expected_lines {
        return;
    }
    if should_dump_full_trace(case_name) {
        dump_full_trace(&format!("{case_name} actual"), actual_lines);
        dump_full_trace(&format!("{case_name} expected"), expected_lines);
    }
    let min_len = actual_lines.len().min(expected_lines.len());
    let mismatch_idx = actual_lines
        .iter()
        .zip(expected_lines.iter())
        .position(|(lhs, rhs)| lhs != rhs)
        .unwrap_or(min_len);
    let ctx_start = mismatch_idx.saturating_sub(3);
    let ctx_end = (mismatch_idx + 3).min(min_len);
    eprintln!("{case_name} mismatch context [{ctx_start}..{ctx_end}):");
    for idx in ctx_start..ctx_end {
        eprintln!(
            "  idx={idx}: actual={:?} | expected={:?}",
            actual_lines.get(idx),
            expected_lines.get(idx)
        );
    }
    panic!(
        "{case_name} mismatch at idx={mismatch_idx}, actual_len={}, expected_len={}, actual={:?}, expected={:?}",
        actual_lines.len(),
        expected_lines.len(),
        actual_lines.get(mismatch_idx),
        expected_lines.get(mismatch_idx)
    );
}

fn strip_name_noise_suffix(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    'scan: while i < bytes.len() {
        if bytes[i] == b'?' {
            for marker in [b"clone".as_slice(), b"summon".as_slice()] {
                let marker_start = i + 1;
                let marker_end = marker_start + marker.len();
                if marker_end <= bytes.len() && &bytes[marker_start..marker_end] == marker {
                    let mut j = marker_end;
                    while j < bytes.len() && bytes[j].is_ascii_digit() {
                        j += 1;
                    }
                    if j > marker_end {
                        i = j;
                        continue 'scan;
                    }
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).expect("stripping ASCII suffix should keep valid UTF-8")
}

fn assert_trace_with_name_noise_ignored(case_name: &str, actual_lines: &[String], expected_lines: &[String]) {
    let normalized_actual = actual_lines.iter().map(|line| strip_name_noise_suffix(line)).collect::<Vec<String>>();
    let normalized_expected = expected_lines.iter().map(|line| strip_name_noise_suffix(line)).collect::<Vec<String>>();
    assert_trace_with_context(case_name, &normalized_actual, &normalized_expected);
}

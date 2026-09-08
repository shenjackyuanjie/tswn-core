//! 共用回放测试套件。
//!
//! 本模块中的 fixture 不依赖具体引擎，只读取原始输入、渲染后的更新快照、胜者和分数。

use std::collections::HashMap;

use crate::{EngineAdapter, EventSnapshot, RuntimeEngine, SnapshotKind};
use tswn_core::runtime::update::{RunUpdate, UpdateType};
use tswn_core::runtime::{EntityIdx, RuntimeRunner, default_custom_runtime_import_config};

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

fn runtime_entity_name(runner: &RuntimeRunner, id: usize) -> String {
    u32::try_from(id)
        .ok()
        .and_then(|id| runner.runtime().entities.get(EntityIdx(id)))
        .map(|entity| entity.template.display_name.clone())
        .unwrap_or_else(|| format!("#{id}"))
}

fn format_runtime_update_message(runner: &RuntimeRunner, update: &RunUpdate) -> String {
    let caster = runtime_entity_name(runner, update.caster);
    let target = runtime_entity_name(runner, update.target);
    let mut msg = update.message.to_string();
    msg = msg.replace("[0]", &caster);
    msg = msg.replace("[1]", &target);
    let param = if let Some(param) = update.param {
        param.to_string()
    } else if update.targets.is_empty() {
        update.score.to_string()
    } else {
        update
            .targets
            .iter()
            .map(|id| runtime_entity_name(runner, *id))
            .collect::<Vec<_>>()
            .join(",")
    };
    msg.replace("[2]", &param)
}

impl EngineAdapter for RuntimeEngine {
    type Runner = RuntimeRunner;

    fn new_from_raw(raw: String) -> Result<Self::Runner, String> {
        let config = default_custom_runtime_import_config().map_err(|err| format!("{err:?}"))?;
        RuntimeRunner::from_custom_mixed_namerena_raw(raw, config).map_err(|err| format!("{err:?}"))
    }

    fn main_round(runner: &mut Self::Runner) -> Vec<EventSnapshot> {
        let outcome = runner.run_round();
        let Some(frame) = outcome.frame else {
            return Vec::new();
        };
        frame
            .updates
            .updates
            .into_iter()
            .map(|update| {
                let kind = match update.update_type {
                    UpdateType::Win => SnapshotKind::Win,
                    UpdateType::NextLine => SnapshotKind::NextLine,
                    UpdateType::None => SnapshotKind::Event,
                };
                EventSnapshot {
                    message: format_runtime_update_message(runner, &update),
                    caster_name: runtime_entity_name(runner, update.caster),
                    score: update.score,
                    kind,
                }
            })
            .collect()
    }

    fn have_winner(runner: &Self::Runner) -> bool { runner.runtime().world.winner_team().is_some() }

    fn winner_names(runner: &Self::Runner) -> Vec<String> {
        let Some(team) = runner.runtime().world.winner_team() else {
            return Vec::new();
        };
        runner
            .runtime()
            .world
            .team_alive(team)
            .unwrap_or_default()
            .iter()
            .filter_map(|entity| runner.runtime().entities.get(*entity))
            .map(|entity| entity.template.name.clone())
            .collect()
    }

    fn winner_team_index(runner: &Self::Runner) -> Option<usize> { runner.runtime().world.winner_team() }

    fn rc4_state(runner: &Self::Runner) -> Option<(usize, usize)> {
        Some((runner.runtime().rng.i as usize, runner.runtime().rng.j as usize))
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

pub fn assert_runtime_matches_frozen_golden(raw: &str, case_name: &str) {
    crate::golden::assert_runtime_matches_frozen_golden(raw, case_name);
}

pub fn assert_runtime_matches_frozen_golden_with_eval_rq(raw: &str, case_name: &str, eval_rq: f64) {
    crate::golden::assert_runtime_matches_frozen_golden_with_eval_rq(raw, case_name, eval_rq);
}

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

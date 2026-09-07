//! 规范战斗流的人类和 JSONL 消费端。
use serde::Serialize;
use std::{
    collections::HashMap,
    error::Error,
    io::{self, Write},
};
use tswn_core::cli_api::battle::{BattleOptions, BattlePlayerState, BattleReplayFrame, BattleResult, BattleSession};

type OutputResult = Result<(), Box<dyn Error>>;

#[derive(Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
enum BattleEvent<'a> {
    Initial(&'a [BattlePlayerState]),
    Frame(&'a BattleReplayFrame),
    Result(&'a BattleResult),
}

fn write_event(out: &mut impl Write, event: BattleEvent<'_>) -> OutputResult {
    serde_json::to_writer(&mut *out, &event)?;
    out.write_all(b"\n")?;
    out.flush()?;
    Ok(())
}

pub fn run(raw: String, jsonl: bool, max_rounds: usize) {
    let result = BattleSession::new(
        &raw,
        BattleOptions {
            max_rounds,
            ..BattleOptions::default()
        },
    )
    .map_err(|error| Box::new(error) as Box<dyn Error>)
    .and_then(|mut session| write_battle(&mut session, &mut io::stdout().lock(), jsonl));
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

pub fn run_diff(raw: String) { super::runtime::run_runtime_diff(raw); }

fn write_battle(session: &mut BattleSession, out: &mut impl Write, jsonl: bool) -> OutputResult {
    if jsonl {
        write_event(out, BattleEvent::Initial(session.initial_states()))?;
    } else {
        writeln!(out, "=== 玩家状态 ===")?;
        for state in session.initial_states() {
            writeln!(
                out,
                "- {} (id={}): HP={}/{}, ATK={}, DEF={}, SPD={}, AGI={}, MAG={}, MP={}, MDF={}, ITL={}, all_sum={} 系数: {}",
                state.display_name,
                state.id,
                state.hp,
                state.max_hp,
                state.attack,
                state.defense,
                state.speed,
                state.agility,
                state.magic,
                state.magic_point,
                state.resistance,
                state.wisdom,
                state.all_sum,
                state.name_factor
            )?;
        }
        out.flush()?;
    }
    let mut scores = HashMap::<usize, u64>::new();
    while let Some(frame) = session.next_frame()? {
        if jsonl {
            write_event(out, BattleEvent::Frame(&frame))?;
        } else {
            writeln!(out, "\n=== 回合 {} ===", frame.round_index + 1)?;
            for row in &frame.rows {
                let text = row
                    .clips
                    .iter()
                    .flat_map(|clip| &clip.parts)
                    .map(|part| part.text.as_str())
                    .collect::<String>();
                writeln!(out, "{}{text}", if row.indent { "  " } else { "" })?;
            }
            for update in &frame.updates {
                if let Some(caster) = update.caster_id {
                    *scores.entry(caster).or_default() += u64::from(update.score);
                }
            }
            out.flush()?;
        }
    }
    let result = session.result().ok_or("session ended without a result")?;
    if jsonl {
        write_event(out, BattleEvent::Result(&result))?;
    } else {
        writeln!(out, "\n=== 对局结果 ===")?;
        if result.finished {
            writeln!(out, "赢家:")?;
            for state in result.final_states.iter().filter(|state| result.winner_ids.contains(&state.id)) {
                writeln!(
                    out,
                    "- {} (id={}, all_sum={}, battle_score={}, hp={})",
                    state.display_name,
                    state.id,
                    state.all_sum,
                    scores.get(&state.id).copied().unwrap_or(0),
                    state.hp
                )?;
            }
        } else {
            writeln!(out, "对局截断: {:?}（推进 {} 轮）", result.stop_reason, result.rounds_advanced)?;
        }
        writeln!(out, "总战斗分: {}", scores.values().sum::<u64>())?;
        out.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct LineWriter {
        pending: Vec<u8>,
        events: Vec<serde_json::Value>,
    }
    impl Write for LineWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.pending.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            assert_eq!(
                self.pending.iter().filter(|byte| **byte == b'\n').count(),
                1,
                "flush exactly one JSON line"
            );
            self.events.push(serde_json::from_slice(&self.pending).unwrap());
            self.pending.clear();
            Ok(())
        }
    }

    #[test]
    fn jsonl_flushes_each_canonical_payload_in_order() {
        let raw = "left@red\n\nright@blue\nseed:42@!";
        for max_rounds in [1, 20_000] {
            let options = BattleOptions {
                max_rounds,
                ..BattleOptions::default()
            };
            let replay = tswn_core::cli_api::battle::battle_replay(raw, options).unwrap();
            let mut session = BattleSession::new(raw, options).unwrap();
            let mut writer = LineWriter::default();
            write_battle(&mut session, &mut writer, true).unwrap();
            assert!(writer.pending.is_empty());
            assert_eq!(writer.events.len(), replay.frames.len() + 2);
            assert_eq!(
                writer.events[0],
                serde_json::json!({"type": "initial", "data": replay.initial_states})
            );
            for (event, frame) in writer.events[1..writer.events.len() - 1].iter().zip(&replay.frames) {
                assert_eq!(event, &serde_json::json!({"type": "frame", "data": frame}));
            }
            assert_eq!(
                writer.events.last().unwrap(),
                &serde_json::json!({"type": "result", "data": session.result().unwrap()})
            );
        }
    }

    #[test]
    fn output_failure_stops_before_advancing_runtime() {
        struct BrokenWriter;
        impl Write for BrokenWriter {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
            fn flush(&mut self) -> io::Result<()> { Ok(()) }
        }
        let mut session = BattleSession::new("a\n\nb", BattleOptions::default()).unwrap();
        assert!(write_battle(&mut session, &mut BrokenWriter, true).is_err());
        assert_eq!(session.rounds_advanced(), 0);
        assert!(session.result().is_none());
    }

    #[test]
    fn human_fight_prints_canonical_rows_and_explicit_truncation() {
        let mut session = BattleSession::new(
            "left@red\n\nright@blue",
            BattleOptions {
                max_rounds: 1,
                ..BattleOptions::default()
            },
        )
        .unwrap();
        let mut output = Vec::new();
        write_battle(&mut session, &mut output, false).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("=== 玩家状态 ==="));
        assert!(text.contains("=== 回合 1 ==="));
        assert!(text.contains("对局截断: MaxRounds"));
        assert_eq!(session.rounds_advanced(), 1);
    }
}

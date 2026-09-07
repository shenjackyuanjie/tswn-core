//! Cross-module canonical session/replay contracts using frozen Runtime inputs.
use crate::golden::STRESS_CASES;
use std::collections::HashSet;
use tswn_core::cli_api::battle::{BattleOptions, BattleSession, battle_replay};

#[test]
fn battle_session_replay_parity_on_frozen_corpus() {
    let mut minions = HashSet::new();
    let mut messages = String::new();
    let mut saw_round_gap = false;
    for case in STRESS_CASES {
        let raw = case.effective_input();
        let options = BattleOptions {
            eval_rq: case.eval_rq,
            ..BattleOptions::default()
        };
        let replay = battle_replay(&raw, options).unwrap();
        let mut session = BattleSession::new(&raw, options).unwrap();
        assert_eq!(session.initial_states(), replay.initial_states, "{} initial", case.file_name);
        for expected in &replay.frames {
            let actual = session.next_frame().unwrap().expect(case.file_name);
            assert_eq!(&actual, expected, "{} frame {}", case.file_name, expected.frame_index);
            saw_round_gap |= actual.frame_index != actual.round_index;
            for state in &actual.states {
                if let Some(kind) = state.minion_kind {
                    minions.insert(kind);
                }
            }
            for update in &actual.updates {
                messages.push_str(&update.message_template);
            }
        }
        assert!(session.next_frame().unwrap().is_none());
        let result = session.result().unwrap();
        assert_eq!(result.final_states, replay.final_states, "{} final", case.file_name);
        assert_eq!(result.status, replay.status);
        assert_eq!(result.stop_reason, replay.stop_reason);
        assert_eq!(result.winner_ids, replay.winner_ids);
        assert_eq!(result.winner_team_indices, replay.winner_team_indices);
        assert_eq!(result.rounds_advanced, replay.rounds_advanced);
        assert_eq!(result.frames_emitted, replay.frames_emitted);
        assert_eq!(replay.state_granularity, "round");
    }
    for kind in ["clone", "summon", "shadow", "zombie"] {
        assert!(minions.contains(kind), "missing {kind} fixture coverage");
    }
    for token in ["苏生", "守护", "魅惑"] {
        assert!(messages.contains(token), "missing {token} fixture coverage");
    }
    assert!(saw_round_gap, "fixtures must exercise empty Runtime rounds");
}

#[test]
fn battle_session_replay_parity_with_round_budget_and_icons() {
    let raw = STRESS_CASES[0].effective_input();
    for max_rounds in [1, 3, 20] {
        let options = BattleOptions {
            max_rounds,
            include_icons: true,
            ..BattleOptions::default()
        };
        let replay = battle_replay(&raw, options).unwrap();
        let mut session = BattleSession::new(&raw, options).unwrap();
        let mut frames = Vec::new();
        while let Some(frame) = session.next_frame().unwrap() {
            frames.push(frame);
        }
        assert_eq!(frames, replay.frames);
        assert_eq!(session.result().unwrap().final_states, replay.final_states);
        assert!(session.rounds_advanced() <= max_rounds);
    }
}

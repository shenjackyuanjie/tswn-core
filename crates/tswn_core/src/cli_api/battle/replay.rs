use super::{
    dto::*,
    session::{build_frame, states_from_runner},
};
use crate::cli_api::{CliApiError, CliApiResult, invalid_input};
use crate::runtime::RuntimeRunner;
use std::collections::HashMap;

pub fn battle_replay(raw: &str, options: BattleOptions) -> CliApiResult<BattleReplay> {
    if raw.trim().is_empty() {
        return Err(invalid_input("raw_input is empty"));
    }
    if options.max_rounds == 0 {
        return Err(invalid_input("battle_replay max_rounds must be positive"));
    }

    let (groups, seed) = RuntimeRunner::split_namerena_into_groups(raw.to_owned());
    let mut runner = RuntimeRunner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, options.eval_rq)
        .map_err(|err| CliApiError::RunnerInit(err.to_string()))?;
    let mut icon_cache = HashMap::new();
    let initial_states = states_from_runner(&runner, options.include_icons, &mut icon_cache);
    let mut previous_states = initial_states.clone();
    let mut frames = Vec::new();
    let mut idle_rounds = 0usize;
    let mut rounds_advanced = 0usize;

    while !runner.have_winner() && frames.len() < options.max_rounds {
        let updates = runner.main_round();
        rounds_advanced += 1;
        if updates.updates.is_empty() {
            idle_rounds += 1;
            if idle_rounds > 16usize.saturating_mul(runner.all_player_ids().len().max(1)) {
                break;
            }
            previous_states = states_from_runner(&runner, options.include_icons, &mut icon_cache);
            continue;
        }
        idle_rounds = 0;
        let states = states_from_runner(&runner, options.include_icons, &mut icon_cache);
        let mut frame = build_frame(&updates, &previous_states, &states, &runner);
        frame.frame_index = frames.len();
        frame.round_index = rounds_advanced - 1;
        frames.push(frame);
        previous_states = states;
    }

    let finished = runner.have_winner();
    Ok(BattleReplay {
        status: if finished {
            BattleStatus::Finished
        } else {
            BattleStatus::Truncated
        },
        stop_reason: if finished {
            BattleStopReason::Winner
        } else if frames.len() >= options.max_rounds {
            BattleStopReason::MaxRounds
        } else {
            BattleStopReason::NoProgress
        },
        rounds_advanced,
        frames_emitted: frames.len(),
        finished,
        truncated: !finished,
        initial_states,
        frames,
        final_states: states_from_runner(&runner, options.include_icons, &mut icon_cache),
        winner_ids: runner.winner_ids(),
        winner_team_indices: runner.winner_team_indices(),
        state_granularity: "round",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battle_replay_returns_render_ready_frames() {
        let replay = battle_replay("left@red\n\nright@blue\n", BattleOptions::default()).unwrap();

        assert!(replay.finished);
        assert!(!replay.frames.is_empty());
        assert_eq!(replay.initial_states.len(), 2);
        assert!(!replay.winner_ids.is_empty());
        assert!(replay.frames.iter().any(|frame| !frame.rows.is_empty()));
    }

    #[test]
    fn battle_replay_rejects_zero_max_rounds() {
        let options = BattleOptions {
            max_rounds: 0,
            ..BattleOptions::default()
        };
        let err = battle_replay("left\n\nright", options).unwrap_err();
        assert_eq!(err.to_string(), "battle_replay max_rounds must be positive");
    }
}

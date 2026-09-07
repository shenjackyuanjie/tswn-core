use super::{BattleSession, dto::*};
use crate::cli_api::{CliApiError, CliApiResult};

/// Collect the canonical session without introducing another execution policy.
pub fn battle_replay(raw: &str, options: BattleOptions) -> CliApiResult<BattleReplay> {
    let mut session = BattleSession::new(raw, options)?;
    let initial_states = session.initial_states().to_vec();
    let mut frames = Vec::new();
    while let Some(frame) = session.next_frame()? {
        frames.push(frame);
    }
    let result = session
        .result()
        .ok_or_else(|| CliApiError::Internal("session ended without a result".into()))?;
    Ok(BattleReplay {
        status: result.status,
        stop_reason: result.stop_reason,
        rounds_advanced: result.rounds_advanced,
        frames_emitted: result.frames_emitted,
        finished: result.finished,
        truncated: result.truncated,
        initial_states,
        frames,
        final_states: result.final_states,
        winner_ids: result.winner_ids,
        winner_team_indices: result.winner_team_indices,
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
        assert_eq!(err.to_string(), "battle max_rounds must be positive");
    }
}

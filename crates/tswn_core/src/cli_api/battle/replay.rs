use super::{BattleSession, dto::*};
use crate::cli_api::{CliApiError, CliApiResult};

/// 收集规范会话，但不引入另一套执行策略。
pub fn battle_replay(raw: &str, options: BattleOptions) -> CliApiResult<BattleReplay> {
    crate::runtime::forced_action::set_controlled_team(options.controlled_team);
    let result = battle_replay_inner(raw, options);
    crate::runtime::forced_action::set_controlled_team(None);
    result
}

fn battle_replay_inner(raw: &str, options: BattleOptions) -> CliApiResult<BattleReplay> {
    let mut session = BattleSession::new(raw, options)?;
    let initial_states = session.initial_states().to_vec();
    let mut frames = Vec::new();
    while let Some(frame) = session.next_frame()? {
        frames.push(frame);
    }
    // 玩家单位把战斗逼停：不是终局，回放截到这里，等前端下指令后重新模拟。
    if let Some(paused_actor) = session.paused_actor() {
        return Ok(BattleReplay {
            paused_actor: Some(paused_actor as usize),
            status: BattleStatus::Running,
            stop_reason: BattleStopReason::Paused,
            rounds_advanced: session.rounds_advanced(),
            frames_emitted: session.frames_emitted(),
            finished: false,
            truncated: false,
            initial_states,
            frames,
            final_states: session.current_states().to_vec(),
            winner_ids: Vec::new(),
            winner_team_indices: Vec::new(),
            state_granularity: "round",
        });
    }
    let result = session
        .result()
        .ok_or_else(|| CliApiError::Internal("session ended without a result".into()))?;
    Ok(BattleReplay {
        paused_actor: None,
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

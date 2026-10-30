use super::{BattleSession, dto::*};
use crate::cli_api::{CliApiError, CliApiResult};
use crate::runtime::PlannedAction;

/// 收集规范会话，但不引入另一套执行策略。
pub fn battle_replay(raw: &str, options: BattleOptions) -> CliApiResult<BattleReplay> {
    battle_replay_with_actions(raw, options, &[])
}

/// 用外部行动脚本收集回放：按脚本执行每个实体的行动，受控单位缺指令时在此停下。
///
/// 脚本用尽后受控队伍（`options.controlled_team`）的本体与分身会逼停回放，
/// 此时返回的 `paused_actor` 指出等待指令的实体，`stop_reason` 为 `Paused`，
/// 但 `status` 仍是 running、`finished`/`truncated` 都是 false。调用方追加指令后
/// 重新调用即可；同一份脚本的重复模拟结果确定。
pub fn battle_replay_with_actions(raw: &str, options: BattleOptions, actions: &[PlannedAction]) -> CliApiResult<BattleReplay> {
    let mut session = BattleSession::new_with_actions(raw, options, actions)?;
    let initial_states = session.initial_states().to_vec();
    let mut frames = Vec::new();
    while let Some(frame) = session.next_frame()? {
        frames.push(frame);
    }
    if let Some(paused_actor) = session.paused_actor() {
        return Ok(BattleReplay {
            paused_actor: Some(paused_actor),
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
    use crate::runtime::{BuiltinActiveSkill, PlannedSkill};

    fn controlled_options() -> BattleOptions {
        BattleOptions {
            controlled_team: Some(0),
            max_rounds: 20,
            ..BattleOptions::default()
        }
    }

    #[test]
    fn battle_replay_returns_render_ready_frames() {
        let replay = battle_replay("left@red\n\nright@blue\n", BattleOptions::default()).unwrap();

        assert!(replay.finished);
        assert!(!replay.frames.is_empty());
        assert_eq!(replay.initial_states.len(), 2);
        assert!(!replay.winner_ids.is_empty());
        assert!(replay.frames.iter().any(|frame| !frame.rows.is_empty()));
        assert_eq!(replay.paused_actor, None);
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

    #[test]
    fn empty_script_matches_automatic_replay() {
        let raw = "left@red\n\nright@blue\nseed:auto@!";
        let automatic = battle_replay(raw, BattleOptions::default()).unwrap();
        let scripted = battle_replay_with_actions(raw, BattleOptions::default(), &[]).unwrap();
        assert_eq!(automatic, scripted);
    }

    #[test]
    fn controlled_unit_pauses_until_it_gets_an_action() {
        let raw = "left@red\n\nright@blue\nseed:pause@!";
        let paused = battle_replay_with_actions(raw, controlled_options(), &[]).unwrap();
        assert_eq!(paused.stop_reason, BattleStopReason::Paused);
        assert_eq!(paused.status, BattleStatus::Running);
        assert!(!paused.finished && !paused.truncated);
        let actor = paused.paused_actor.expect("受控单位应当逼停回放");

        // 给暂停者排一次普攻：这一手要真的执行，然后再次停在下一次需要指令时。
        let replay = battle_replay_with_actions(
            raw,
            controlled_options(),
            &[PlannedAction {
                actor,
                skill: PlannedSkill::BasicAttack,
                magic_cost: None,
            }],
        )
        .unwrap();
        assert!(
            replay
                .frames
                .iter()
                .flat_map(|frame| &frame.updates)
                .any(|update| update.caster_id == Some(actor)),
            "排好的普攻没有出现在回放里"
        );
        assert_eq!(replay.paused_actor, Some(actor));
    }

    #[test]
    fn unusable_planned_action_is_rejected_instead_of_falling_back() {
        let raw = r#"left@red+ol:{"skills":{"fire":50}}

right@blue+ol:{"skills":{}}"#;
        let paused = battle_replay_with_actions(raw, controlled_options(), &[]).unwrap();
        let actor = paused.paused_actor.expect("受控单位应当逼停回放");

        // 该单位只装备了火球术，指令要求血祭（Summon）时应当报错，而不是静默改成普攻。
        let error = battle_replay_with_actions(
            raw,
            controlled_options(),
            &[PlannedAction {
                actor,
                skill: PlannedSkill::Builtin(BuiltinActiveSkill::Summon),
                magic_cost: Some(8),
            }],
        )
        .unwrap_err();
        assert_eq!(error.code(), crate::cli_api::CliApiErrorCode::InvalidArgument);
        assert!(error.to_string().contains("has not equipped"), "{error}");
    }

    #[test]
    fn planned_actor_must_exist() {
        let error = battle_replay_with_actions(
            "left@red\n\nright@blue",
            controlled_options(),
            &[PlannedAction {
                actor: 99,
                skill: PlannedSkill::BasicAttack,
                magic_cost: None,
            }],
        )
        .unwrap_err();
        assert_eq!(error.code(), crate::cli_api::CliApiErrorCode::InvalidArgument);
    }

    #[test]
    fn controlled_team_must_match_an_entity() {
        let options = BattleOptions {
            controlled_team: Some(9),
            ..BattleOptions::default()
        };
        let error = battle_replay("left@red\n\nright@blue", options).unwrap_err();
        assert_eq!(error.code(), crate::cli_api::CliApiErrorCode::InvalidArgument);
        assert!(error.to_string().contains("controlled_team 9"), "{error}");
    }
}

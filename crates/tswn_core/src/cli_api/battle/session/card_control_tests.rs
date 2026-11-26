// 覆盖实际暂停续行和多实体药水失效，不冻结源码或静态配置。
use super::*;
use crate::runtime::forced_action;
const RAW: &str = "one@red+ol:{\"attrs\":[40,40,140,40,40,40,100,20000],\"skills\":{},\"name_factor_enabled\":false}\ntwo@red+ol:{\"attrs\":[40,40,140,40,40,40,100,20000],\"skills\":{},\"name_factor_enabled\":false}\n\nenemy@blue+ol:{\"attrs\":[10,40,100,40,40,40,100,20000],\"skills\":{},\"name_factor_enabled\":false}\nseed:card-regression@!";
fn options() -> BattleOptions {
    BattleOptions {
        controlled_team: Some(0),
        max_rounds: 2000,
        ..BattleOptions::default()
    }
}
fn pause(session: &mut BattleSession) -> Vec<BattleReplayFrame> {
    let mut frames = Vec::new();
    while let Some(frame) = session.next_frame().unwrap() {
        frames.push(frame);
    }
    assert!(session.paused_actor().is_some());
    assert_eq!(session.stop_reason(), Some(BattleStopReason::Paused));
    assert!(!session.is_done());
    frames
}
// 双方本体轮流停在同一增量会话，地图当前 HP 不改变最大 HP，非法整批不产生部分修改。
#[test]
fn pvp_controls_both_sides_and_preserves_initial_max_hp() {
    let mut session = BattleSession::new(
        RAW,
        BattleOptions {
            controlled_team: Some(u32::MAX as usize),
            ..options()
        },
    )
    .unwrap();
    let ids: Vec<u32> = session.initial_states().iter().map(|s| s.id as u32).collect();
    session.set_initial_hp(&[ids[0], 77, ids[1], 88]).unwrap();
    assert_eq!(
        session.initial_states().iter().find(|s| s.id == ids[0] as usize).unwrap().hp,
        77
    );
    assert_eq!(
        session.initial_states().iter().find(|s| s.id == ids[0] as usize).unwrap().max_hp,
        20000
    );
    assert!(session.set_initial_hp(&[ids[0], 55, ids[1], 0]).is_err());
    assert_eq!(
        session.initial_states().iter().find(|s| s.id == ids[0] as usize).unwrap().hp,
        77
    );
    let mut teams = std::collections::HashSet::new();
    for _ in 0..8 {
        pause(&mut session);
        let actor = session.paused_actor().unwrap();
        teams.insert(session.current_states().iter().find(|s| s.id == actor).unwrap().team_index);
        assert!(session.set_initial_hp(&[actor as u32, 100]).is_err());
        session.submit_action(actor, 255, 0).unwrap();
    }
    assert_eq!(teams.len(), 2);
}

// 真正暂停的运行时处于负 MP 时，普通攻击仍须产生战报并继续到下一次决策。
#[test]
fn normal_attack_advances_with_negative_mp() {
    for mp in [-1, -20, -200] {
        let mut session = BattleSession::new(RAW, options()).unwrap();
        pause(&mut session);
        let actor = session.paused_actor().unwrap();
        session
            .driver
            .runner
            .runtime_mut()
            .entities
            .get_mut(crate::runtime::EntityIdx(actor as u32))
            .unwrap()
            .runtime
            .magic_point = mp;
        session.submit_action(actor, 255, 0).unwrap();
        let frames = pause(&mut session);
        assert!(!frames.is_empty(), "negative MP {mp} must still advance a normal attack");
        assert_eq!(
            session
                .driver
                .runner
                .runtime()
                .entities
                .get(crate::runtime::EntityIdx(actor as u32))
                .unwrap()
                .runtime
                .magic_point,
            mp + 16,
            "normal attack must cost zero MP and recover normally"
        );
    }
}

// 兼容模式选不出目标时暂停并保留这一手：整回合回滚，行动条与随机流不被消耗。
#[test]
fn fallback_mode_pauses_without_dropping_a_targetless_action() {
    // 两组都学会治疗，受控方无论落在哪一组都能走到目标选择。
    let raw = r#"left@red+ol:{"skills":{"heal":50},"name_factor_enabled":false}

right@blue+ol:{"skills":{"heal":50},"name_factor_enabled":false}"#;
    let mut session = BattleSession::new(raw, options()).unwrap();
    pause(&mut session);
    let actor = session.paused_actor().unwrap();
    // 治疗只挑受伤的队友；先把受控方补满，目标口径就必然选不出目标。
    let entity = session.driver.runner.runtime_mut().entities.get_mut(EntityIdx(actor as u32)).unwrap();
    let max_hp = entity.template.max_hp;
    entity.runtime.hp = max_hp;
    // 15 是公开编号里的治疗。
    session.submit_action(actor, 15, 1).unwrap();
    forced_action::with_fallback_mode(forced_action::ForcedFallback::Fallback, || pause(&mut session));
    assert_eq!(session.paused_actor(), Some(actor), "选不出目标时应当暂停在原位");
    // 暂停那回合整段回滚，这一手没被消耗：调用方仍能从队列里读到它。
    assert_eq!(
        session.driver.runner.runtime().control.pending(),
        1,
        "暂停后这一手必须还在队列里"
    );
}

// 普攻不得掷技能耗蓝骰；只允许目标选择本身消耗随机流。
#[test]
fn normal_attack_skips_skill_budget_rng() {
    use crate::runtime::{EntityIdx, PlainSkillPreActionOutcome};
    let mut session = BattleSession::new(RAW, options()).unwrap();
    pause(&mut session);
    let actor = EntityIdx(session.paused_actor().unwrap() as u32);
    let runtime = session.driver.runner.runtime_mut();
    runtime.entities.get_mut(actor).unwrap().runtime.magic_point = -20;
    let mut reference = runtime.clone();
    reference
        .select_plain_default_attack_target(actor, true)
        .or_else(|| reference.select_plain_default_attack_target(actor, false));
    let mut forced = forced_action::ForcedState::default();
    forced.controlled_team = Some(0);
    runtime.control.queue_all(crate::runtime::decode_actions(&[actor.0, 255, 0]).unwrap());
    let action = forced_action::with_state(&mut forced, || {
        runtime.prepare_plain_action(actor, true, PlainSkillPreActionOutcome::default())
    });
    assert!(action.is_some());
    assert_eq!(
        (runtime.rng.i, runtime.rng.j, runtime.rng.main_val),
        (reference.rng.i, reference.rng.j, reference.rng.main_val)
    );
}

#[test]
fn streaming_resume_matches_batch_and_potions_expire_per_actor() {
    let mut session = BattleSession::new(RAW, options()).unwrap();
    let mut independent = BattleSession::new(RAW, options()).unwrap();
    let mut commands = Vec::new();
    let mut frames = pause(&mut session);
    let initial = session.initial_states().to_vec();
    let mut counts = HashMap::<PlrId, usize>::new();
    let mut overlap = false;
    for turn in 0..10 {
        let actor = session.paused_actor().unwrap();
        let before = session.model_state().unwrap();
        assert!(session.next_frame().unwrap().is_none());
        assert_eq!(before, session.model_state().unwrap());
        // 交错推进另一会话，不能影响当前会话的待发动作或药水。
        if turn == 0 {
            pause(&mut independent);
        }
        let count = counts.entry(actor).or_default();
        let packed = if *count == 0 { 1 << 8 } else { 0 };
        *count += 1;
        session.submit_action(actor, 255, packed).unwrap();
        commands.extend([actor as u32, 255, packed]);
        let added = pause(&mut session);
        let mut boosted = 0;
        for state in session.current_states().iter().filter(|s| s.team_index == 0) {
            let base = initial.iter().find(|s| s.id == state.id).unwrap().attack;
            let n = counts.get(&state.id).copied().unwrap_or(0);
            let expected = if n == 1 { base + (base / 4).max(8) } else { base };
            assert_eq!(state.attack, expected, "turn {turn}, actor {}", state.id);
            boosted += usize::from(n == 1);
        }
        overlap |= boosted == 2;
        frames.extend(added);
    }
    assert!(overlap, "两个角色的药水必须能同时生效");
    assert!(counts.values().all(|n| *n >= 2));
    let replay =
        super::super::battle_replay_with_actions(RAW, options(), &crate::runtime::decode_actions(&commands).unwrap()).unwrap();
    assert_eq!(replay.frames, frames);
    assert_eq!(replay.final_states, session.current_states());
    assert_eq!(replay.paused_actor, session.paused_actor());
    assert!(independent.paused_actor().is_some());
}

// 批量重放从开局重新创建实体，队列中的分身 id 必须等召唤后再消费。
#[test]
fn replay_accepts_commands_for_clones_created_after_initialization() {
    let raw = "caster@red+ol:{\"attrs\":[40,40,200,40,40,40,100,20000],\"skills\":{\"sklclone\":106},\"name_factor_enabled\":false}\n\nenemy@blue+ol:{\"attrs\":[10,40,80,40,40,40,40,20000],\"skills\":{},\"name_factor_enabled\":false}\nseed:3719276890@!";
    let baseline = BattleSession::new(raw, BattleOptions::default()).unwrap();
    let team = baseline
        .initial_states()
        .iter()
        .find(|s| s.input_team_index == Some(0))
        .unwrap()
        .team_index;
    let replay_options = BattleOptions {
        allow_future_actors: true,
        controlled_team: Some(team),
        ..options()
    };
    let mut session = BattleSession::new(raw, replay_options.clone()).unwrap();
    let mut frames = pause(&mut session);
    let initial_ids: Vec<_> = session.initial_states().iter().map(|s| s.id as u32).collect();
    let mut commands = Vec::new();
    let mut clone_turns = 0;
    for turn in 0..12 {
        let actor = session.paused_actor().unwrap();
        if !initial_ids.contains(&(actor as u32)) {
            clone_turns += 1;
        }
        let (skill, mp) = if turn == 0 { (23, 8) } else { (255, 0) };
        session.submit_action(actor, skill, mp).unwrap();
        commands.extend([actor as u32, skill, mp]);
        frames.extend(pause(&mut session));
        let replay = super::super::battle_replay_with_actions(
            raw,
            replay_options.clone(),
            &crate::runtime::decode_actions(&commands).unwrap(),
        )
        .unwrap();
        assert_eq!(replay.frames, frames);
        assert_eq!(replay.paused_actor, session.paused_actor());
    }
    assert!(clone_turns >= 2, "分身需要能重复出牌并继续行动");
}

#[test]
fn fractional_hp_is_team_scoped_and_invalid_options_are_rejected() {
    let base = BattleSession::new(RAW, options()).unwrap();
    let half = BattleSession::new(
        RAW,
        BattleOptions {
            start_hp_pct: Some(0.5),
            ..options()
        },
    )
    .unwrap();
    for (a, b) in base.initial_states().iter().zip(half.initial_states()) {
        assert_eq!(
            b.hp,
            if a.team_index == 0 {
                (a.max_hp as f64 * 0.5).round() as i32
            } else {
                a.hp
            }
        );
    }
    for pct in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            BattleSession::new(
                RAW,
                BattleOptions {
                    start_hp_pct: Some(pct),
                    ..options()
                }
            )
            .is_err()
        );
    }
    assert!(
        BattleSession::new(
            RAW,
            BattleOptions {
                start_hp_pct: Some(1.2),
                ..BattleOptions::default()
            }
        )
        .is_err()
    );
    assert!(
        BattleSession::new(
            RAW,
            BattleOptions {
                controlled_team: Some(50),
                ..options()
            }
        )
        .is_err()
    );
}

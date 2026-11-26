use super::*;
use crate::runtime::EntityIdx;

fn session(max_rounds: usize) -> BattleSession {
    BattleSession::new(
        "left@red\n\nright@blue\n",
        BattleOptions {
            max_rounds,
            ..BattleOptions::default()
        },
    )
    .unwrap()
}

// 实际召唤后的投影必须保留各自图标键，不能回退到 Boss 本体徽章。
#[test]
fn summoned_animals_keep_their_own_battle_icons() {
    use crate::runtime::RunUpdates;
    let mut s = BattleSession::new("quilt@!\n\nalpha\n\nseed:1", BattleOptions::default()).unwrap();
    let rt = s.driver.runner.runtime_mut();
    let boss = rt.entities.iter().find(|(_, e)| e.template.name == "quilt").unwrap().0;
    let mut updates = RunUpdates::new();
    for _ in 0..3 {
        rt.drain_quilt_action_into(boss, &mut updates);
    }
    rt.tiger_spawn_leopard(boss, &mut updates);
    let states = states_from_runner(&s.driver.runner, false, false, &mut HashMap::new());
    for key in ["quilt-monkey", "quilt-fish", "smdd-leopard"] {
        let state = states.iter().find(|state| state.id_key_name == key).unwrap();
        assert_eq!(state.icon_key, key);
    }
}

fn remove_round_actors(session: &mut BattleSession) {
    for id in session.driver.runner.all_player_ids() {
        session.driver.runner.runtime_mut().world.remove_round_actor(EntityIdx(id as u32));
    }
}

// 暂停面板必须包含本次行动前已触发的护盾，不能显示回滚检查点的旧状态。
#[test]
fn controlled_pause_exposes_pre_action_shield() {
    let raw = "left@red+ol:{\"attrs\":[60,60,90,60,90,60,90,20000],\"skills\":{\"shield\":128,\"fire\":100},\"name_factor_enabled\":false}\n\nright@blue+ol:{\"attrs\":[60,60,90,60,90,60,90,20000],\"skills\":{},\"name_factor_enabled\":false}\nseed:pause-shield@!";
    let mut session = BattleSession::new(
        raw,
        BattleOptions {
            controlled_teams: vec![0, 1],
            preview_controlled_pause: true,
            team_upgrades: false,
            ..BattleOptions::default()
        },
    )
    .unwrap();
    // 让带护盾的本体先行动；验证的是公开面板，不依赖具体随机护盾数值。
    session
        .driver
        .runner
        .runtime_mut()
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .runtime
        .move_state
        .speed_points = crate::runtime::MOVE_POINT_THRESHOLD + 1;
    while session.next_frame().unwrap().is_some() {}
    assert_eq!(session.paused_actor(), Some(0));
    assert!(session.current_states()[0].shield > 0, "暂停面板必须显示本次行动前生成的护盾");
}

// 预览消耗必须等于实际扣除，覆盖行动前 smart 掷骰导致的随机流偏移。
#[test]
fn controlled_pause_cost_matches_action_execution() {
    for seed in 0..4 {
        let raw = format!(
            "left@red+ol:{{\"attrs\":[60,60,90,60,90,60,90,20000],\"skills\":{{\"fire\":100}},\"name_factor_enabled\":false}}\n\nright@blue+ol:{{\"attrs\":[60,60,90,60,90,60,90,20000],\"skills\":{{\"fire\":100}},\"name_factor_enabled\":false}}\nseed:pause-cost-{seed}@!"
        );
        let options = BattleOptions {
            controlled_teams: vec![0, 1],
            preview_controlled_pause: true,
            team_upgrades: false,
            ..BattleOptions::default()
        };
        let mut paused = BattleSession::new(&raw, options.clone()).unwrap();
        while paused.next_frame().unwrap().is_some() {}
        let actor = paused.paused_actor().unwrap();
        let cost = paused.peek_magic_cost().unwrap();
        assert_eq!(paused.peek_magic_cost(), Some(cost));
        let action = PlannedAction {
            target_team: None,
            actor,
            skill: PlannedSkill::Builtin(crate::runtime::BuiltinActiveSkill::Fire),
            magic_cost: None,
            potion: 0,
        };
        let mut automatic = BattleSession::new_with_actions(&raw, options.clone(), &[action]).unwrap();
        let mut explicit = BattleSession::new_with_actions(
            &raw,
            options,
            &[PlannedAction {
                magic_cost: Some(cost),
                ..action
            }],
        )
        .unwrap();
        let frame = automatic.next_frame().unwrap().unwrap();
        let after = frame.states.iter().find(|state| state.id == actor).unwrap().magic_point;
        let explicit_frame = explicit.next_frame().unwrap().unwrap();
        assert_eq!(
            explicit_frame.states.iter().find(|state| state.id == actor).unwrap().magic_point,
            after,
            "预排消耗与现场消耗必须一致，seed={seed}"
        );
    }
}

// 每次等待期间反复查询预览，再原地续行，帧和下一次暂停状态必须与完整脚本重演一致。
#[test]
fn controlled_resume_matches_replay_after_preview_queries() {
    let raw = "left@red+ol:{\"attrs\":[60,60,90,60,90,60,90,20000],\"skills\":{\"shield\":128,\"fire\":100},\"name_factor_enabled\":false}\n\nright@blue+ol:{\"attrs\":[60,60,90,60,90,60,90,20000],\"skills\":{\"shield\":128,\"fire\":100},\"name_factor_enabled\":false}\nseed:pause-resume@!";
    let options = BattleOptions {
        controlled_teams: vec![0, 1],
        preview_controlled_pause: true,
        team_upgrades: false,
        ..BattleOptions::default()
    };
    let collect = |session: &mut BattleSession| {
        let mut frames = Vec::new();
        while let Some(frame) = session.next_frame().unwrap() {
            frames.push(serde_json::to_value(frame).unwrap());
        }
        frames
    };
    let mut live = BattleSession::new(raw, options.clone()).unwrap();
    let mut frames = collect(&mut live);
    let mut actions = Vec::new();
    for _ in 0..8 {
        let actor = live.paused_actor().unwrap();
        let cost = live.peek_magic_cost().unwrap();
        let enough_mp = live.current_states().iter().find(|state| state.id == actor).unwrap().magic_point >= i32::from(cost);
        let (skill, public_id, magic_cost) = if enough_mp {
            (PlannedSkill::Builtin(crate::runtime::BuiltinActiveSkill::Fire), 0, Some(cost))
        } else {
            (PlannedSkill::BasicAttack, 255, None)
        };
        let state = serde_json::to_value(live.current_states()).unwrap();
        let targets = live.preview_targets(actor, skill).unwrap();
        assert_eq!(live.preview_targets(actor, skill).unwrap(), targets);
        assert_eq!(live.peek_magic_cost(), Some(cost));
        assert_eq!(serde_json::to_value(live.current_states()).unwrap(), state);
        actions.push(PlannedAction {
            target_team: None,
            actor,
            skill,
            magic_cost,
            potion: 0,
        });
        live.submit_action(actor, public_id, u32::from(magic_cost.unwrap_or(0))).unwrap();
        frames.extend(collect(&mut live));
        let mut replay = BattleSession::new_with_actions(raw, options.clone(), &actions).unwrap();
        assert_eq!(collect(&mut replay), frames);
        assert_eq!(
            serde_json::to_value(replay.current_states()).unwrap(),
            serde_json::to_value(live.current_states()).unwrap()
        );
        assert_eq!(replay.paused_actor(), live.paused_actor());
    }
}

// 双方场景下席位预览应与普通预览一致，均读取实际等待位置而非回滚检查点。
#[test]
fn targeted_preview_matches_current_pause_without_consuming_state() {
    for seed in 0..16 {
        let raw = format!(
            "left@red+ol:{{\"attrs\":[60,60,90,60,90,60,90,20000],\"skills\":{{}},\"name_factor_enabled\":false}}\n\nright-a@blue+ol:{{\"attrs\":[60,60,90,60,90,60,90,20000],\"skills\":{{}},\"name_factor_enabled\":false}}\nright-b@blue+ol:{{\"attrs\":[60,60,90,60,90,60,90,20000],\"skills\":{{}},\"name_factor_enabled\":false}}\nseed:target-pause-{seed}@!"
        );
        let mut session = BattleSession::new(
            &raw,
            BattleOptions {
                controlled_teams: vec![0, 1],
                preview_controlled_pause: true,
                team_upgrades: false,
                ..BattleOptions::default()
            },
        )
        .unwrap();
        while session.next_frame().unwrap().is_some() {}
        let actor = session.paused_actor().unwrap();
        let own_team = session
            .current_states()
            .iter()
            .find(|state| state.id == actor)
            .unwrap()
            .input_team_index
            .unwrap();
        let state = serde_json::to_value(session.current_states()).unwrap();
        let cost = session.peek_magic_cost();
        let expected = session.preview_targets(actor, PlannedSkill::BasicAttack).unwrap();
        for _ in 0..2 {
            assert_eq!(
                session.preview_targets_for_team(actor, PlannedSkill::BasicAttack, 1 - own_team).unwrap(),
                expected,
                "席位预览应使用当前暂停的随机流，seed={seed}"
            );
            assert_eq!(session.peek_magic_cost(), cost);
            assert_eq!(serde_json::to_value(session.current_states()).unwrap(), state);
        }
    }
}

#[test]
fn kill_pause_refreshes_states_without_pending_updates() {
    let raw = r#"killer@red+ol:{"attrs":[127,127,127,127,127,127,127,10000],"skills":{"quake":128,"merge":64}}

victim-a@blue+ol:{"attrs":[1,1,1,1,1,1,1,1],"skills":{"zombie":32}}
victim-b@blue+ol:{"attrs":[1,1,1,1,1,1,1,1],"skills":{"zombie":32}}
victim-c@blue+ol:{"attrs":[1,1,1,1,1,1,1,1],"skills":{"zombie":32}}
seed:kill-pause-snapshot@!"#;
    let mut decisions = Vec::new();
    let mut kills = 0;
    for _ in 0..40 {
        let mut session = BattleSession::new(
            raw,
            BattleOptions {
                team_upgrades: false,
                include_skills: true,
                passive_decision_mode: true,
                passive_decisions: decisions.clone(),
                ..BattleOptions::default()
            },
        )
        .unwrap();
        while session.next_frame().unwrap().is_some() {}
        let request = session.pending_passive().expect("应在击倒被动边界暂停").clone();
        let merge = request.skill == "core.skill.merge";
        if merge {
            kills += 1;
            let target = request.target.unwrap();
            assert!(!session.current_states().iter().find(|state| state.id == target).unwrap().alive);
            let expected = states_from_runner(&session.driver.runner, false, true, &mut session.icon_cache);
            assert_eq!(session.current_states(), expected);
            if kills == 2 {
                assert!(
                    session.current_states()[0]
                        .skills
                        .iter()
                        .any(|skill| skill.export_name == "core.skill.zombie")
                );
                return;
            }
        }
        decisions.push(crate::runtime::PassiveDecision {
            actor: request.actor,
            skill: request.skill,
            activate: merge,
            success: merge,
            target: None,
        });
    }
    panic!("未到达第二次击倒");
}

#[test]
fn paused_forced_action_captures_exact_boundary_and_is_read_only() {
    for hide in [false, true] {
        let skills = if hide { r#"{"hide":128}"# } else { "{}" };
        let raw = format!("left@red+ol:{{\"skills\":{skills}}}\n\nright@blue+ol:{{\"skills\":{{}}}}\nseed:7@!");
        let mut session = BattleSession::new(
            &raw,
            BattleOptions {
                controlled_teams: vec![0, 1],
                ..BattleOptions::default()
            },
        )
        .unwrap();
        assert_eq!(session.paused_forced_action(), None);
        let runtime = session.driver.runner.runtime_mut();
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.move_state.speed_points =
            crate::runtime::MOVE_POINT_THRESHOLD + 1;
        runtime
            .entities
            .get_mut(EntityIdx(0))
            .unwrap()
            .states
            .add_entry(crate::runtime::StateEntry::berserk(10, 2));
        session.next_frame().unwrap();
        assert_eq!(session.paused_actor(), Some(0));
        assert!(session.current_states()[0].status_labels.iter().any(|label| label.starts_with("狂暴")));
        // 默认上游语义下隐匿会取消强制行动；`hide_berserk_compat` 打开后带隐匿也捕获 Berserk。
        let expected = (cfg!(feature = "hide_berserk_compat") || !hide).then_some(ForcedAction::Berserk);
        let state = session.model_state().unwrap();
        let rng = session.driver.runner.runtime().rng.clone();
        let rounds = session.rounds_advanced();
        for _ in 0..3 {
            assert_eq!(session.paused_forced_action(), expected);
            assert_eq!(session.next_frame().unwrap(), None);
            assert_eq!(session.model_state().unwrap(), state);
            assert_eq!(session.rounds_advanced(), rounds);
            let current_rng = &session.driver.runner.runtime().rng;
            assert_eq!(
                (current_rng.i, current_rng.j, current_rng.main_val),
                (rng.i, rng.j, rng.main_val)
            );
        }
        assert_eq!(session.driver.runner.runtime().control.awaiting_forced_action(), None);
    }
}

#[test]
fn shield_serializes_through_initial_frame_clip_and_final_states() {
    let mut session = session(1);
    let initial_json = serde_json::to_value(session.initial_states()).unwrap();
    assert!(initial_json.as_array().unwrap().iter().all(|state| state["shield"] == 0));
    let max_shield = session.initial_states()[1].max_shield;
    assert_eq!(initial_json[1]["max_shield"], max_shield);
    session.driver.runner.runtime_mut().entities.get_mut(EntityIdx(1)).unwrap().runtime.shield = 40;
    let previous = states_from_runner(&session.driver.runner, false, false, &mut session.icon_cache);
    session.driver.runner.runtime_mut().entities.get_mut(EntityIdx(1)).unwrap().runtime.shield = 25;
    let states = states_from_runner(&session.driver.runner, false, false, &mut session.icon_cache);
    let mut updates = RunUpdates::new();
    updates.add(crate::RunUpdate::new("[0]发起攻击", 0, 1, 0));
    updates.add(crate::RunUpdate::new("[1]受到[2]点伤害", 0, 1, 0));
    updates.add(crate::RunUpdate::new("[1]挡住攻击", 0, 1, 0));
    let frame = build_frame(&updates, &previous, &states, &session.driver.runner, session.has_initial_boss);
    let frame_json = serde_json::to_value(&frame).unwrap();
    assert_eq!(frame_json["states"][1]["shield"], 25);
    assert_eq!(frame_json["states"][1]["max_shield"], max_shield);
    let clips = &frame_json["rows"][0]["clips"];
    assert_eq!(clips[0]["sidebar_previous_states"][1]["shield"], 40);
    assert_eq!(clips[0]["sidebar_states"][1]["shield"], 40);
    assert_eq!(clips[1]["sidebar_previous_states"][1]["shield"], 40);
    assert_eq!(clips[1]["sidebar_states"][1]["shield"], 25);
    assert_eq!(clips[2]["sidebar_previous_states"][1]["shield"], 25);
    assert_eq!(clips[2]["sidebar_states"][1]["shield"], 25);
    for clip in clips.as_array().unwrap() {
        assert_eq!(clip["sidebar_previous_states"][1]["max_shield"], max_shield);
        assert_eq!(clip["sidebar_states"][1]["max_shield"], max_shield);
    }
    assert_eq!(
        session.driver.runner.runtime().entities.get(EntityIdx(1)).unwrap().runtime.shield,
        25
    );

    remove_round_actors(&mut session);
    assert_eq!(session.next_frame().unwrap(), None);
    let result_json = serde_json::to_value(session.result().unwrap()).unwrap();
    assert_eq!(result_json["final_states"][1]["shield"], 25);
    assert_eq!(result_json["final_states"][1]["max_shield"], max_shield);
}

#[test]
fn half_skill_percent_message_classifies_as_damage_and_preserves_actual_hp_pair() {
    let mut session = session(1);
    let runtime = session.driver.runner.runtime_mut();
    let caster = runtime.entities.get_mut(EntityIdx(0)).unwrap();
    caster.runtime.wisdom = 512;
    caster.runtime.magic = 0;
    let target = runtime.entities.get_mut(EntityIdx(1)).unwrap();
    target.runtime.hp = 20;
    target.runtime.resistance = 0;
    target.runtime.agility = 0;
    target.template.skills = Default::default();
    let previous = states_from_runner(&session.driver.runner, false, false, &mut session.icon_cache);
    let mut updates = RunUpdates::new();
    session
        .driver
        .runner
        .runtime_mut()
        .drain_plain_half_skill_into(EntityIdx(0), EntityIdx(1), &mut updates);
    let states = states_from_runner(&session.driver.runner, false, false, &mut session.icon_cache);
    let frame = build_frame(&updates, &previous, &states, &session.driver.runner, session.has_initial_boss);
    assert_eq!(frame.updates[0].message_template, "[0]使用[瘟疫]");
    assert_eq!(frame.updates[1].message_template, "[1]体力减少[2]%");
    assert_eq!(frame.updates[1].tone, "damage");
    assert_eq!(frame.updates[1].param, Some(47));
    assert_eq!(frame.updates[1].score, 9);
    assert_eq!(frame.updates[1].hp_delta, Some(-9));
    let clips = &frame.rows[0].clips;
    assert!(clips[0].parts.iter().all(|part| !part.show_hp));
    let part = clips[1].parts.iter().find(|part| part.player_id == Some(1)).unwrap();
    assert!(part.show_hp);
    assert_eq!((part.hp_before, part.hp_after), (20, 11));
    let json = serde_json::to_value(&frame).unwrap();
    assert_eq!(json["updates"][1]["hp_delta"], -9);
    assert_eq!(json["rows"][0]["clips"][1]["parts"][0]["show_hp"], true);
}

#[test]
fn stress_half_fixture_exposes_percent_hp_parts() {
    let raw = "My_Generation lrSP5W8@芒萁\nUnbound v7mEDujiDSLN@芒萁\n\n潮汐-糖藷愠颏@涵虚\n风驰-xtxxgwcb@涵虚\nseed:33554538@!";
    let mut session = BattleSession::new(raw, BattleOptions::default()).unwrap();
    let mut saw_half = false;
    while let Some(frame) = session.next_frame().unwrap() {
        for update in &frame.updates {
            if update.message_template == "[1]体力减少[2]%" && update.score > 0 {
                if !saw_half {
                    println!(
                        "瘟疫百分比句首次出现：frame={} round={} {}",
                        frame.frame_index, frame.round_index, update.message_rendered
                    );
                }
                saw_half = true;
                assert_eq!(update.tone, "damage");
                assert_eq!(update.hp_delta, Some(-(update.score as i32)));
                assert!(frame.rows.iter().flat_map(|row| &row.clips).any(|clip| {
                    clip.tone == "damage"
                        && clip
                            .parts
                            .iter()
                            .any(|part| part.player_id == update.target_id && part.show_hp && part.hp_before > part.hp_after)
                }));
            }
        }
    }
    assert!(saw_half);
}

#[test]
fn shield_overlay_fixture_exposes_positive_snapshot_and_clip_delta() {
    let raw = r#"left@red+ol:{"skills":{"shield":80}}

right@blue+ol:{"skills":{}}
seed:7@!"#;
    let mut session = BattleSession::new(raw, BattleOptions::default()).unwrap();
    let mut saw_shield = false;
    let mut saw_consumption = false;
    while let Some(frame) = session.next_frame().unwrap() {
        if !saw_shield && frame.states.iter().any(|state| state.shield > 0) {
            saw_shield = true;
            println!("护盾首次出现：frame={} round={}", frame.frame_index, frame.round_index);
        }
        for clip in frame.rows.iter().flat_map(|row| &row.clips) {
            for (before, after) in clip.sidebar_previous_states.iter().zip(&clip.sidebar_states) {
                if before.shield > after.shield {
                    saw_consumption = true;
                    assert!(clip.parts.iter().any(|part| part.player_id == Some(after.id)));
                }
            }
        }
    }
    assert!(saw_shield);
    assert!(saw_consumption);
}

#[test]
fn skills_are_only_exported_when_requested() {
    let raw = r#"left@red+ol:{"skills":{"shield":80}}

right@blue+ol:{"skills":{}}
seed:7@!"#;

    // 默认关闭：技能表为空，序列化时连键都不出现，既有载荷不变。
    let plain = BattleSession::new(raw, BattleOptions::default()).unwrap();
    assert!(plain.initial_states().iter().all(|state| state.skills.is_empty()));
    let json = serde_json::to_value(plain.initial_states()).unwrap();
    assert!(json[0].get("skills").is_none(), "默认不应序列化 skills: {json}");

    let options = BattleOptions {
        include_skills: true,
        ..BattleOptions::default()
    };
    let mut session = BattleSession::new(raw, options).unwrap();
    let left = &session.initial_states()[0];
    assert_eq!(left.display_name, "left");
    assert_eq!(left.skills.len(), 1, "overlay 只声明 shield 一项熟练度：{:?}", left.skills);
    assert_eq!(left.skills[0].level, 80);
    assert!(!left.skills[0].export_name.is_empty());

    // 帧内快照同样带熟练度，且不影响战斗结果。
    let mut saw_skills = false;
    while let Some(frame) = session.next_frame().unwrap() {
        saw_skills |= frame.states.iter().any(|state| !state.skills.is_empty());
    }
    assert!(saw_skills);
    assert!(session.result().unwrap().finished);
}

#[test]
fn initial_and_terminal_queries_are_consistent_and_idempotent() {
    let mut session = session(20_000);
    assert_eq!(session.initial_states(), session.current_states());
    assert_eq!(session.status(), BattleStatus::Running);
    assert_eq!(session.stop_reason(), None);
    assert_eq!(session.result(), None);
    let initial = session.initial_states().to_vec();
    let mut count = 0;
    let mut previous_round = None;
    while let Some(frame) = session.next_frame().unwrap() {
        assert_eq!(frame.frame_index, count);
        assert!(previous_round.is_none_or(|previous| frame.round_index > previous));
        previous_round = Some(frame.round_index);
        assert_eq!(frame.states, session.current_states());
        count += 1;
    }
    assert_eq!(initial, session.initial_states());
    let result = session.result().unwrap();
    assert_eq!(result.status, BattleStatus::Finished);
    assert_eq!(session.paused_forced_action(), None);
    assert_eq!(result.stop_reason, BattleStopReason::Winner);
    assert!(result.finished && !result.truncated);
    assert_eq!(result.frames_emitted, count);
    assert!(result.rounds_advanced >= count);
    assert!(!result.winner_ids.is_empty());
    for _ in 0..10 {
        assert_eq!(session.next_frame().unwrap(), None);
        assert_eq!(session.result().unwrap(), result);
    }
}

#[test]
fn empty_rounds_count_towards_max_rounds() {
    let mut session = session(3);
    remove_round_actors(&mut session);
    assert_eq!(session.next_frame().unwrap(), None);
    let result = session.result().unwrap();
    assert_eq!(result.rounds_advanced, 3);
    assert_eq!(result.frames_emitted, 0);
    assert_eq!(result.stop_reason, BattleStopReason::MaxRounds);
    assert!(result.truncated && !result.finished);
    assert!(result.winner_ids.is_empty() && result.winner_team_indices.is_empty());
}

#[test]
fn no_progress_stops_at_entity_scaled_limit_and_max_rounds_has_priority() {
    for (max_rounds, expected) in [(100, BattleStopReason::NoProgress), (32, BattleStopReason::MaxRounds)] {
        let mut session = session(max_rounds);
        remove_round_actors(&mut session);
        assert_eq!(session.next_frame().unwrap(), None);
        assert_eq!(session.rounds_advanced(), 32);
        assert_eq!(session.stop_reason(), Some(expected));
        assert_eq!(session.status(), BattleStatus::Truncated);
    }
}

#[test]
fn winner_on_empty_update_emits_frame_and_beats_round_limit() {
    let mut session = session(1);
    let runtime = session.driver.runner.runtime_mut();
    let loser = runtime.entities.get_mut(EntityIdx(1)).unwrap();
    loser.runtime.hp = 0;
    loser.runtime.alive = false;
    let frame = session.next_frame().unwrap().unwrap();
    assert!(frame.updates.is_empty());
    assert!(frame.finished);
    assert_eq!(frame.frame_index, 0);
    assert_eq!(frame.round_index, 0);
    assert_eq!(frame.winner_ids, vec![0]);
    assert!(frame.rows.iter().flat_map(|row| &row.clips).any(|clip| clip.winner));
    assert_eq!(session.stop_reason(), Some(BattleStopReason::Winner));
    assert_eq!(frame.states, session.result().unwrap().final_states);
}

#[test]
fn visible_winner_on_last_round_beats_round_limit() {
    let mut full = session(20_000);
    while full.next_frame().unwrap().is_some() {}
    let mut limited = session(full.rounds_advanced());
    while limited.next_frame().unwrap().is_some() {}
    assert_eq!(limited.result(), full.result());
    assert!(limited.is_finished());
}

#[cfg(feature = "png_render")]
#[test]
fn icons_are_cached_per_key_and_disabled_by_default() {
    let mut plain = session(3);
    assert!(plain.current_states().iter().all(|state| state.icon_png_base64.is_none()));
    plain.next_frame().unwrap();
    assert!(plain.icon_cache.is_empty());
    let mut icons = BattleSession::new(
        "left@red\n\nright@blue\n",
        BattleOptions {
            include_icons: true,
            max_rounds: 3,
            ..BattleOptions::default()
        },
    )
    .unwrap();
    let initial_cache = icons.icon_cache.clone();
    while icons.next_frame().unwrap().is_some() {}
    for state in icons.current_states() {
        assert_eq!(state.icon_png_base64.as_ref(), icons.icon_cache.get(&state.icon_key));
    }
    for (key, value) in initial_cache {
        assert_eq!(icons.icon_cache.get(&key), Some(&value));
    }
}

#[cfg(feature = "png_render")]
#[test]
fn special_battle_icon_keys_match_the_original_display_groups() {
    let options = BattleOptions {
        include_icons: true,
        ..Default::default()
    };
    let session = BattleSession::new("mario@!\nAlice@plain\n\na@!\nb@!\nc@green\n\nmario", options.clone()).unwrap();
    let by_name: HashMap<_, _> = session.initial_states().iter().map(|state| (state.id_key_name.as_str(), state)).collect();
    for (name, icon) in [
        ("mario@!", "mario@!"),
        ("Alice@plain", "mario@!"),
        ("a@!", "a"),
        ("b@!", "b"),
        ("c@green", "a"),
        ("mario", "mario"),
    ] {
        let state = by_name[name];
        assert_eq!(state.icon_key, icon, "{name}");
        assert_eq!(state.icon_png_base64.as_ref(), Some(&render_icon_b64_from_name(icon)));
    }
    let mut slime = BattleSession::new("slime@!\n\nlazy@!\nparity3a\nparity3b", options).unwrap();
    let child_has_boss_icon = std::iter::from_fn(|| slime.next_frame().unwrap()).any(|frame| {
        frame
            .states
            .iter()
            .any(|state| state.id_key_name.starts_with("slime?") && state.icon_key == "slime@!")
    });
    assert!(child_has_boss_icon);
}

#[cfg(feature = "png_render")]
#[test]
fn icon_projection_preserves_full_keys_cache_values_and_rng() {
    let runner = RuntimeRunner::new_from_namerena_raw("同名@红队\n\n同名@蓝队\nseed:cache@!".to_owned()).unwrap();
    let snapshots = runner.player_snapshots();
    assert_eq!(snapshots[0].id_name, snapshots[1].id_name);
    assert_ne!(snapshots[0].id_key_name, snapshots[1].id_key_name);
    let before = runner.model_state().unwrap();
    let rng_before = runner.runtime().rng.clone();
    let cached_key = snapshots[0].id_key_name.clone();
    let missing_key = snapshots[1].id_key_name.clone();
    let mut cache = HashMap::from([(cached_key.clone(), "预填图标".to_owned())]);
    let original_cache = cache.clone();

    let plain = states_from_runner(&runner, false, false, &mut cache);
    assert!(plain.iter().all(|state| state.icon_png_base64.is_none()));
    assert_eq!(cache, original_cache);

    let with_icons = states_from_runner(&runner, true, false, &mut cache);
    assert_eq!(cache.len(), 2);
    assert_eq!(with_icons[0].icon_key, cached_key);
    assert_eq!(with_icons[0].icon_png_base64.as_deref(), Some("预填图标"));
    assert_eq!(with_icons[1].icon_key, missing_key);
    assert_eq!(cache[&missing_key], render_icon_b64_from_name(&missing_key));
    for ((state, plain), snapshot) in with_icons.iter().zip(&plain).zip(&snapshots) {
        assert_eq!(state.id, snapshot.id);
        assert_eq!(state.id_key_name, snapshot.id_key_name);
        assert_eq!(state.icon_png_base64.as_ref(), cache.get(&state.icon_key));
        let mut without_icon = state.clone();
        without_icon.icon_png_base64 = None;
        assert_eq!(&without_icon, plain);
    }
    // 纯快照没有技能槽位；图标投影比较只覆盖快照本身负责的字段。
    assert!(with_icons[1].skills.is_empty(), "include_skills 关闭时不填技能表");
    let mut projected_snapshot = with_icons[1].clone();
    projected_snapshot.skills.clear();
    assert_eq!(state_from_snapshot(snapshots[1].clone(), true), projected_snapshot);
    let filled_cache = cache.clone();
    assert_eq!(states_from_runner(&runner, true, false, &mut cache), with_icons);
    assert_eq!(cache, filled_cache);
    assert_eq!(runner.model_state().unwrap(), before);
    let rng_after = &runner.runtime().rng;
    assert_eq!((rng_after.i, rng_after.j), (rng_before.i, rng_before.j));
    assert_eq!(rng_after.main_val, rng_before.main_val);
    #[cfg(not(feature = "no_debug"))]
    assert_eq!(rng_after.byte_count, rng_before.byte_count);
}

#[cfg(not(feature = "png_render"))]
#[test]
fn missing_png_feature_rejects_icon_requests() {
    let options = BattleOptions {
        include_icons: true,
        ..BattleOptions::default()
    };
    let raw = "left@red\n\nright@blue";
    for error in [
        BattleSession::new(raw, options.clone()).unwrap_err(),
        super::super::battle_replay(raw, options).unwrap_err(),
    ] {
        assert_eq!(error.code().as_str(), "UNSUPPORTED_OPTION");
        assert!(error.to_string().contains("png_render"));
    }
}

#[cfg(not(feature = "png_render"))]
#[test]
fn missing_png_feature_keeps_plain_and_model_sessions_usable() {
    let raw = "left@red\n\nright@blue";
    let options = BattleOptions {
        max_rounds: 2,
        ..BattleOptions::default()
    };
    let mut plain = BattleSession::new(raw, options.clone()).unwrap();
    let mut model = super::super::BattleModelSession::new(
        raw,
        BattleOptions {
            include_icons: true,
            ..options
        },
    )
    .unwrap();
    assert!(!plain.initial_states().is_empty());
    assert!(plain.initial_states().iter().all(|state| state.icon_png_base64.is_none()));
    assert_eq!(plain.model_state().unwrap(), model.model_state().unwrap());
    loop {
        let display = plain.next_frame().unwrap();
        let boundary = model.next_frame().unwrap();
        assert_eq!(display.is_some(), boundary.is_some());
        if let (Some(display), Some(boundary)) = (&display, boundary) {
            assert_eq!(
                (display.frame_index, display.round_index),
                (boundary.frame_index, boundary.round_index),
            );
            assert!(display.states.iter().all(|state| state.icon_png_base64.is_none()));
        }
        assert_eq!(plain.model_state().unwrap(), model.model_state().unwrap());
        if display.is_none() {
            break;
        }
    }
    let result = plain.result().unwrap();
    let model_result = model.result().unwrap();
    assert_eq!(result.stop_reason, model_result.stop_reason);
    assert_eq!(result.rounds_advanced, model_result.rounds_advanced);
    assert!(result.final_states.iter().all(|state| state.icon_png_base64.is_none()));
}

#[test]
fn rejects_invalid_input_and_options_with_core_codes() {
    assert_eq!(
        BattleSession::new(" ", BattleOptions::default()).unwrap_err().code().as_str(),
        "INVALID_INPUT"
    );
    for options in [
        BattleOptions {
            max_rounds: 0,
            ..BattleOptions::default()
        },
        BattleOptions {
            eval_rq: f64::NAN,
            ..BattleOptions::default()
        },
    ] {
        assert_eq!(
            BattleSession::new("a\n\nb", options).unwrap_err().code().as_str(),
            "INVALID_ARGUMENT"
        );
    }
}

#[test]
fn runtime_error_is_sticky_and_does_not_become_a_truncated_result() {
    let mut session = BattleSession::new(
        "alpha@red+bed2[3000]\n\nbeta@blue",
        BattleOptions {
            max_rounds: 1,
            ..BattleOptions::default()
        },
    )
    .unwrap();
    assert!(!session.is_failed());
    session.invalidate_runtime_for_test();
    // 同时安排一个胜者：无效 Runtime 配置必须具有优先级。
    session.driver.runner.runtime_mut().entities.get_mut(EntityIdx(1)).unwrap().runtime.alive = false;
    let mut previous_error = None;
    for _ in 0..2 {
        let error = session.next_frame().unwrap_err();
        assert!(session.is_failed());
        assert_eq!(session.paused_forced_action(), None);
        assert!(!session.is_done());
        assert_eq!(session.status(), BattleStatus::Running);
        assert!(!session.is_truncated());
        let current_error = (error.code().as_str(), error.to_string());
        if let Some(previous) = &previous_error {
            assert_eq!(&current_error, previous);
        }
        previous_error = Some(current_error);
        assert_eq!(error.code().as_str(), "RUNTIME_FAILED");
        assert_eq!(session.result(), None);
        assert_eq!(session.stop_reason(), None);
        assert_eq!(session.rounds_advanced(), 0);
    }
}

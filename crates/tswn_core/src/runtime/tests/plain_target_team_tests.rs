use super::*;
use crate::runtime::control::TargetWisdom;

/// 按第三队人数构造多队目标选择 fixture：actor/ally 同队、两个 chosen 为敌，另加第三队。
fn fixture(third_team_size: usize) -> CombatRuntime {
    let config = default_custom_runtime_import_config().unwrap();
    let skills = [BuiltinActiveSkill::Fire, BuiltinActiveSkill::Quake, BuiltinActiveSkill::Rapid]
        .map(|skill| (config.registry.skill_id_by_export_name(skill.export_name()).unwrap(), 128));
    let mut players = vec![
        PlayerTemplate::new(1, "actor", 0, 1000, 3)
            .with_magic_point(1000)
            .with_skill_loadout(SkillLoadout::from_skill_levels(skills)),
        PlayerTemplate::new(2, "ally", 0, 1000, 3),
        PlayerTemplate::new(3, "chosen-a", 1, 50, 3).with_target_score_stats(100, 100, 1.0),
        PlayerTemplate::new(4, "chosen-b", 1, 200, 3).with_target_score_stats(100, 100, 1.0),
    ];
    for i in 0..third_team_size {
        players.push(PlayerTemplate::new(5 + i, format!("other-{i}"), 2, 1000, 3));
    }
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(players, config.registry));
    for binding in config.skill_handlers {
        runtime.set_skill_handler_with_capabilities(binding.skill_id, binding.handler, &binding.capabilities);
    }
    runtime
}

#[cfg(feature = "no_debug")]
#[test]
fn single_candidate_default_attack_preserves_target_and_rng() {
    for (teams, actor, dead, target) in [
        (&[0, 1][..], 0, None, 1),
        (&[0, 1][..], 1, None, 0),
        (&[0, 0, 1, 1][..], 0, Some(2), 3),
        (&[0, 1][..], 0, Some(0), 1),
    ] {
        let players = teams
            .iter()
            .enumerate()
            .map(|(id, &team)| PlayerTemplate::new(id, format!("player-{id}"), team, 1000, 3))
            .collect();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(players));
        if let Some(dead) = dead {
            let dead_idx = EntityIdx(dead as u32);
            let entity = runtime.entities.get_mut(dead_idx).unwrap();
            entity.runtime.alive = false;
            entity.runtime.hp = 0;
            assert!(runtime.world.mark_dead(dead_idx, teams[dead]));
        }
        runtime.rng = RC4::new(b"single-candidate-default-attack", 1);
        let alive = runtime.world.flat_alive().to_vec();
        let skips = alive
            .iter()
            .enumerate()
            .filter_map(|(index, entity)| (teams[entity.0 as usize] == teams[actor]).then_some(index))
            .collect::<Vec<_>>();
        let target = EntityIdx(target);
        let mut reference = runtime.rng.clone();
        for smart in [false, true].into_iter().cycle().take(512) {
            for _ in 0..if smart { 5 } else { 4 } {
                let picked = reference.pick_skip_range(&alive, &skips).unwrap();
                assert_eq!(alive[picked], target);
            }
            if !smart {
                reference.rFFFF();
            }
            assert_eq!(
                runtime.select_plain_default_attack_target(EntityIdx(actor as u32), smart),
                Some(target)
            );
            assert_rng_state_eq(&runtime.rng, &reference);
        }
    }
}

#[cfg(feature = "no_debug")]
#[test]
fn two_candidate_default_attack_preserves_sampling_and_first_tie() {
    for (actor, dead) in [(0, &[][..]), (3, &[][..]), (0, &[1][..]), (0, &[0, 1][..])] {
        let teams = [0, 0, 1, 1];
        let players = teams
            .iter()
            .enumerate()
            .map(|(id, &team)| PlayerTemplate::new(id, format!("player-{id}"), team, 100, 3))
            .collect();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(players));
        for &id in dead {
            let idx = EntityIdx(id as u32);
            let entity = runtime.entities.get_mut(idx).unwrap();
            entity.runtime.alive = false;
            entity.runtime.hp = 0;
            assert!(runtime.world.mark_dead(idx, teams[id]));
        }
        runtime.rng = RC4::new(b"two-candidate-default-attack", 1);
        let alive = runtime.world.flat_alive().to_vec();
        let skips = alive
            .iter()
            .enumerate()
            .filter_map(|(index, entity)| (teams[entity.0 as usize] == teams[actor]).then_some(index))
            .collect::<Vec<_>>();
        let mut reference = runtime.rng.clone();
        for smart in [false, true].into_iter().cycle().take(512) {
            let limit = if smart { 3 } else { 2 };
            let mut selected = Vec::new();
            let mut duplicates = 0;
            while duplicates <= limit && selected.len() < limit {
                let index = reference.pick_skip_range(&alive, &skips).unwrap();
                if selected.contains(&alive[index]) {
                    duplicates += 1;
                } else {
                    selected.push(alive[index]);
                }
            }
            // 两名候选初始属性相同，聪明评分相同；普通评分比较依次读取的随机数。
            let first = selected[0];
            let expected = if smart {
                first
            } else {
                let first_score = reference.rFFFF();
                match selected.get(1).copied() {
                    Some(second) if reference.rFFFF() > first_score => second,
                    _ => first,
                }
            };
            assert_eq!(
                runtime.select_plain_default_attack_target(EntityIdx(actor as u32), smart),
                Some(expected)
            );
            assert_rng_state_eq(&runtime.rng, &reference);
        }
    }
}

#[cfg(feature = "no_debug")]
#[test]
fn short_enemy_skill_selection_matches_generic_ranking_and_rng() {
    for teams in [&[0, 1][..], &[0, 1, 1][..], &[0, 0, 1, 1][..]] {
        let players = teams
            .iter()
            .enumerate()
            .map(|(id, &team)| PlayerTemplate::new(id, format!("player-{id}"), team, 100, 3))
            .collect();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(players));
        runtime.rng = RC4::new(b"short-enemy-skill-selection", 1);
        let alive = runtime.world.flat_alive().to_vec();
        let skips = alive
            .iter()
            .enumerate()
            .filter_map(|(index, entity)| (teams[entity.0 as usize] == teams[0]).then_some(index))
            .collect::<Vec<_>>();
        let mut reference = runtime.rng.clone();
        for count in 1..=6 {
            for smart in [false, true].into_iter().cycle().take(128) {
                let mut expected = Vec::new();
                let mut duplicates = 0;
                while duplicates <= count && expected.len() < count {
                    let picked = reference.pick_skip_range(&alive, &skips).unwrap();
                    if expected.contains(&alive[picked]) {
                        duplicates += 1;
                    } else {
                        expected.push(alive[picked]);
                    }
                }
                // 两个目标属性相同；聪明时保留首次命中顺序，普通时以随机评分稳定排序。
                if !smart {
                    let mut scored = expected.into_iter().map(|id| (id, reference.rFFFF())).collect::<Vec<_>>();
                    scored.sort_by_key(|&(_, score)| std::cmp::Reverse(score));
                    expected = scored.into_iter().map(|(id, _)| id).collect();
                }
                let actual = runtime.select_plain_default_enemy_targets_with_count(EntityIdx(0), smart, count);
                assert_eq!(actual.as_slice(), expected, "teams={teams:?}, smart={smart}, count={count}");
                assert_rng_state_eq(&runtime.rng, &reference);
            }
        }
    }
}

#[cfg(feature = "no_debug")]
#[test]
fn half_skill_short_selection_preserves_invalid_draw_budget() {
    for enemy_hp in [
        &[160][..],
        &[161][..],
        &[160, 399][..],
        &[161, 400][..],
        &[160, 400][..],
        &[161, 399][..],
    ] {
        let targets = enemy_hp
            .iter()
            .enumerate()
            .map(|(i, &hp)| PlayerTemplate::new(i + 2, format!("target-{i}"), 1, hp, 3));
        let players = std::iter::once(PlayerTemplate::new(1, "actor", 0, 100, 3)).chain(targets).collect();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(players));
        let mut reference = runtime.clone();
        let alive = reference.world.flat_alive().to_vec();
        let skips = alive.iter().position(|&id| id == EntityIdx(0)).unwrap();
        for smart in [false, true].into_iter().cycle().take(512) {
            let limit = if smart { 3 } else { 2 };
            let mut selected = Vec::new();
            let mut duplicate = 0;
            let mut invalid = 0;
            while duplicate <= limit && invalid <= 2 * limit && selected.len() < limit {
                let index = reference.rng.pick_skip_range(&alive, &[skips]).unwrap();
                let id = alive[index];
                let hp = reference.entities.get(id).unwrap().runtime.hp;
                if smart && (hp <= 160 || hp >= 400) {
                    invalid += 1;
                } else if selected.contains(&id) {
                    duplicate += 1;
                } else {
                    selected.push(id);
                }
            }
            let mut ranked = selected
                .into_iter()
                .map(|id| (id, reference.score_plain_half_target(id, smart)))
                .collect::<Vec<_>>();
            ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let expected = ranked.into_iter().map(|(id, _)| id).collect::<Vec<_>>();
            let actual = runtime.select_plain_half_targets(EntityIdx(0), smart);
            assert_eq!(actual.as_slice(), expected, "hp={enemy_hp:?}, smart={smart}");
            assert_rng_state_eq(&runtime.rng, &reference.rng);
        }
    }
}

#[test]
fn targeted_preview_restores_world_rng_and_matches_duel_selection() {
    let field = fixture(2);
    let pair = fixture(0);
    for skill in [
        PlannedSkill::BasicAttack,
        PlannedSkill::MagicAttack,
        PlannedSkill::Builtin(BuiltinActiveSkill::Fire),
    ] {
        for smart in [false, true] {
            for seed in 0..32 {
                let mut runtime = field.clone();
                let mut duel = pair.clone();
                let rng = RC4::new(&[seed], 1);
                runtime.rng = rng.clone();
                duel.rng = rng.clone();
                let actor = EntityIdx(0);
                runtime.control.set_awaiting_forced_action(0, None, smart);
                let actual = runtime
                    .preview_current_action_targets(actor, skill, Some(1), TargetWisdom::WholeAction)
                    .unwrap();
                // 独立建立实际 2 对 2 对局，用自动行动的选择器作对照。
                let expected: Vec<_> = match skill {
                    PlannedSkill::BasicAttack | PlannedSkill::MagicAttack => {
                        duel.select_plain_default_attack_target(actor, smart).into_iter().collect()
                    }
                    _ => duel.select_plain_default_enemy_targets(actor, smart).into_iter().collect(),
                };
                assert_eq!(actual, expected.iter().map(|id| id.0 as usize).collect::<Vec<_>>());
                assert!(matches!(actual.first(), Some(2 | 3)));
                assert_eq!(runtime.world, field.world);
                assert_rng_state_eq(&runtime.rng, &rng);
                runtime.queue_planned_action(PlannedAction {
                    actor: 0,
                    skill,
                    magic_cost: Some(0),
                    target_team: Some(1),
                    potion: 0,
                });
                let executed: Vec<_> = match runtime.prepare_plain_action(actor, smart, Default::default()).unwrap() {
                    PreparedPlainAction::BasicAttack { target, .. } => vec![target],
                    PreparedPlainAction::BuiltinSkill(action) => action.targets.into_iter().collect(),
                    _ => panic!("应执行指定攻击"),
                };
                assert_eq!(executed, expected);
                assert_rng_state_eq(&runtime.rng, &duel.rng);
            }
        }
    }
}

#[test]
fn targeted_execution_matches_preview_and_does_not_scope_real_world() {
    for skill in [
        PlannedSkill::BasicAttack,
        PlannedSkill::Builtin(BuiltinActiveSkill::Fire),
        PlannedSkill::Builtin(BuiltinActiveSkill::Quake),
        PlannedSkill::Builtin(BuiltinActiveSkill::Rapid),
    ] {
        for smart in [false, true] {
            let mut runtime = fixture(2);
            let world = runtime.world.clone();
            runtime.control.set_awaiting_forced_action(0, None, smart);
            let preview = runtime
                .preview_current_action_targets(EntityIdx(0), skill, Some(1), TargetWisdom::WholeAction)
                .unwrap();
            runtime.queue_planned_action(PlannedAction {
                potion: 0,
                actor: 0,
                skill,
                magic_cost: Some(0),
                target_team: Some(1),
            });
            let action = runtime.prepare_plain_action(EntityIdx(0), smart, Default::default()).unwrap();
            let targets: Vec<usize> = match action {
                PreparedPlainAction::BasicAttack { target, .. } => vec![target.0 as usize],
                PreparedPlainAction::BuiltinSkill(prepared) => prepared.targets.iter().map(|id| id.0 as usize).collect(),
                _ => panic!("应执行玩家选择的动作"),
            };
            assert_eq!(targets, preview);
            assert_eq!(runtime.world, world);
            assert!([2, 3].contains(&targets[0]));
            if matches!(
                skill,
                PlannedSkill::Builtin(BuiltinActiveSkill::Quake | BuiltinActiveSkill::Rapid)
            ) {
                assert!(targets.iter().any(|id| [4, 5].contains(id)), "后续候选应允许其他席位");
                let mut distinct = targets.clone();
                distinct.sort();
                distinct.dedup();
                assert_eq!(distinct.len(), targets.len());
            }
        }
    }
}

#[test]
fn invalid_target_team_cannot_fall_back_to_other_players() {
    let mut runtime = fixture(2);
    for team in [0, 99] {
        let result = runtime.preview_targeted_planned_targets(EntityIdx(0), PlannedSkill::BasicAttack, false, Some(team));
        assert!(result.is_err());
    }
    assert!(
        runtime
            .preview_targeted_planned_targets(EntityIdx(0), PlannedSkill::Builtin(BuiltinActiveSkill::Heal), false, Some(1))
            .is_err()
    );
}

/// 按技能类型返回自动多目标选择池（Quake/Rapid）。
fn automatic_multihit_pool(runtime: &mut CombatRuntime, skill: BuiltinActiveSkill, smart: bool) -> Vec<EntityIdx> {
    let actor = EntityIdx(0);
    match skill {
        BuiltinActiveSkill::Quake => {
            runtime.select_plain_default_enemy_targets_with_count(actor, smart, if smart { 6 } else { 5 })
        }
        BuiltinActiveSkill::Rapid => runtime.select_plain_rapid_targets(actor, smart),
        _ => unreachable!(),
    }
    .into_vec()
}

#[test]
fn multihit_followups_use_the_turn_wisdom_over_the_whole_field() {
    let field = fixture(9);
    let duel = fixture(0);
    for skill in [BuiltinActiveSkill::Quake, BuiltinActiveSkill::Rapid] {
        for smart in [false, true] {
            for seed in 0..32 {
                let actor = EntityIdx(0);
                let planned = PlannedSkill::Builtin(skill);
                let mut pair = duel.clone();
                let rng = RC4::new(&[seed], 1);
                pair.rng = rng.clone();
                let first = automatic_multihit_pool(&mut pair, skill, smart)[0];

                for include_followups in [false, true] {
                    let mut full = field.clone();
                    full.rng = pair.rng.clone();
                    let global = automatic_multihit_pool(&mut full, skill, smart && include_followups);
                    let expected: Vec<_> = std::iter::once(first)
                        .chain(global.iter().copied().filter(|id| *id != first))
                        .take(global.len())
                        .collect();

                    let mut actual = field.clone();
                    actual.rng = rng.clone();
                    actual.control.set_awaiting_forced_action(0, None, smart);
                    let wisdom = TargetWisdom::for_action(true, include_followups);
                    let preview = actual.preview_current_action_targets(actor, planned, Some(1), wisdom).unwrap();
                    assert_eq!(
                        preview,
                        expected.iter().map(|id| id.0 as usize).collect::<Vec<_>>(),
                        "{skill:?} smart={smart} include_followups={include_followups} seed={seed}"
                    );
                    assert_rng_state_eq(&actual.rng, &rng);
                    actual.queue_planned_action(PlannedAction {
                        actor: 0,
                        skill: planned,
                        target_team: Some(1),
                        magic_cost: Some(0),
                        potion: 0,
                    });
                    actual
                        .control
                        .set_action_target_teams(&[Some(1)], Some(&[true]), Some(&[include_followups]));
                    let Some(PreparedPlainAction::BuiltinSkill(executed)) =
                        actual.prepare_plain_action(actor, smart, Default::default())
                    else {
                        panic!("应执行选定的群攻")
                    };
                    assert_eq!(executed.targets.as_slice(), expected.as_slice());
                    assert_rng_state_eq(&actual.rng, &full.rng);
                }
            }
        }
    }
}

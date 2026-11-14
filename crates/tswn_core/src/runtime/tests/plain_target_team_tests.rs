use super::*;

fn fixture() -> CombatRuntime {
    let config = default_custom_runtime_import_config().unwrap();
    let skills = [BuiltinActiveSkill::Fire, BuiltinActiveSkill::Quake, BuiltinActiveSkill::Rapid]
        .map(|skill| (config.registry.skill_id_by_export_name(skill.export_name()).unwrap(), 128));
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "actor", 0, 1000, 3)
                .with_magic_point(1000)
                .with_skill_loadout(SkillLoadout::from_skill_levels(skills)),
            PlayerTemplate::new(2, "chosen-a", 1, 1000, 3),
            PlayerTemplate::new(3, "chosen-b", 1, 1000, 3),
            PlayerTemplate::new(4, "other-a", 2, 1000, 3),
            PlayerTemplate::new(5, "other-b", 2, 1000, 3),
        ],
        config.registry,
    ));
    for binding in config.skill_handlers {
        runtime.set_skill_handler_with_capabilities(binding.skill_id, binding.handler, &binding.capabilities);
    }
    runtime
}

#[test]
fn targeted_preview_restores_world_rng_and_matches_duel_selection() {
    for skill in [
        PlannedSkill::BasicAttack,
        PlannedSkill::MagicAttack,
        PlannedSkill::Builtin(BuiltinActiveSkill::Fire),
    ] {
        for smart in [false, true] {
            let mut runtime = fixture();
            let before = runtime.clone();
            let actual = runtime.preview_targeted_planned_targets(EntityIdx(0), skill, smart, Some(1)).unwrap();
            let mut duel = before.clone();
            duel.world = duel.world.targeting_duel(0, 1);
            let expected = duel.preview_planned_targets(EntityIdx(0), skill, smart).unwrap();
            assert_eq!(actual, expected);
            assert!(!actual.is_empty());
            assert!(actual.iter().all(|id| [1, 2].contains(id)));
            assert_eq!(runtime.world, before.world);
            assert_rng_state_eq(&runtime.rng, &before.rng);
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
        let mut runtime = fixture();
        let world = runtime.world.clone();
        let preview = runtime.preview_targeted_planned_targets(EntityIdx(0), skill, false, Some(1)).unwrap();
        runtime.queue_planned_action(PlannedAction {
            potion: 0,
            actor: 0,
            skill,
            magic_cost: Some(0),
            target_team: Some(1),
        });
        let action = runtime.prepare_plain_action(EntityIdx(0), false, Default::default()).unwrap();
        let targets: Vec<usize> = match action {
            PreparedPlainAction::BasicAttack { target, .. } => vec![target.0 as usize],
            PreparedPlainAction::BuiltinSkill(prepared) => prepared.targets.iter().map(|id| id.0 as usize).collect(),
            _ => panic!("应执行玩家选择的动作"),
        };
        assert_eq!(targets, preview);
        assert_eq!(runtime.world, world);
        assert!([1, 2].contains(&targets[0]));
        if matches!(
            skill,
            PlannedSkill::Builtin(BuiltinActiveSkill::Quake | BuiltinActiveSkill::Rapid)
        ) {
            assert!(targets.iter().any(|id| [3, 4].contains(id)), "后续候选应允许其他席位");
            let mut distinct = targets.clone();
            distinct.sort();
            distinct.dedup();
            assert_eq!(distinct.len(), targets.len());
        }
    }
}

#[test]
fn invalid_target_team_cannot_fall_back_to_other_players() {
    let mut runtime = fixture();
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

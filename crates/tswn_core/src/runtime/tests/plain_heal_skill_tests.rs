use super::*;

#[test]
fn manual_heal_uses_the_same_candidates_and_scores_as_automatic_heal() {
    let config = default_custom_runtime_import_config().unwrap();
    let heal = config.registry.skill_id_by_export_name("core.skill.heal").unwrap();
    let original = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "healer", 0, 300, 3)
                .with_magic_point(100)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(heal, 128)])),
            PlayerTemplate::new(2, "ally-a", 0, 300, 3),
            PlayerTemplate::new(3, "ally-b", 0, 300, 3),
            PlayerTemplate::new(4, "enemy", 1, 300, 3),
        ],
        config.registry,
    ));
    let actor = EntityIdx(0);
    let skill = PlannedSkill::Builtin(BuiltinActiveSkill::Heal);
    for smart in [false, true] {
        for lost in [[0, 0, 0], [0, 1, 80], [0, 80, 81], [90, 170, 200]] {
            for seed in 0..8 {
                let mut expected = original.clone();
                expected.rng = RC4::new(&[seed], 1);
                for (index, loss) in lost.into_iter().enumerate() {
                    expected.entities.get_mut(EntityIdx(index as u32)).unwrap().runtime.hp = 300 - loss;
                }
                let mut actual = expected.clone();
                let targets = expected.select_plain_heal_targets(actor, smart);
                actual.control.set_awaiting_forced_action(0, None, smart);
                let preview = actual.preview_current_action_targets(actor, skill, None, true).unwrap();
                assert_eq!(preview, targets.iter().map(|id| id.0 as usize).collect::<Vec<_>>());
                assert_rng_state_eq(&actual.rng, &RC4::new(&[seed], 1));
                if smart && lost.iter().all(|loss| *loss <= 80) {
                    assert!(preview.is_empty());
                }
                actual.queue_planned_action(PlannedAction {
                    actor: 0,
                    skill,
                    target_team: None,
                    magic_cost: Some(0),
                    potion: 0,
                });
                actual.control.set_action_target_teams(&[None], Some(&[true]));
                let prepared = actual.prepare_plain_action(actor, smart, Default::default());
                if targets.is_empty() {
                    assert!(prepared.is_none());
                } else {
                    let Some(PreparedPlainAction::BuiltinSkill(action)) = prepared else {
                        panic!("应执行治愈")
                    };
                    assert_eq!(action.targets, targets);
                }
                assert_rng_state_eq(&actual.rng, &expected.rng);
            }
        }
    }
}

#[test]
fn plain_heal_clearing_ice_preserves_upgrade_runtime_stats() {
    let mut builder = ExtensionRegistryBuilder::default();
    let heal = builder
        .register_skill(
            "core",
            "heal",
            BuiltinActiveSkill::Heal.export_name(),
            TargetPolicy::Ally,
            SkillPriority(15),
        )
        .expect("heal skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "healer", 0, 100, 3)
                .with_magic(12_000)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(heal, 128)])),
            PlayerTemplate::new(2, "target", 0, 300, 3).with_speed(185),
        ],
        registry,
    ));
    let target = EntityIdx(1);
    {
        let target_entity = runtime.entities.get_mut(target).unwrap();
        target_entity.runtime.hp = 100;
        assert!(target_entity.activate_upgrade_runtime());
        target_entity.states.add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 0));
    }
    let mut updates = RunUpdates::new();

    runtime.drain_plain_heal_skill_into(EntityIdx(0), 0, target, &mut updates);

    let target_entity = runtime.entities.get(target).unwrap();
    assert!(target_entity.runtime.upgrade_active);
    assert_eq!(target_entity.runtime.speed, 205);
    assert!(!target_entity.states.is_frozen());
    assert!(updates.updates.iter().any(|update| update.message == "[1]从[冰冻]中解除"));
}

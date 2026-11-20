use super::*;
use crate::runtime::control::TargetWisdom;

#[test]
fn manual_ally_skills_use_the_same_targets_and_rng_as_automatic_skills() {
    let config = default_custom_runtime_import_config().unwrap();
    let skills = [BuiltinActiveSkill::Heal, BuiltinActiveSkill::Haste, BuiltinActiveSkill::Revive];
    let loadout = skills.map(|skill| (config.registry.skill_id_by_export_name(skill.export_name()).unwrap(), 128));
    let original = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "healer", 0, 300, 3)
                .with_magic_point(100)
                .with_skill_loadout(SkillLoadout::from_skill_levels(loadout)),
            PlayerTemplate::new(2, "ally-a", 0, 300, 3).with_target_score_stats(40, 20, 1.0),
            PlayerTemplate::new(3, "ally-b", 0, 300, 3).with_target_score_stats(90, 30, 1.0),
            PlayerTemplate::new(4, "ally-c", 0, 300, 3).with_target_score_stats(70, 10, 1.0),
            PlayerTemplate::new(5, "enemy", 1, 300, 3),
        ],
        config.registry,
    ));
    let actor = EntityIdx(0);
    // 覆盖治愈的损血 80/81，加速的 HP 59/60，以及苏生的零个或多个可复活队友。
    for hp in [
        [300, 300, 300, 300],
        [300, 299, 220, 300],
        [300, 220, 219, 300],
        [210, 130, 100, 75],
        [59, 60, 59, 0],
        [59, 0, 0, 0],
    ] {
        let mut field = original.clone();
        for (index, health) in hp.into_iter().enumerate() {
            let id = EntityIdx(index as u32);
            let entity = field.entities.get_mut(id).unwrap();
            entity.runtime.hp = health;
            if health == 0 {
                entity.runtime.alive = false;
                assert!(field.world.mark_dead(id, 0));
            }
        }
        for builtin in skills {
            let skill = PlannedSkill::Builtin(builtin);
            for smart in [false, true] {
                for seed in 0..8 {
                    let mut expected = field.clone();
                    expected.rng = RC4::new(&[seed], 1);
                    let mut actual = expected.clone();
                    let targets = match builtin {
                        BuiltinActiveSkill::Heal => expected.select_plain_heal_targets(actor, smart),
                        BuiltinActiveSkill::Haste => expected.select_plain_haste_targets(actor, smart),
                        BuiltinActiveSkill::Revive => expected.select_plain_revive_targets(actor, smart),
                        _ => unreachable!(),
                    };
                    actual.control.set_awaiting_forced_action(0, None, smart);
                    let preview = actual
                        .preview_current_action_targets(actor, skill, None, TargetWisdom::WholeAction)
                        .unwrap();
                    assert_eq!(
                        preview,
                        targets.iter().map(|id| id.0 as usize).collect::<Vec<_>>(),
                        "{builtin:?} smart={smart} hp={hp:?} seed={seed}"
                    );
                    assert_rng_state_eq(&actual.rng, &RC4::new(&[seed], 1));
                    if builtin == BuiltinActiveSkill::Heal && smart && hp.iter().all(|v| *v >= 220) {
                        assert!(preview.is_empty());
                    }
                    actual.queue_planned_action(PlannedAction {
                        actor: 0,
                        skill,
                        target_team: None,
                        magic_cost: Some(0),
                        potion: 0,
                    });
                    actual.control.set_action_target_teams(&[None], Some(&[true]), None);
                    let prepared = actual.prepare_plain_action(actor, smart, Default::default());
                    if targets.is_empty() {
                        assert!(prepared.is_none());
                    } else {
                        let Some(PreparedPlainAction::BuiltinSkill(action)) = prepared else {
                            panic!("应执行 {builtin:?}")
                        };
                        assert_eq!(action.targets, targets);
                    }
                    assert_rng_state_eq(&actual.rng, &expected.rng);
                }
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

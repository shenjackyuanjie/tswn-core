use super::*;

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

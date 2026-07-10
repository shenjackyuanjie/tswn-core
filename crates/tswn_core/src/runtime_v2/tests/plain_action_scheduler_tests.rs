use super::*;

fn pre_action_runtime(pre_action_order: [usize; 2]) -> CombatRuntime {
    let mut builder = ExtensionRegistryBuilder::default();
    let assassinate = builder
        .register_skill(
            "core",
            "assassinate",
            BuiltinActiveSkill::Assassinate.export_name(),
            TargetPolicy::Enemy,
            SkillPriority(21),
        )
        .expect("assassinate skill should register");
    let hide = builder
        .register_skill(
            "core",
            "hide",
            DEFAULT_CORE_HIDE_SKILL_EXPORT,
            TargetPolicy::None,
            SkillPriority(34),
        )
        .expect("hide skill should register");
    let registry = builder.build();
    let loadout = SkillLoadout::from_skill_levels([(assassinate, 64), (hide, 64)]).with_pre_action_order(pre_action_order);
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3).with_skill_loadout(loadout),
            PlayerTemplate::new(2, "target", 1, 100, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.assassinate = Some(AssassinateRuntime {
        fixed_lane: 0,
        target: EntityIdx(1),
        break_on_damage: true,
    });
    runtime
}

#[test]
fn plain_pre_action_hide_before_assassinate_keeps_forced_backstab() {
    let mut runtime = pre_action_runtime([1, 0]);

    let outcome = runtime.run_plain_skill_pre_action_accumulator(EntityIdx(0));

    assert_eq!(
        outcome.forced_skill.unwrap().selected,
        SelectedBuiltinSkill {
            skill: BuiltinActiveSkill::Assassinate,
            fixed_lane: 0,
        }
    );
    assert!(!outcome.clear_forced_action);
}

#[test]
fn plain_pre_action_late_hide_clears_forced_backstab_without_dropping_pending() {
    let mut runtime = pre_action_runtime([0, 1]);

    let outcome = runtime.run_plain_skill_pre_action_accumulator(EntityIdx(0));

    assert!(outcome.forced_skill.is_none());
    assert!(!outcome.clear_forced_action);
    assert!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.assassinate.is_some());
}

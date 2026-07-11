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

#[test]
fn plain_merge_restores_newly_enabled_hide_pre_action_lane() {
    let mut builder = ExtensionRegistryBuilder::default();
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
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 100, 3)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(hide, 0)]).with_pre_action_order([])),
            PlayerTemplate::new(2, "target", 1, 100, 3)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(hide, 64)]).with_pre_action_order([0])),
        ],
        registry,
    ));
    let mut updates = RunUpdates::new();

    assert!(runtime.apply_plain_merge_into(EntityIdx(0), EntityIdx(1), &mut updates));

    let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
    assert_eq!(owner.template.skills.level_at(0), Some(64));
    assert_eq!(owner.template.skills.pre_action_order(), &[0]);
    owner.states.add_entry(StateEntry::berserk(PLAIN_BERSERK_STATE_KEY, 2));
    let outcome = runtime.run_plain_skill_pre_action_accumulator(EntityIdx(0));
    assert!(outcome.forced_skill.is_none());
    assert!(outcome.clear_forced_action);
}

#[test]
fn plain_merge_does_not_restore_hide_pre_action_for_minion_lane() {
    let mut builder = ExtensionRegistryBuilder::default();
    let minion_kind = builder
        .register_player_kind_with_policies(
            "core",
            "clone",
            DEFAULT_CORE_CLONE_KIND_EXPORT,
            PlayerKindFlags::MINION,
            PlayerKindPolicies::default(),
        )
        .expect("clone kind should register");
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
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 100, 3),
            PlayerTemplate::with_kind(2, "owner?0", minion_kind, 0, 100, 3)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(hide, 0)]).with_pre_action_order([])),
            PlayerTemplate::new(3, "target", 1, 100, 3)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(hide, 64)]).with_pre_action_order([0])),
        ],
        registry,
    ));
    let minion = EntityIdx(1);
    runtime.entities.get_mut(minion).unwrap().runtime.owner = EntityIdx(0);
    runtime.entities.get_mut(minion).unwrap().runtime.root_owner = EntityIdx(0);
    let mut updates = RunUpdates::new();

    assert!(runtime.apply_plain_merge_into(minion, EntityIdx(2), &mut updates));

    let minion_entity = runtime.entities.get(minion).unwrap();
    assert_eq!(minion_entity.template.skills.level_at(0), Some(64));
    assert!(minion_entity.template.skills.pre_action_order().is_empty());
}

#[test]
fn plain_clone_rebuild_drops_runtime_merge_hide_pre_action_when_level_clamps_to_zero() {
    let mut builder = ExtensionRegistryBuilder::default();
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
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 100, 3)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(hide, 0)]).with_pre_action_order([])),
            PlayerTemplate::new(2, "target", 1, 100, 3)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(hide, 64)]).with_pre_action_order([0])),
        ],
        registry,
    ));
    let mut updates = RunUpdates::new();

    assert!(runtime.apply_plain_merge_into(EntityIdx(0), EntityIdx(1), &mut updates));
    let owner_skills = &runtime.entities.get(EntityIdx(0)).unwrap().template.skills;
    assert_eq!(owner_skills.level_at(0), Some(64));
    assert_eq!(owner_skills.pre_action_order(), &[0]);

    let clone_skills = owner_skills.rebuilt_for_clone();

    assert_eq!(clone_skills.level_at(0), Some(0));
    assert!(clone_skills.pre_action_order().is_empty());
}

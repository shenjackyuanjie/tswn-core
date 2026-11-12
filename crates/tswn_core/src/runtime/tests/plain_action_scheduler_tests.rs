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
fn plain_pending_assassinate_can_be_selected_normally_after_late_hide_clears_forced() {
    let mut runtime = pre_action_runtime([0, 1]);
    assert!(runtime.entities.get_mut(EntityIdx(0)).unwrap().template.skills.set_level_at(0, 128));
    let pre_action = runtime.run_plain_skill_pre_action_accumulator(EntityIdx(0));
    assert!(pre_action.forced_skill.is_none());

    let prepared = runtime
        .scan_plain_action_skill_probabilities(EntityIdx(0), false)
        .expect("pending assassinate should remain eligible in the normal skill scan");

    assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Assassinate);
    assert!(prepared.targets.is_empty());
    let mut updates = RunUpdates::new();
    runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);
    assert_eq!(updates.updates[0].message.as_ref(), "[0]发动[背刺]");
    assert!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.assassinate.is_none());
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
    // 隐匿的 pre-action 是否置 clear_forced_action 由 hide_berserk_compat 决定。
    assert_eq!(outcome.clear_forced_action, !cfg!(feature = "hide_berserk_compat"));
}

#[test]
fn plain_skill_only_merge_refreshes_pending_haste_multiplier() {
    let mut builder = ExtensionRegistryBuilder::default();
    let merged_skill = builder
        .register_skill("test", "merged", "test.skill.merged", TargetPolicy::None, SkillPriority(10))
        .expect("merged skill should register");
    let haste = builder
        .register_state(
            "core",
            "haste",
            DEFAULT_CORE_HASTE_STATE_EXPORT,
            ProcMask::POST_ACTION,
            SkillPriority(210),
        )
        .expect("haste state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 100, 3)
                .with_speed(100)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(merged_skill, 0)])),
            PlayerTemplate::new(2, "target", 1, 100, 3).with_skill_loadout(SkillLoadout::from_skill_levels([(merged_skill, 64)])),
        ],
        registry,
    ));
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::haste_with_effective_faster(77, haste, 4, 2, 9, SkillPriority(210)));
    let mut updates = RunUpdates::new();

    assert!(runtime.apply_plain_merge_into(EntityIdx(0), EntityIdx(1), &mut updates));

    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(owner.template.skills.level_at(0), Some(64));
    assert_eq!(
        owner.states.entry(77).and_then(StateEntry::haste_runtime_value),
        Some((4, 4, 9))
    );
    assert_eq!(owner.effective_speed(), 400);
}

#[test]
fn plain_merge_restores_hide_pre_action_for_minion_lane() {
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
    assert_eq!(minion_entity.template.skills.pre_action_order(), &[0]);
    runtime
        .entities
        .get_mut(minion)
        .unwrap()
        .states
        .add_entry(StateEntry::berserk(PLAIN_BERSERK_STATE_KEY, 2));
    let outcome = runtime.run_plain_skill_pre_action_accumulator(minion);
    // 默认上游语义下隐匿会置 clear_forced_action；`hide_berserk_compat` 打开后不会。
    assert_eq!(outcome.clear_forced_action, !cfg!(feature = "hide_berserk_compat"));
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

#[test]
fn plain_frozen_actor_threshold_action_skips_mp_gate_and_visible_action() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
        PlayerTemplate::new(1, "frozen", 0, 100, 3).with_speed(600).with_magic_point(80),
        PlayerTemplate::new(2, "target", 1, 100, 3).with_speed(1),
    ]));
    let actor = EntityIdx(0);
    runtime
        .entities
        .get_mut(actor)
        .unwrap()
        .states
        .add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 1024));
    runtime.entities.get_mut(actor).unwrap().runtime.move_state.speed_points = crate::runtime::MOVE_POINT_THRESHOLD + 1;

    let outcome = runtime.run_minimal_round_once();

    assert!(outcome.is_none());
    let actor = runtime.entities.get(actor).unwrap();
    assert_eq!(actor.runtime.magic_point, 80);
    assert_eq!(actor.runtime.move_state.speed_points, 1);
    assert!(actor.states.is_frozen());
    assert_eq!(runtime.rng.i, 2);
}

#[test]
fn paused_forced_action_distinguishes_berserk_from_pre_action_cancellation() {
    let actor = EntityIdx(0);
    for cancellation in ["none", "hide", "invalid_assassinate"] {
        let mut runtime = pre_action_runtime([0, 1]);
        let entity = runtime.entities.get_mut(actor).unwrap();
        entity.states.add_entry(StateEntry::berserk(PLAIN_BERSERK_STATE_KEY, 2));
        match cancellation {
            "hide" => entity.runtime.assassinate = None,
            "invalid_assassinate" => {
                entity.template.skills.remove_pre_action_lane(1);
                entity.runtime.assassinate.as_mut().unwrap().target = EntityIdx(999);
            }
            _ => {
                entity.template.skills.remove_pre_action_lane(0);
                entity.template.skills.remove_pre_action_lane(1);
                entity.runtime.assassinate = None;
            }
        }
        let pre_action = runtime.run_plain_skill_pre_action_accumulator(actor);
        assert!(pre_action.forced_skill.is_none());
        // 默认保持上游语义（隐匿也会取消强制行动）；`hide_berserk_compat` 打开后只有
        // 无效潜行目标才会取消。
        let cancels_forced_action = if cfg!(feature = "hide_berserk_compat") {
            cancellation == "invalid_assassinate"
        } else {
            cancellation != "none"
        };
        assert_eq!(pre_action.clear_forced_action, cancels_forced_action);
        assert!(runtime.has_plain_berserk_state(actor));
        let mut automatic = runtime.clone();
        runtime.set_controlled_team(Some(0));
        let before = runtime.rng.clone();
        assert_eq!(runtime.prepare_plain_action(actor, false, pre_action.clone()), None);
        assert_eq!(runtime.awaiting_actor(), Some(0));
        let forced = (!cancels_forced_action).then_some(ForcedAction::Berserk);
        for _ in 0..3 {
            assert_eq!(runtime.control.awaiting_forced_action(), forced);
            assert_rng_state_eq(&runtime.rng, &before);
        }
        assert_eq!(runtime.take_awaiting_actor(), Some(0));
        assert_eq!(runtime.control.awaiting_forced_action(), None);

        // basic 仍只为真正狂暴消费节奏；取消强制后必须允许真正的手动攻击。
        runtime.queue_planned_action(PlannedAction {
            actor: 0,
            skill: PlannedSkill::BasicAttack,
            magic_cost: Some(0),
            potion: 0,
        });
        let prepared = runtime.prepare_plain_action(actor, false, pre_action.clone()).unwrap();
        assert_eq!(runtime.planned_actions_pending(), 0);
        assert_eq!(runtime.awaiting_actor(), None);
        assert_eq!(runtime.control.awaiting_forced_action(), None);
        if forced.is_some() {
            assert!(matches!(prepared, PreparedPlainAction::ForcedAttack { .. }));
            let auto = automatic.prepare_plain_action(actor, false, pre_action).unwrap();
            assert_eq!(auto, prepared);
            assert_rng_state_eq(&runtime.rng, &automatic.rng);
        } else {
            assert!(matches!(prepared, PreparedPlainAction::BasicAttack { use_magic: false, .. }));
        }
    }
}

#[test]
fn berserk_automatic_and_controlled_pacing_keep_full_round_and_rng_identical() {
    let mut automatic = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
        PlayerTemplate::new(1, "berserk", 0, 1000, 3).with_speed(600),
        PlayerTemplate::new(2, "target", 1, 1000, 3).with_speed(1),
    ]));
    let actor = automatic.entities.get_mut(EntityIdx(0)).unwrap();
    actor.states.add_entry(StateEntry::berserk(PLAIN_BERSERK_STATE_KEY, 2));
    actor.runtime.move_state.speed_points = MOVE_POINT_THRESHOLD + 1;
    let mut controlled = automatic.clone();
    controlled.set_controlled_team(Some(0));
    controlled.queue_planned_action(PlannedAction {
        actor: 0,
        skill: PlannedSkill::BasicAttack,
        magic_cost: Some(0),
        potion: 0,
    });
    let expected = automatic.run_minimal_round_once().unwrap();
    let actual = controlled.run_minimal_round_once().unwrap();
    assert_eq!(actual.action, expected.action);
    assert_eq!(
        format!("{:?}", actual.frame.unwrap().updates.updates),
        format!("{:?}", expected.frame.unwrap().updates.updates)
    );
    assert_eq!(actual.winner_team, expected.winner_team);
    assert_rng_state_eq(&controlled.rng, &automatic.rng);
    assert_eq!(
        controlled.entities.get(EntityIdx(0)).unwrap().states,
        automatic.entities.get(EntityIdx(0)).unwrap().states
    );
    assert_eq!(controlled.planned_actions_pending(), 0);
    assert_eq!(controlled.awaiting_actor(), None);
    assert_eq!(controlled.control.awaiting_forced_action(), None);
}

#[test]
fn plain_hide_berserk_forced_action_follows_compat_feature() {
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
    let loadout = SkillLoadout::from_skill_levels([(hide, 64)]).with_pre_action_order([0]);
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 100, 3).with_skill_loadout(loadout),
            PlayerTemplate::new(2, "target", 1, 100, 3),
        ],
        registry,
    ));
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::berserk(PLAIN_BERSERK_STATE_KEY, 4));

    let outcome = runtime.run_plain_skill_pre_action_accumulator(EntityIdx(0));
    assert!(outcome.forced_skill.is_none());
    // 默认保持上游语义（隐匿会取消强制行动）；`hide_berserk_compat` 打开后不再取消。
    let cancels = !cfg!(feature = "hide_berserk_compat");
    assert_eq!(
        outcome.clear_forced_action, cancels,
        "隐匿是否取消强制行动由 hide_berserk_compat 决定"
    );

    if cfg!(feature = "hide_berserk_compat") {
        let prepared = runtime.prepare_plain_action(EntityIdx(0), false, outcome);
        assert!(
            matches!(prepared, Some(PreparedPlainAction::ForcedAttack { .. })),
            "开启 hide_berserk_compat 后仍应执行强制攻击，实际为 {prepared:?}"
        );
    }
}

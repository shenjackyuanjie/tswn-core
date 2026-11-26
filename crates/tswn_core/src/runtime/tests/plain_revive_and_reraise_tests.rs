//! 复活（revive）与护身符（reraise）：目标选择、世界恢复与自动复活 RNG。
use super::*;

/// 按等级构造注册了自动复活技能（DIE hook）的 CombatRuntime。
fn reraise_runtime(level: u32) -> CombatRuntime {
    let mut builder = ExtensionRegistryBuilder::default();
    let reraise = builder
        .register_skill_with_hooks(
            "core",
            "reraise",
            DEFAULT_CORE_RERAISE_SKILL_EXPORT,
            ProcMask::DIE,
            TargetPolicy::None,
            SkillPriority(10),
        )
        .unwrap();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "attacker", 0, 100, 3),
            PlayerTemplate::new(2, "owner", 1, 100, 3).with_skill_loadout(SkillLoadout::from_skill_levels([(reraise, level)])),
        ],
        builder.build(),
    ));
    runtime.set_skill_handler(reraise, run_reraise_die_skill);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 0;
    runtime.control.set_passive_decision_mode(true);
    runtime
}

/// 构造一份 PassiveDecision，activate 与 success 同取 success。
fn decision(actor: usize, skill: &str, success: bool) -> PassiveDecision {
    PassiveDecision {
        actor,
        skill: skill.to_owned(),
        activate: success,
        success,
        target: None,
    }
}

#[test]
fn reraise_is_automatic_in_manual_battles_and_uses_core_rng() {
    for level in [1, 64, 128] {
        let mut runtime = reraise_runtime(level);
        let mut expected_rng = runtime.rng.clone();
        let success = expected_rng.r127() < level;
        let hp = if success { expected_rng.r16() as i32 } else { 0 };
        runtime.drain_die_hooks_into(EntityIdx(1), &mut RunUpdates::new());
        let owner = runtime.entities.get(EntityIdx(1)).unwrap();
        assert_eq!(owner.runtime.hp, hp);
        assert_eq!(
            owner.template.skills.level_at(0),
            Some(if success { (level + 1) >> 1 } else { level })
        );
        assert!(runtime.control.pending_passive().is_none());
        assert_rng_state_eq(&runtime.rng, &expected_rng);
    }
}

#[test]
fn plain_reraise_restores_the_rolled_hp_even_above_max_hp() {
    let mut runtime = reraise_runtime(128);
    let owner = EntityIdx(1);
    runtime.entities.get_mut(owner).unwrap().template.max_hp = 1;
    let mut expected_rng = runtime.rng.clone();
    expected_rng.r127();
    let hp = expected_rng.r16() as i32;
    assert!(hp > 1);

    runtime.drain_die_hooks_into(owner, &mut RunUpdates::new());

    assert_eq!(runtime.entities.get(owner).unwrap().runtime.hp, hp);
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn reraise_replays_old_decisions_without_shifting_following_choices() {
    for success in [false, true] {
        let mut runtime = reraise_runtime(1);
        runtime.control.set_passive_decisions([
            decision(1, DEFAULT_CORE_RERAISE_SKILL_EXPORT, success),
            decision(0, "evade", false),
        ]);
        let mut expected_rng = runtime.rng.clone();
        let hp = if success { expected_rng.r16() as i32 } else { 0 };
        runtime.drain_die_hooks_into(EntityIdx(1), &mut RunUpdates::new());
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, hp);
        assert!(runtime.control.pending_passive().is_none());
        assert_rng_state_eq(&runtime.rng, &expected_rng);
        let next = runtime.control.next_passive_decision().unwrap();
        assert_eq!((next.actor, next.skill.as_str()), (0, "evade"));
    }
}

#[test]
fn automatic_reraise_does_not_consume_another_actor_or_skill_decision() {
    for (actor, skill) in [(0, DEFAULT_CORE_RERAISE_SKILL_EXPORT), (1, "evade")] {
        let mut runtime = reraise_runtime(128);
        runtime.control.set_passive_decisions([decision(actor, skill, false)]);
        runtime.drain_die_hooks_into(EntityIdx(1), &mut RunUpdates::new());
        assert!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp > 0);
        let next = runtime.control.next_passive_decision().unwrap();
        assert_eq!((next.actor, next.skill.as_str()), (actor, skill));
    }
}

#[test]
fn instant_multiplier_scales_reraise_probability_without_scaling_its_level_or_hp() {
    for multiplier in [0.1, 2.0, 100.0] {
        let mut runtime = reraise_runtime(64);
        runtime.control.set_skill_multiplier(multiplier);
        let mut expected_rng = runtime.rng.clone();
        let success = (expected_rng.rFFFFFF() as f64 / 16_777_216.0) < (0.5_f64 * multiplier).min(1.0);
        let hp = if success { expected_rng.r16() as i32 } else { 0 };
        runtime.drain_die_hooks_into(EntityIdx(1), &mut RunUpdates::new());
        let owner = runtime.entities.get(EntityIdx(1)).unwrap();
        assert_eq!(owner.runtime.hp, hp);
        assert_eq!(owner.template.skills.level_at(0), Some(if success { 32 } else { 64 }));
        assert_rng_state_eq(&runtime.rng, &expected_rng);
    }
}

#[test]
fn instant_multiplier_scales_upgrade_trigger_after_the_normal_low_hp_gate() {
    for multiplier in [0.1, 100.0] {
        let mut runtime = reraise_runtime(64);
        runtime.control.set_skill_multiplier(multiplier);
        runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 1;
        let mut expected_rng = runtime.rng.clone();
        expected_rng.r63();
        let success = (expected_rng.rFFFFFF() as f64 / 16_777_216.0) < (0.5_f64 * multiplier).min(1.0);
        runtime.run_plain_upgrade_post_damage_into(EntityIdx(1), 32, 1, EntityIdx(0), &mut RunUpdates::new());
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.upgrade_active, success);
    }
}

#[test]
fn plain_revive_uses_charmed_effective_team_roster() {
    let mut builder = ExtensionRegistryBuilder::default();
    let charm = builder
        .register_state(
            "core",
            "charm",
            DEFAULT_CORE_CHARM_STATE_EXPORT,
            ProcMask::POST_ACTION,
            SkillPriority(210),
        )
        .expect("charm state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "actor", 0, 100, 3),
            PlayerTemplate::new(2, "original-team-dead", 0, 100, 3).with_target_score_stats(1_000, 0, 1.0),
            PlayerTemplate::new(3, "effective-team-dead", 1, 100, 3).with_target_score_stats(1, 0, 1.0),
        ],
        registry,
    ));
    for target in [EntityIdx(1), EntityIdx(2)] {
        let entity = runtime.entities.get_mut(target).unwrap();
        entity.runtime.hp = 0;
        entity.runtime.alive = false;
        assert!(runtime.world.mark_dead(target, entity.runtime.team));
    }
    assert!(
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::charm(
            76,
            charm,
            2,
            Some(1),
            Some(0),
            Some(0),
            2,
            SkillPriority(210),
        ))
    );

    let selected = runtime.select_plain_revive_targets(EntityIdx(0), true);

    assert_eq!(selected.as_slice(), &[EntityIdx(2)]);
}

#[test]
fn plain_revive_without_valid_target_continues_to_clone() {
    let mut builder = ExtensionRegistryBuilder::default();
    let revive = builder
        .register_skill(
            "core",
            "revive",
            BuiltinActiveSkill::Revive.export_name(),
            TargetPolicy::Ally,
            SkillPriority(16),
        )
        .expect("revive skill should register");
    let clone = builder
        .register_skill(
            "core",
            "clone",
            BuiltinActiveSkill::Clone.export_name(),
            TargetPolicy::None,
            SkillPriority(23),
        )
        .expect("clone skill should register");
    let registry = builder.build();
    let loadout = SkillLoadout::from_skill_levels([(revive, 128), (clone, 128)]).with_active_order(vec![0, 1]);
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3).with_skill_loadout(loadout),
            PlayerTemplate::new(2, "ally", 0, 100, 3),
            PlayerTemplate::new(3, "enemy", 1, 100, 3),
        ],
        registry,
    ));

    let prepared = runtime
        .scan_plain_action_skill_probabilities(EntityIdx(0), true)
        .expect("clone should be selected after revive finds no target");

    assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Clone);
    assert_eq!(prepared.selected.fixed_lane, 1);
    assert_eq!(prepared.targets.as_slice(), &[EntityIdx(0)]);
}

#[test]
fn plain_revive_selects_dead_non_minion_ally_by_attr_sum() {
    let mut builder = ExtensionRegistryBuilder::default();
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "minion",
            "custom.minion",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
            PlayerKindPolicies::default(),
        )
        .expect("minion kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3),
            PlayerTemplate::new(2, "weak-dead", 0, 100, 3).with_target_score_stats(10, 0, 1.0),
            PlayerTemplate::new(3, "strong-dead", 0, 100, 3).with_target_score_stats(100, 0, 1.0),
            PlayerTemplate::with_kind(4, "minion-dead", minion_kind, 0, 100, 3).with_target_score_stats(1_000, 0, 1.0),
            PlayerTemplate::new(5, "enemy", 1, 100, 3),
        ],
        registry,
    ));
    for target in [EntityIdx(1), EntityIdx(2), EntityIdx(3)] {
        let entity = runtime.entities.get_mut(target).unwrap();
        entity.runtime.hp = 0;
        entity.runtime.alive = false;
        assert!(runtime.world.mark_dead(target, 0));
    }

    let selected = runtime.select_plain_revive_targets(EntityIdx(0), true);

    assert_eq!(selected.first(), Some(&EntityIdx(2)));
    assert!(selected.contains(&EntityIdx(1)));
    assert!(!selected.contains(&EntityIdx(3)));
}

#[test]
fn plain_revive_can_select_dead_clone_minion() {
    let mut builder = ExtensionRegistryBuilder::default();
    let clone_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "clone",
            "custom.clone",
            PlayerKindFlags::MINION,
            PlayerKindPolicies::default(),
        )
        .expect("clone kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3),
            PlayerTemplate::with_kind(2, "clone", clone_kind, 0, 100, 3),
            PlayerTemplate::new(3, "enemy", 1, 100, 3),
        ],
        registry,
    ));
    {
        let clone = runtime.entities.get_mut(EntityIdx(1)).unwrap();
        clone.runtime.hp = 0;
        clone.runtime.alive = false;
    }
    assert!(runtime.world.mark_dead(EntityIdx(1), 0));

    let selected = runtime.select_plain_revive_targets(EntityIdx(0), true);

    assert!(selected.contains(&EntityIdx(1)));
}

#[test]
fn plain_revive_skips_merge_and_zombie_corpses() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
        PlayerTemplate::new(1, "caster", 0, 100, 3),
        PlayerTemplate::new(2, "merge-corpse", 0, 100, 3),
        PlayerTemplate::new(3, "zombie-corpse", 0, 100, 3),
        PlayerTemplate::new(4, "enemy", 1, 100, 3),
    ]));
    for (target, corpse) in [
        (EntityIdx(1), RuntimeCorpseKind::Merge),
        (EntityIdx(2), RuntimeCorpseKind::Zombie),
    ] {
        let entity = runtime.entities.get_mut(target).unwrap();
        entity.runtime.hp = 0;
        entity.runtime.alive = false;
        entity.runtime.corpse = corpse;
        assert!(runtime.world.mark_dead(target, 0));
    }

    assert!(runtime.select_plain_revive_targets(EntityIdx(0), true).is_empty());
}

#[test]
fn plain_revive_restores_world_emits_legacy_updates_and_halves_level() {
    let mut builder = ExtensionRegistryBuilder::default();
    let revive = builder
        .register_skill(
            "core",
            "revive",
            BuiltinActiveSkill::Revive.export_name(),
            TargetPolicy::Ally,
            SkillPriority(16),
        )
        .expect("revive skill should register");
    let registry = builder.build();
    let loadout = SkillLoadout::from_skill_levels([(revive, 19)]);
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3).with_magic(150).with_skill_loadout(loadout),
            PlayerTemplate::new(2, "target", 0, 200, 3),
            PlayerTemplate::new(3, "enemy", 1, 100, 3),
        ],
        registry,
    ));
    {
        let target = runtime.entities.get_mut(EntityIdx(1)).unwrap();
        target.runtime.hp = 0;
        target.runtime.alive = false;
    }
    assert!(runtime.world.mark_dead(EntityIdx(1), 0));
    let mut expected_rng = runtime.rng.clone();
    let expected_atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng);
    let expected_heal = ((expected_atp / 75.0).ceil() as i32).clamp(1, 200);
    let mut updates = RunUpdates::new();

    runtime.drain_plain_revive_skill_into(EntityIdx(0), 0, EntityIdx(1), &mut updates);

    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert!(target.runtime.alive);
    assert_eq!(target.runtime.hp, expected_heal);
    assert!(runtime.world.round_order().contains(&EntityIdx(1)));
    assert_eq!(runtime.world.team_alive(0), Some(&[EntityIdx(0), EntityIdx(1)][..]));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(1), EntityIdx(2)]);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().template.skills.level_at(0),
        Some(10)
    );
    assert_eq!(
        updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score, update.param))
            .collect::<Vec<_>>(),
        vec![
            ("[0]使用[苏生术]", 1, None),
            ("[1][复活]了", (expected_heal + 60) as u32, None),
            ("[1]回复体力[2]点", 0, Some(expected_heal as u32)),
        ]
    );
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn plain_revive_random_score_consumes_legacy_rffff() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
        PlayerTemplate::new(1, "caster", 0, 100, 3),
        PlayerTemplate::new(2, "target", 0, 100, 3),
    ]));
    let mut expected_rng = runtime.rng.clone();
    let expected = expected_rng.rFFFF() as f64;

    let actual = runtime.score_plain_revive_target(EntityIdx(1), false);

    assert_eq!(actual, expected);
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn plain_reraise_restores_hp_without_readding_round_actor() {
    let mut builder = ExtensionRegistryBuilder::default();
    let reraise = builder
        .register_skill_with_hooks(
            "core",
            "reraise",
            DEFAULT_CORE_RERAISE_SKILL_EXPORT,
            ProcMask::DIE,
            TargetPolicy::None,
            SkillPriority(10),
        )
        .expect("reraise skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "killer", 0, 100, 3),
            PlayerTemplate::new(2, "target", 1, 100, 3).with_skill_loadout(SkillLoadout::from_skill_levels([(reraise, 128)])),
            PlayerTemplate::new(3, "tail", 0, 100, 3),
        ],
        registry,
    ));
    runtime.set_skill_handler(reraise, run_reraise_die_skill);
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(EntityIdx(0)));
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(EntityIdx(1)));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 0;
    let before_order = runtime.world.round_order().to_vec();
    let mut updates = RunUpdates::new();

    runtime.drain_plain_lethal_damage_into(EntityIdx(0), EntityIdx(1), &mut updates);

    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert!(target.runtime.alive);
    assert!(target.runtime.hp > 0);
    assert_eq!(runtime.world.round_order(), before_order.as_slice());
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(EntityIdx(2)));
}

use super::*;

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

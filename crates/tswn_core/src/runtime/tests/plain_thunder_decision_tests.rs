use super::*;

fn runtime(defend: bool) -> CombatRuntime {
    let mut builder = ExtensionRegistryBuilder::default();
    let defend_id = builder
        .register_skill_with_hooks(
            "core",
            "defend",
            DEFAULT_CORE_DEFEND_SKILL_EXPORT,
            ProcMask::POST_DEFEND,
            TargetPolicy::None,
            SkillPriority(10),
        )
        .unwrap();
    let target = PlayerTemplate::new(2, "target", 1, 100_000, 3).with_def_res(0, 80).with_agility(40);
    let target = if defend {
        target.with_skill_loadout(SkillLoadout::from_skill_levels([(defend_id, 32)]))
    } else {
        target
    };
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "caster", 0, 100_000, 3).with_magic(10), target],
        builder.build(),
    ));
    runtime.control.set_passive_decision_mode(true);
    runtime
}

fn evade(activate: bool, success: bool) -> PassiveDecision {
    PassiveDecision {
        actor: 1,
        skill: "evade".into(),
        activate,
        success,
        target: None,
    }
}

#[test]
fn thunder_pauses_identify_cast_hit_and_previous_chance() {
    let mut previous_level = None;
    for hit in 1..=3 {
        let mut runtime = runtime(false);
        runtime.control.set_passive_decisions((1..hit).map(|_| evade(false, false)));
        let mut updates = RunUpdates::new();
        let paused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runtime.drain_plain_thunder_skill_into(EntityIdx(0), EntityIdx(1), &mut updates);
        }));
        assert!(paused.is_err());
        let request = runtime.control.pending_passive().unwrap();
        assert_eq!(request.skill, "evade");
        let context = runtime.control.thunder_context().unwrap();
        assert_eq!((context.id, context.attacker, context.target, context.hit), (1, 0, 1, hit));
        assert!(context.is_evade);
        assert_eq!(context.previous_level, previous_level);
        if let Some(level) = previous_level {
            assert!(request.level >= level);
        }
        previous_level = Some(request.level);
        let partial = runtime.control.take_pending_updates().unwrap();
        assert_eq!(
            partial.updates.iter().filter(|item| item.message.starts_with("[1]受到[2]点伤害")).count(),
            hit as usize - 1
        );
    }
}

#[test]
fn thunder_context_survives_defend_but_is_not_an_evade_request() {
    let mut runtime = runtime(true);
    runtime.control.set_passive_decisions([evade(false, false)]);
    let paused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.drain_plain_thunder_skill_into(EntityIdx(0), EntityIdx(1), &mut RunUpdates::new());
    }));
    assert!(paused.is_err());
    assert_eq!(
        runtime.control.pending_passive().unwrap().skill,
        DEFAULT_CORE_DEFEND_SKILL_EXPORT
    );
    let context = runtime.control.thunder_context().unwrap();
    assert_eq!((context.id, context.hit), (1, 1));
    assert!(!context.is_evade);
}

#[test]
fn thunder_success_stops_later_hits_and_clears_context() {
    let mut runtime = runtime(false);
    runtime
        .control
        .set_passive_decisions([evade(false, false), evade(true, true), evade(false, false)]);
    let mut updates = RunUpdates::new();
    runtime.drain_plain_thunder_skill_into(EntityIdx(0), EntityIdx(1), &mut updates);
    assert!(runtime.control.thunder_context().is_none());
    assert_eq!(
        updates.updates.iter().filter(|item| item.message.starts_with("[1]受到[2]点伤害")).count(),
        1
    );
    assert!(runtime.control.next_passive_decision().is_some());
}

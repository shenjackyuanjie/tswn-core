use super::*;

fn control_runtime(charged: bool) -> CombatRuntime {
    let mut builder = ExtensionRegistryBuilder::default();
    let exchange = builder
        .register_skill(
            "core",
            "exchange",
            BuiltinActiveSkill::Exchange.export_name(),
            TargetPolicy::Enemy,
            SkillPriority(1),
        )
        .unwrap();
    CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3)
                .with_magic(60)
                .with_at_boost(if charged { 3.0 } else { 1.0 })
                .with_skill_loadout(SkillLoadout::from_skill_levels([(exchange, 64)])),
            PlayerTemplate::new(2, "target", 1, 200, 3).with_def_res(1000, 1000).with_agility(1000),
        ],
        builder.build(),
    ))
}

fn cast(runtime: &mut CombatRuntime, exchange: bool) {
    let mut updates = RunUpdates::new();
    if exchange {
        runtime.drain_plain_exchange_skill_into(EntityIdx(0), 0, EntityIdx(1), &mut updates);
    } else {
        runtime.drain_plain_half_skill_into(EntityIdx(0), EntityIdx(1), &mut updates);
    }
}

#[test]
fn charged_exchange_and_half_bypass_manual_evade_without_consuming_its_script() {
    for exchange in [false, true] {
        let mut runtime = control_runtime(true);
        runtime.control.set_passive_decision_mode(true);
        runtime.control.set_passive_decisions([PassiveDecision {
            actor: 1,
            skill: "evade".into(),
            activate: true,
            success: true,
            target: None,
        }]);
        let before_rng = runtime.rng.clone();
        cast(&mut runtime, exchange);
        assert!(runtime.control.pending_passive().is_none());
        assert!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp < 200);
        assert_rng_state_eq(&runtime.rng, &before_rng);
        assert_eq!(runtime.control.next_passive_decision().unwrap().skill, "evade");
    }
}

#[test]
fn uncharged_exchange_and_half_still_request_and_respect_manual_evade() {
    for exchange in [false, true] {
        let mut runtime = control_runtime(false);
        runtime.control.set_passive_decision_mode(true);
        let paused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cast(&mut runtime, exchange)));
        assert!(paused.is_err());
        let pending = runtime.control.pending_passive().unwrap();
        assert_eq!(pending.skill, "evade");
        assert!(!pending.unavoidable);

        let mut runtime = control_runtime(false);
        runtime.control.set_passive_decision_mode(true);
        runtime.control.set_passive_decisions([PassiveDecision {
            actor: 1,
            skill: "evade".into(),
            activate: true,
            success: true,
            target: None,
        }]);
        cast(&mut runtime, exchange);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 200);
    }
}

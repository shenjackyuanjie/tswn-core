use super::*;

fn poison_runtime(hp: i32, atp: f64, count: i32) -> (CombatRuntime, StateId) {
    let mut builder = ExtensionRegistryBuilder::default();
    let poison_state = builder
        .register_state("core", "poison", "core.poison", ProcMask::POST_ACTION, SkillPriority(150))
        .expect("poison state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, hp, 3).with_magic(16),
            PlayerTemplate::new(2, "right", 1, 40, 3),
        ],
        registry,
    ));
    runtime.set_state_handler(poison_state, run_poison_post_action_state);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::poison(
        75,
        poison_state,
        Some(1),
        Some(0),
        atp,
        count,
        SkillPriority(150),
    ));
    (runtime, poison_state)
}

#[test]
fn run_state_hooks_poison_post_action_ticks_damage_and_keeps_state() {
    let (mut runtime, _) = poison_runtime(40, 160.0, 4);

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("poison tick should emit updates");

    let remaining_atp = 160.0 - (160.0 * (1.0 + 3.0 * 0.10000000149011612) / 4.0);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 39);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(0))
            .unwrap()
            .states
            .entry(75)
            .and_then(StateEntry::poison_value),
        Some((Some(1), Some(0), remaining_atp, 3))
    );
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "[1][毒性发作]");
    assert_eq!(frame.updates.updates[0].caster, 1);
    assert_eq!(frame.updates.updates[0].target, 0);
    assert_eq!(frame.updates.updates[1].message, "[1]受到[2]点伤害");
    assert_eq!(frame.updates.updates[1].caster, 1);
    assert_eq!(frame.updates.updates[1].target, 0);
    assert_eq!(frame.updates.updates[1].score, 1);
    assert_eq!(frame.updates.updates[1].delay0, 1002);
}

#[test]
fn run_state_hooks_poison_post_action_clears_and_emits_release_after_tick() {
    let (mut runtime, _) = poison_runtime(40, 80.0, 1);

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("poison clear should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 39);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(75), None);
    assert_eq!(frame.updates.updates.len(), 4);
    assert_eq!(frame.updates.updates[0].message, "[1][毒性发作]");
    assert_eq!(frame.updates.updates[1].message, "[1]受到[2]点伤害");
    assert_eq!(
        frame.updates.updates[2].update_type,
        crate::runtime::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[3].message, "[1]从[中毒]中解除");
    assert_eq!(frame.updates.updates[3].caster, 0);
    assert_eq!(frame.updates.updates[3].target, 0);
}

#[test]
fn run_state_hooks_poison_post_action_emits_death_before_die_hooks() {
    let mut builder = ExtensionRegistryBuilder::default();
    let poison_state = builder
        .register_state("core", "poison", "core.poison", ProcMask::POST_ACTION, SkillPriority(150))
        .expect("poison state should register");
    let die_state = builder
        .register_state("custom", "die", "custom.die", ProcMask::DIE, SkillPriority(0))
        .expect("die state should register");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 3, 3).with_magic(16),
            PlayerTemplate::new(2, "right", 1, 40, 3),
        ],
        builder.build(),
    ));
    runtime.set_state_handler(poison_state, run_poison_post_action_state);
    runtime.set_state_handler(die_state, state_marks_update);
    {
        let states = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
        states.add_entry(StateEntry::poison(
            75,
            poison_state,
            Some(1),
            Some(0),
            240.0,
            1,
            SkillPriority(150),
        ));
        states.add_entry(StateEntry {
            legacy_order_key: 44,
            extension_state_id: Some(die_state),
            hook_mask: ProcMask::DIE,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(1),
            payload: StatePayload::None,
        });
    }

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("lethal poison tick should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 0);
    assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(75), None);
    let visible = frame
        .updates
        .updates
        .iter()
        .filter(|update| !matches!(update.update_type, crate::runtime::update::UpdateType::NextLine))
        .map(|update| (update.message.as_ref(), update.score))
        .collect::<Vec<_>>();
    assert_eq!(
        visible,
        vec![
            ("[1][毒性发作]", 0),
            ("[1]受到[2]点伤害", 3),
            ("[1]被击倒了", 50),
            ("state mark", 44),
        ]
    );
}

#[test]
fn run_state_hooks_poison_post_action_emits_release_after_die_hook_reraises_owner() {
    fn reraise_owner(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
        context.reraise_owner(entry, 5).expect("reraise owner should exist");
    }

    let mut builder = ExtensionRegistryBuilder::default();
    let poison_state = builder
        .register_state("core", "poison", "core.poison", ProcMask::POST_ACTION, SkillPriority(150))
        .expect("poison state should register");
    let reraise_skill = builder
        .register_skill_with_hooks(
            "core",
            "reraise",
            "core.reraise",
            ProcMask::DIE,
            TargetPolicy::None,
            SkillPriority(10),
        )
        .expect("reraise skill should register");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3)
                .with_magic(16)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(reraise_skill, 127)])),
            PlayerTemplate::new(2, "right", 1, 40, 3),
        ],
        builder.build(),
    ));
    runtime.set_state_handler(poison_state, run_poison_post_action_state);
    runtime.set_skill_handler(reraise_skill, reraise_owner);
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.runtime.hp = 3;
        owner.states.add_entry(StateEntry::poison(
            75,
            poison_state,
            Some(1),
            Some(0),
            240.0,
            1,
            SkillPriority(150),
        ));
    }

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("reraise poison tick should emit updates");

    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(owner.runtime.hp, 5);
    assert!(owner.runtime.alive);
    assert_eq!(owner.states.entry(75), None);
    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .filter(|update| update.message == "[1]从[中毒]中解除")
            .count(),
        1
    );
}

#[test]
fn run_state_hooks_poison_post_action_skips_dead_owner_without_mutation() {
    let (mut runtime, _) = poison_runtime(40, 160.0, 4);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.alive = false;

    let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

    assert!(frame.is_none());
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(0))
            .unwrap()
            .states
            .entry(75)
            .and_then(StateEntry::poison_value),
        Some((Some(1), Some(0), 160.0, 4))
    );
}

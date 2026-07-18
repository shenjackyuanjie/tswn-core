use super::*;

fn state_clears_later_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    context.add_update(crate::runtime::update::RunUpdate::new(
        "state clear",
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        entry.legacy_order_key,
    ));
    context.clear_owner_state(22).expect("later state should exist before generation refresh");
}

fn state_marks_update(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    context.add_update(crate::runtime::update::RunUpdate::new(
        "state mark",
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        entry.legacy_order_key,
    ));
}

#[test]
fn state_hook_execution_rebuilds_after_generation_change() {
    let mut builder = ExtensionRegistryBuilder::default();
    let clearer = builder
        .register_state("custom", "clearer", "custom.clearer", ProcMask::POST_ACTION, SkillPriority(0))
        .expect("clearer state should register");
    let later = builder
        .register_state("custom", "later", "custom.later", ProcMask::POST_ACTION, SkillPriority(1))
        .expect("later state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    {
        let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
        store.add_entry(StateEntry {
            legacy_order_key: 11,
            extension_state_id: Some(clearer),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        });
        store.add_entry(StateEntry {
            legacy_order_key: 22,
            extension_state_id: Some(later),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(1),
            registration_order: RegistrationOrder(1),
            payload: StatePayload::None,
        });
    }
    runtime.set_state_handler(clearer, state_clears_later_state);
    runtime.set_state_handler(later, state_marks_update);

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("clearer state should emit update");

    assert_eq!(frame.updates.updates.len(), 1);
    assert_eq!(frame.updates.updates[0].message, "state clear");
    assert!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(22).is_none());
}

use super::*;
use crate::runtime_v2::{ExtensionRegistryBuilder, ProcMask, SkillPriority, StateId, TargetPolicy};

fn damage(amount: i32) -> QueuedEffect {
    QueuedEffect::Damage {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        amount,
    }
}

fn custom(handler: EffectHandlerId, amount: i32) -> QueuedEffect {
    QueuedEffect::Custom(CustomEffect::new(
        handler,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::Int(amount),
    ))
}

#[test]
fn effect_queue_pops_root_effects_in_batch_order() {
    let mut queue = EffectQueue::default();
    queue.push(damage(1));
    queue.push(damage(2));

    assert_eq!(queue.pop_next(), Some(damage(1)));
    assert_eq!(queue.pop_next(), Some(damage(2)));
    assert_eq!(queue.pop_next(), None);
}

#[test]
fn effect_queue_pops_nested_effects_before_older_siblings() {
    let mut queue = EffectQueue::default();
    queue.push(damage(1));
    queue.push(damage(2));

    assert_eq!(queue.pop_next(), Some(damage(1)));
    queue.push_nested(damage(3));

    assert_eq!(queue.pop_next(), Some(damage(3)));
    assert_eq!(queue.pop_next(), Some(damage(2)));
    assert_eq!(queue.pop_next(), None);
}

#[test]
fn effect_queue_pops_custom_effects_in_nested_order() {
    let mut queue = EffectQueue::default();
    queue.push(custom(EffectHandlerId(0), 1));
    queue.push(custom(EffectHandlerId(0), 2));

    assert_eq!(queue.pop_next(), Some(custom(EffectHandlerId(0), 1)));
    queue.push_nested(custom(EffectHandlerId(0), 3));

    assert_eq!(queue.pop_next(), Some(custom(EffectHandlerId(0), 3)));
    assert_eq!(queue.pop_next(), Some(custom(EffectHandlerId(0), 2)));
    assert_eq!(queue.pop_next(), None);
}

#[test]
fn effect_handlers_size_from_registry_and_reject_unknown_ids() {
    let mut builder = ExtensionRegistryBuilder::default();
    let handler = builder
        .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(0))
        .expect("handler should register");
    let registry = builder.build();
    let mut handlers = EffectHandlers::from_registry(&registry);

    handlers.set(handler, |_, _| {});

    assert!(handlers.get(handler).is_some());
    assert!(handlers.get(EffectHandlerId(1)).is_none());
}

#[test]
fn skill_handlers_size_from_registry_and_reject_unknown_ids() {
    let mut builder = ExtensionRegistryBuilder::default();
    let skill = builder
        .register_skill("custom", "mark", "custom.mark", TargetPolicy::Enemy, SkillPriority(0))
        .expect("skill should register");
    let registry = builder.build();
    let mut handlers = SkillHandlers::from_registry(&registry);

    handlers.set(skill, |_, _| {});

    assert!(handlers.get(skill).is_some());
    assert!(handlers.get(SkillId(1)).is_none());
}

#[test]
fn state_handlers_size_from_registry_and_reject_unknown_ids() {
    let mut builder = ExtensionRegistryBuilder::default();
    let state = builder
        .register_state("custom", "burning", "custom.burning", ProcMask::POST_ACTION, SkillPriority(0))
        .expect("state should register");
    let registry = builder.build();
    let mut handlers = StateHandlers::from_registry(&registry);

    handlers.set(state, |_, _| {});

    assert!(handlers.get(state).is_some());
    assert!(handlers.get(StateId(1)).is_none());
}

#[test]
fn renderer_tables_size_from_registry_and_reject_unknown_ids() {
    let mut builder = ExtensionRegistryBuilder::default();
    let replay = builder
        .register_replay_renderer("custom", "replay", "custom.replay", SkillPriority(0))
        .expect("replay renderer should register");
    let show = builder
        .register_show_renderer("custom", "show", "custom.show", SkillPriority(0))
        .expect("show renderer should register");
    let registry = builder.build();
    let mut replay_renderers = ReplayRenderers::from_registry(&registry);
    let mut show_renderers = ShowRenderers::from_registry(&registry);

    replay_renderers.set(replay, |_| None);
    show_renderers.set(show, |_| None);

    assert!(replay_renderers.get(replay).is_some());
    assert!(replay_renderers.get(ReplayRendererId(1)).is_none());
    assert!(show_renderers.get(show).is_some());
    assert!(show_renderers.get(ShowRendererId(1)).is_none());
}

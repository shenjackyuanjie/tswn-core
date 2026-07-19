use super::*;

#[test]
fn entity_arena_preserves_reserved_player_id_holes() {
    let registry = ExtensionRegistry::default();
    let mut arena = EntityArena::from_templates_with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 1),
            PlayerTemplate::new(2, "right", 1, 10, 1),
        ],
        &registry,
    );
    let template = PlayerTemplate::new(0, "summon", 0, 5, 1).with_reserved_player_ids_before_spawn(1);

    let spawned = arena.spawn_from_template_with_owner(template, &registry, Some(EntityIdx(0)), Some(EntityIdx(0)));

    assert_eq!(spawned, EntityIdx(3));
    assert_eq!(arena.len(), 4);
    assert!(arena.get(EntityIdx(2)).is_none());
    assert_eq!(arena.get(spawned).unwrap().template.id, 4);
    assert_eq!(
        arena.iter().map(|(idx, entity)| (idx, entity.template.id)).collect::<Vec<_>>(),
        vec![(EntityIdx(0), 1), (EntityIdx(1), 2), (EntityIdx(3), 4)]
    );
}

#[test]
fn entity_arena_never_reuses_reserved_player_id_holes() {
    let registry = ExtensionRegistry::default();
    let mut arena = EntityArena::from_templates_with_registry(vec![PlayerTemplate::new(1, "owner", 0, 10, 1)], &registry);
    let reserved = PlayerTemplate::new(0, "summon", 0, 5, 1).with_reserved_player_ids_before_spawn(1);

    let summon = arena.spawn_from_template_with_owner(reserved, &registry, Some(EntityIdx(0)), Some(EntityIdx(0)));
    let clone = arena.spawn_from_template_with_owner(
        PlayerTemplate::new(0, "clone", 0, 5, 1),
        &registry,
        Some(EntityIdx(0)),
        Some(EntityIdx(0)),
    );

    assert_eq!(summon, EntityIdx(2));
    assert_eq!(clone, EntityIdx(3));
    assert!(arena.get(EntityIdx(1)).is_none());
    assert_eq!(arena.get(summon).unwrap().template.id, 3);
    assert_eq!(arena.get(clone).unwrap().template.id, 4);
}

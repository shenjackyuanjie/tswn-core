use super::*;

#[test]
fn sync_runtime_entities_adds_pending_linked_shadow_before_same_tick_owner_cleanup() {
    use crate::engine::engine_core::EngineCore;
    use crate::engine::world_state::WorldState;
    use crate::player::skill::act::minion::{MinionKind, MinionRuntimeState};

    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner@team".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);

    let mut clone = storage.get_player(&owner_id).expect("cannot get owner").clone();
    clone.id = storage.new_plr_id();
    clone.name = "owner?0".to_string();
    clone.status.hp = 1;
    clone.status.max_hp = 1;
    clone.status.set_alive(true);
    clone.set_state(MinionRuntimeState {
        owner: Some(owner_id),
        kind: MinionKind::Clone,
    });
    let clone_id = storage.just_insert_player(clone);

    storage.sync_groups(&[vec![owner_id, clone_id]]);
    storage.sync_alive_groups(&[vec![owner_id, clone_id]]);

    let mut world = WorldState::new(vec![vec![owner_id, clone_id]]);
    world.round_pos = 1;

    let mut pending_shadow = storage.get_player(&clone_id).expect("cannot get clone").clone();
    pending_shadow.id = storage.new_plr_id();
    pending_shadow.name = "owner?1".to_string();
    pending_shadow.status.hp = 1;
    pending_shadow.status.max_hp = 1;
    pending_shadow.status.set_alive(true);
    pending_shadow.set_state(MinionRuntimeState {
        owner: Some(clone_id),
        kind: MinionKind::Shadow,
    });
    let pending_shadow_id = pending_shadow.as_ptr();
    storage.queue_spawn(clone_id, pending_shadow);

    let mut randomer = RC4 {
        i: 0,
        j: 0,
        main_val: [0u8; 256],
        ..Default::default()
    };
    let mut updates = RunUpdates::new();

    storage
        .just_get_player_mut(clone_id)
        .expect("cannot get clone mut")
        .damage(999, owner_id, noop_on_damage, &mut randomer, &mut updates, &storage);

    EngineCore::default().sync_runtime_entities(&mut world, &storage);

    assert_eq!(world.players, vec![owner_id]);
    assert_eq!(world.round_pos, -1);
    assert_eq!(world.team_roster(0), Some(&[owner_id, clone_id, pending_shadow_id][..]));
    assert_eq!(world.team_alive(0), Some(&[owner_id][..]));
    assert!(!world.contains_alive(clone_id));
    assert!(!world.contains_alive(pending_shadow_id));

    let shadow = storage.get_player(&pending_shadow_id).expect("pending shadow should be inserted during sync");
    assert!(!shadow.alive());
    assert_eq!(shadow.get_status().hp, 0);
}
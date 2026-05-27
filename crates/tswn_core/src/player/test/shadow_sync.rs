//! 幻影/召唤物同步顺序测试。
//!
//! 覆盖同一 tick 内召唤物新增、宿主清理、关联实体补入世界状态等边界，
//! 确保运行时实体同步不会因为插入顺序丢失待加入的幻影。

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
        #[cfg(not(feature = "no_debug"))]
        byte_count: 0,
    };
    let mut updates = RunUpdates::new();

    storage.just_get_player_mut(clone_id).expect("cannot get clone mut").damage(
        999,
        owner_id,
        noop_on_damage,
        &mut randomer,
        &mut updates,
        &storage,
    );

    EngineCore::default().sync_runtime_entities(&mut world, &storage);

    assert_eq!(world.players, vec![owner_id]);
    assert_eq!(world.round_pos, -1);
    assert_eq!(world.team_roster(0), Some(&[owner_id, clone_id, pending_shadow_id][..]));
    assert_eq!(world.team_alive(0), Some(&[owner_id][..]));
    assert!(!world.contains_alive(clone_id));
    assert!(!world.contains_alive(pending_shadow_id));

    let shadow = storage
        .get_player(&pending_shadow_id)
        .expect("pending shadow should be inserted during sync");
    assert!(!shadow.alive());
    assert_eq!(shadow.get_status().hp, 0);
}

#[test]
fn possess_shadow_self_death_removes_shadow_from_storage_and_world_alive_views() {
    use crate::engine::engine_core::EngineCore;
    use crate::engine::world_state::WorldState;
    use crate::player::skill::SkillTrait;

    let storage = Storage::new_arc();

    let mut owner = Player::new_from_namerena_raw("owner@ally".to_string(), storage.clone()).expect("cannot init owner");
    owner.build();
    let owner_id = storage.just_insert_player(owner);

    let mut target = Player::new_from_namerena_raw("target@enemy".to_string(), storage.clone()).expect("cannot init target");
    target.build();
    let target_id = storage.just_insert_player(target);

    storage.sync_groups(&[vec![owner_id], vec![target_id]]);
    storage.sync_alive_groups(&[vec![owner_id], vec![target_id]]);

    let mut world = WorldState::new(vec![vec![owner_id], vec![target_id]]);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();

    let mut shadow_skill = crate::player::skill::act::shadow::ShadowSkill::new();
    shadow_skill.act_with_level(255, Vec::new(), false, (owner_id, &mut randomer, &mut updates, &storage));

    let shadow_id = storage
        .pending_spawn_ids_for_owner(owner_id)
        .into_iter()
        .next()
        .expect("shadow spawn should be queued");

    EngineCore::default().sync_runtime_entities(&mut world, &storage);

    {
        let target = storage.just_get_player_mut(target_id).expect("cannot get target");
        target.status.set_frozen(true);
    }

    let mut possess = crate::player::skill::act::possess::PossessSkill::new();
    possess.act_with_level(1, vec![target_id], false, (shadow_id, &mut randomer, &mut updates, &storage));

    let shadow_after_possess = storage.get_player(&shadow_id).expect("shadow should exist in storage");
    assert!(
        updates
            .updates
            .iter()
            .any(|update| update.message == "[1]消失了" && update.target == shadow_id),
        "possess should emit shadow disappear log"
    );
    assert!(!shadow_after_possess.alive(), "shadow should be marked dead after possess");
    assert_eq!(
        shadow_after_possess.get_status().hp,
        0,
        "shadow hp should be zero after possess"
    );

    EngineCore::default().sync_runtime_entities(&mut world, &storage);

    assert!(
        !world.contains_alive(shadow_id),
        "dead possess shadow must be removed from world alive view"
    );
    assert_eq!(world.team_alive(0), Some(&[owner_id][..]));
    assert_eq!(storage.alive_group_at(0), Some(&vec![owner_id]));
}

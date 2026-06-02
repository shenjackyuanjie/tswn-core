//! 玩家持续状态测试。
//!
//! 覆盖急速、减速、冰冻、狂暴、魅惑等运行时状态对属性、行动资格和回合推进的影响。

use super::*;

#[test]
fn update_states_applies_haste_slow_and_ice_effects() {
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
    let ptr = player.as_ptr();
    player.attr = [10, 10, 10, 10, 10, 10, 10, 100];
    player.update_states();
    let base_speed = player.get_status().speed;

    player.set_state(crate::player::skill::haste::HasteState {
        owner: Some(ptr),
        target: Some(ptr),
        on_post_action: None,
        faster: 2,
        step: 3,
    });
    player.update_states();
    assert_eq!(player.get_status().speed, base_speed * 2);

    player.clear_state::<crate::player::skill::haste::HasteState>();
    player.set_state(crate::player::skill::slow::SlowState {
        owner: Some(ptr),
        target: Some(ptr),
        on_post_action: None,
        step: 2,
    });
    player.update_states();
    assert_eq!(player.get_status().speed, base_speed / 2);

    player.set_state(crate::player::skill::ice::IceState {
        target: Some(ptr),
        pre_step_impl: None,
        frozen_step: 1024,
    });
    player.update_states();
    assert!(player.get_status().frozed());
}

#[test]
fn clear_negative_states_keeps_positive_states() {
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
    let ptr = player.as_ptr();

    player.set_state(crate::player::skill::haste::HasteState {
        owner: Some(ptr),
        target: Some(ptr),
        on_post_action: None,
        faster: 2,
        step: 3,
    });
    player.set_state(crate::player::skill::slow::SlowState {
        owner: Some(ptr),
        target: Some(ptr),
        on_post_action: None,
        step: 2,
    });
    player.set_state(crate::player::skill::poison::PoisonState {
        caster: Some(ptr),
        target: Some(ptr),
        atp: 12.0,
        count: 4,
    });
    player.clear_negative_states();

    assert!(player.has_state::<crate::player::skill::haste::HasteState>());
    assert!(!player.has_state::<crate::player::skill::slow::SlowState>());
    assert!(!player.has_state::<crate::player::skill::poison::PoisonState>());
}

#[test]
fn clearing_fire_state_does_not_refresh_runtime_status() {
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
    let ptr = player.as_ptr();

    player.set_state(crate::player::skill::haste::HasteState {
        owner: Some(ptr),
        target: Some(ptr),
        on_post_action: None,
        faster: 4,
        step: 3,
    });
    player.set_state(crate::player::skill::fire::FireState { fire_mag: 0.5 });
    player.status.speed = 412;

    // JS 的 FireState 清理只移除火焰层数，不会触发 F()/updateStates。
    // 如果这里刷新属性，speed 会按 HasteState 重新计算，导致行动节奏提前。
    player.clear_negative_states();

    assert!(!player.has_state::<crate::player::skill::fire::FireState>());
    assert!(player.has_state::<crate::player::skill::haste::HasteState>());
    assert_eq!(player.get_status().speed, 412);
}

#[test]
fn clearing_berserk_state_does_not_refresh_pending_haste() {
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
    let ptr = player.as_ptr();

    // 先用 faster=2 建出当前 speed，再把 HasteState.faster 改成 4，
    // 模拟“蓄力值已经变化，但 JS 尚未调用 updateStates 写回属性”的窗口。
    player.set_state(crate::player::skill::haste::HasteState {
        owner: Some(ptr),
        target: Some(ptr),
        on_post_action: None,
        faster: 2,
        step: 3,
    });
    let speed_before_pending = player.get_status().speed;
    player.get_state_mut::<crate::player::skill::haste::HasteState>().unwrap().faster = 4;
    player.set_state_no_update(crate::player::skill::berserk::BerserkState { step: 1 });

    // 清除狂暴属于清负面状态流程；这里不应顺带刷新疾走的 pending 属性。
    player.clear_negative_states();

    assert!(!player.has_state::<crate::player::skill::berserk::BerserkState>());
    assert!(player.has_state::<crate::player::skill::haste::HasteState>());
    assert_eq!(player.get_status().speed, speed_before_pending);
}

#[test]
fn ice_state_pre_step_expires_with_threshold_check() {
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
    let ptr = player.as_ptr();
    let mut updates = RunUpdates::new();

    player.set_move_point(2000);
    player.set_state(crate::player::skill::ice::IceState {
        target: Some(ptr),
        pre_step_impl: None,
        frozen_step: 0,
    });
    let step = player.apply_pre_step_states(100, &mut updates);
    assert_eq!(step, 0);
    assert!(!player.has_state::<crate::player::skill::ice::IceState>());
    assert!(updates.updates.iter().any(|x| x.message.contains("冰冻")));
}

#[test]
fn poison_state_ticks_and_expires_in_post_action() {
    let storage = Storage::new_arc();
    let player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
    let ptr = storage.just_insert_player(player);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();

    let player_mut = storage.just_get_player_mut(ptr).expect("cannot get player from storage");
    player_mut.status.max_hp = 100;
    player_mut.status.hp = 100;
    player_mut.status.magic = 32;
    player_mut.set_state(crate::player::skill::poison::PoisonState {
        caster: Some(ptr),
        target: Some(ptr),
        atp: 80.0,
        count: 1,
    });
    player_mut.action(&mut randomer, &mut updates, &storage, &ActionTargets::default());

    let player_ref = storage.get_player(&ptr).expect("cannot get player from storage");
    assert!(!player_ref.has_state::<crate::player::skill::poison::PoisonState>());
    assert!(updates.updates.iter().any(|x| x.message.contains("[毒性发作]")));
    assert!(updates.updates.iter().any(|x| x.message.contains("从[中毒]中解除")));
}

#[test]
fn clear_positive_runtime_orders_messages_by_meta_key() {
    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.skills.add_skill(Skill::new_with_id(1, 19));
        owner_mut.skills.add_skill(Skill::new_with_id(1, 20));
        owner_mut.skills.update_proc();
    }

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut
            .skills
            .skill_by_id_mut(0)
            .act(vec![owner_id], true, (owner_id, &mut randomer, &mut updates, &storage));
        owner_mut.skills.post_action((owner_id, &mut randomer, &mut updates, &storage));
        owner_mut
            .skills
            .skill_by_id_mut(1)
            .act(vec![owner_id], true, (owner_id, &mut randomer, &mut updates, &storage));
    }

    let cleared = {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut
            .skills
            .clear_positive_runtime_with_order((owner_id, &mut randomer, &mut updates, &storage))
    };

    assert_eq!(cleared, vec![(100, "[1]的[聚气]被打消了"), (200, "[1]的[蓄力]被中止了"),]);
}

#[test]
fn clear_positive_states_orders_messages_by_state_tag() {
    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.status.hp = 100;
        owner_mut.status.alive = true;
        owner_mut.set_state(crate::player::skill::act::haste::HasteState::default());
        owner_mut.set_state(crate::player::skill::act::iron::IronState { protect: 1, step: 1 });
        owner_mut.set_state(crate::player::skill::skl::upgrade::UpgradeState::default());
    }

    let cleared = {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.clear_positive_states_with_ordered_messages()
    };

    assert_eq!(
        cleared,
        vec![
            (300, "[1]从[疾走]中解除"),
            (400, "[1]的[铁壁]被打消了"),
            (500, "[1]的[垂死]属性被打消"),
        ]
    );
}

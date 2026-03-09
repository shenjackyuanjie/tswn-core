//! # 玩家测试 (test)
//!
//! 本模块包含 [`Player`] 的单元测试和集成测试。
//!
//! ## 测试内容
//!
//! - **玩家创建** — 测试根据原始输入创建 Player
//! - **属性计算** — 测试属性计算逻辑
//! - **技能系统** — 测试技能触发和效果
//! - **状态系统** — 测试状态挂载和触发
//! - **武器系统** — 测试武器计算和效果
//! - **Boss 系统** — 测试各种 Boss 的特殊行为
//!
//! ## 测试函数
//!
//! - `player_raw_new()` — 测试根据原始输入创建 Player
//! - 更多测试函数...
//!
//! ## 运行测试
//!
//! ```bash
//! cargo test --package tswn-core --lib player::test
//! ```
//!
//! ## 示例
//!
//! ```rust,ignore
//! #[test]
//! fn player_raw_new() {
//!     let storage = Storage::new_arc();
//!     let player = Player::new_from_namerena_raw("mario".to_string(), storage.clone());
//!     assert_eq!(player.unwrap().name, "mario");
//! }
//! ```

use super::*;
use crate::engine::update::UpdateType;

#[test]
/// 测试根据原始输入创建 Player
fn player_raw_new() {
    let storage = Storage::new_arc();

    let player = Player::new_from_namerena_raw("mario".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.name, "mario");
    assert_eq!(player.team, None);
    assert_eq!(player.weapon, None);
    assert_eq!(player.player_type, PlayerType::Normal);

    let player = Player::new_from_namerena_raw("mario@red".to_string(), storage.clone());
    let player = player.unwrap();
    println!("{}", player);
    assert_eq!(player.name, "mario");
    assert_eq!(player.team, Some("red".to_string()));
    assert_eq!(player.weapon, None);
    assert_eq!(player.player_type, PlayerType::Normal);

    let player = Player::new_from_namerena_raw("mario+fire".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.name, "mario");
    assert_eq!(player.team, None);
    assert_eq!(player.weapon, Some("fire".to_string()));
    assert_eq!(player.player_type, PlayerType::Normal);

    let player = Player::new_from_namerena_raw("mario+fire+diy{xxxx}".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.name, "mario");
    assert_eq!(player.team, None);
    assert_eq!(player.weapon, Some("fire+diy{xxxx}".to_string()));
    assert_eq!(player.player_type, PlayerType::Normal);

    let player = Player::new_from_namerena_raw("mario@red+fire".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.name, "mario");
    assert_eq!(player.team, Some("red".to_string()));
    assert_eq!(player.weapon, Some("fire".to_string()));
    assert_eq!(player.player_type, PlayerType::Normal);

    let player = Player::new_from_namerena_raw("mario@red+fire+diy{xxxx}".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.name, "mario");
    assert_eq!(player.team, Some("red".to_string()));
    assert_eq!(player.weapon, Some("fire+diy{xxxx}".to_string()));
    assert_eq!(player.player_type, PlayerType::Normal);
}

#[test]
fn player_name() {
    let storage = Storage::new_arc();

    let player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
    assert_eq!(player.id_name(), "aaa");
    assert_eq!(player.display_name(), "aaa");

    // 包含了 @
    let player = Player::new_from_namerena_raw("aaa@bbb".to_string(), storage.clone()).unwrap();
    assert_eq!(player.id_name(), "aaa");
    assert_eq!(player.display_name(), "aaa");

    // 空格分开的名字
    let player = Player::new_from_namerena_raw("aaa bbb".to_string(), storage.clone()).unwrap();
    assert_eq!(player.id_name(), "aaa bbb");
    assert_eq!(player.display_name(), "aaa");

    // 包含了 + 的名字
    let player = Player::new_from_namerena_raw("aaa+bbb".to_string(), storage.clone()).unwrap();
    assert_eq!(player.id_name(), "aaa");
    assert_eq!(player.display_name(), "aaa");
}

#[test]
fn player_raw_types() {
    let storage = Storage::new_arc();

    let player = Player::new_from_namerena_raw("normal@normal".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.player_type, PlayerType::Normal);

    // seed
    let player = Player::new_from_namerena_raw("seed:just seed@!".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.name, "seed:just seed");
    assert_eq!(player.player_type, PlayerType::Seed);

    // testEx
    let player = Player::new_from_namerena_raw("testEx@!".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.player_type, PlayerType::TestEx);

    // test1
    let player = Player::new_from_namerena_raw("test1@\u{0002}".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.team, Some("\u{0002}".to_string()));
    assert_eq!(player.player_type, PlayerType::Test1);

    // test2
    let player = Player::new_from_namerena_raw("test2@\u{0003}".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.team, Some("\u{0003}".to_string()));
    assert_eq!(player.player_type, PlayerType::Test2);

    // boss
    let player = Player::new_from_namerena_raw("mario@!".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.player_type, PlayerType::Boss);

    // boosted
    let player = Player::new_from_namerena_raw("云剑狄卡敢@!".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.player_type, PlayerType::Boost);
}

fn noop_on_damage(_: PlrId, _: PlrId, _: i32, _: &mut RC4, _: &mut RunUpdates, _: &std::sync::Arc<Storage>) {}

#[test]
fn check_move_threshold_matches_dart() {
    let mut status = PlayerStatus {
        move_point: MOVE_POINT_THRESHOLD,
        ..Default::default()
    };
    assert!(!status.check_move());
    status.move_point = MOVE_POINT_THRESHOLD + 1;
    assert!(status.check_move());
}

#[test]
fn update_states_does_not_reset_hp_or_mp() {
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();

    player.attr = [10, 20, 30, 40, 50, 60, 70, 80];
    player.init_values();
    assert_eq!(player.status.hp, player.status.max_hp);
    assert_eq!(player.status.mp, player.status.wisdom >> 1);

    player.status.hp = 11;
    player.status.mp = 22;
    player.update_states();

    assert_eq!(player.status.hp, 11);
    assert_eq!(player.status.mp, 22);
}

#[test]
fn upgrade_uses_other_raw_name_base_rules() {
    let storage = Storage::new_arc();
    let mut lhs = Player::new_from_namerena_raw("lhs".to_string(), storage.clone()).unwrap();
    let mut rhs = Player::new_from_namerena_raw("rhs".to_string(), storage.clone()).unwrap();

    lhs.name_base = vec![0; 128];
    rhs.name_base = vec![0; 128];
    lhs.raw_name_base = [0; 128];
    rhs.raw_name_base = [0; 128];

    lhs.raw_name_base[10] = 42;
    lhs.name_base[10] = 50;
    rhs.raw_name_base[9] = 42;
    rhs.raw_name_base[10] = 99;
    rhs.name_base[10] = 1;

    lhs.raw_name_base[8] = 77;
    lhs.name_base[8] = 10;
    rhs.raw_name_base[6] = 77;
    rhs.raw_name_base[8] = 88;
    rhs.name_base[8] = 2;

    lhs.upgrade(&rhs);

    assert_eq!(lhs.name_base[10], 99);
    assert_eq!(lhs.name_base[8], 88);
}

#[test]
fn damage_update_uses_caster_as_actor() {
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
    let caster: PlrId = 999;
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();

    player.status.max_hp = 100;
    player.status.hp = 100;
    let result = player.damage(7, caster, noop_on_damage, &mut randomer, &mut updates, &storage);

    assert_eq!(result, 7);
    assert!(!updates.updates.is_empty());
    let update = updates.updates.last().unwrap();
    assert_eq!(update.caster, caster);
    assert_eq!(update.target, player.as_ptr());
}

#[test]
fn on_damaged_triggers_on_die() {
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();

    player.status.hp = 0;
    player.status.set_alive(true);

    let old_hp = 7;
    let result = player.on_damaged(7, old_hp, player.as_ptr(), &mut randomer, &mut updates, &storage);

    assert_eq!(result, old_hp);
    assert!(!player.status.alive());
    assert_eq!(player.status.hp, 0);
    assert_eq!(updates.updates.len(), 2);
    assert!(matches!(updates.updates[0].update_type, UpdateType::NextLine));
    assert_eq!(updates.updates[1].message, "[1]被击倒了");
}

#[test]
fn check_immune_matches_player_type_rules() {
    let storage = Storage::new_arc();
    let boost = Player::new_from_namerena_raw("云剑穸跄祇@!".to_string(), storage.clone()).unwrap();
    let normal = Player::new_from_namerena_raw("normal".to_string(), storage.clone()).unwrap();
    let mut randomer = RC4 {
        i: 0,
        j: 0,
        main_val: [0u8; 256],
    };

    assert!(boost.check_immune("poison", &mut randomer));
    assert!(!normal.check_immune("poison", &mut randomer));
}

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
fn post_defend_consumes_shield_state() {
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();

    player.set_state(crate::player::skill::shield::ShieldState {
        sort_id: 6000.0,
        target: Some(player.as_ptr()),
        shield: 10,
    });
    let dmg = player.post_defend(6, 999, noop_on_damage, &mut randomer, &mut updates, &storage);
    assert_eq!(dmg, 0);
    assert_eq!(
        player.get_state::<crate::player::skill::shield::ShieldState>().unwrap().shield,
        4
    );
}

#[test]
fn post_defend_applies_curse_multiplier() {
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
    let mut randomer = RC4 {
        i: 0,
        j: 0,
        main_val: [0u8; 256],
    };
    let mut updates = RunUpdates::new();

    player.set_state(crate::player::skill::curse::CurseState {
        owner: Some(777),
        target: Some(player.as_ptr()),
        on_update_state: None,
        prob: 42,
        multiply: 2,
    });
    let dmg = player.post_defend(5, 777, noop_on_damage, &mut randomer, &mut updates, &storage);
    assert_eq!(dmg, 10);
    assert!(updates.updates.iter().any(|x| x.message.contains("诅咒")));
}

#[test]
fn action_expires_berserk_and_charm_states() {
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();

    player.set_state(crate::player::skill::berserk::BerserkState { step: 1 });
    player.set_state(crate::player::skill::charm::CharmState {
        group_id: 1,
        target: Some(player.as_ptr()),
        on_post_action: None,
        step: 1,
    });
    let targets = ActionTargets {
        all_alive: vec![player.as_ptr()].into(),
        ..ActionTargets::default()
    };
    player.action(&mut randomer, &mut updates, &storage, &targets);

    assert!(!player.has_state::<crate::player::skill::berserk::BerserkState>());
    assert!(!player.has_state::<crate::player::skill::charm::CharmState>());
    assert!(updates.updates.iter().any(|x| x.message.contains("狂暴")));
    assert!(updates.updates.iter().any(|x| x.message.contains("魅惑")));
}

#[test]
fn merge_and_zombie_kill_write_target_states() {
    let storage = Storage::new_arc();
    let target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
    let target_id = storage.just_insert_player(target);
    let mut randomer = RC4 {
        i: 0,
        j: 0,
        main_val: [0u8; 256],
    };
    let mut updates = RunUpdates::new();
    let mut merge = crate::player::skill::merge::MergeSkill::new();
    let mut zombie = crate::player::skill::zombie::ZombieSkill::new();

    let merged = <crate::player::skill::merge::MergeSkill as crate::player::skill::SkillTrait>::kill(
        &mut merge,
        target_id,
        (7, &mut randomer, &mut updates, &storage),
    );
    {
        let target_ref = storage.get_player(&target_id).unwrap();
        let corpse = target_ref
            .get_state::<crate::player::skill::corpse::CorpseState>()
            .expect("merge should write corpse state");
        assert_eq!(corpse.kind, crate::player::skill::corpse::CorpseKind::Merge);
    }
    let zombied = <crate::player::skill::zombie::ZombieSkill as crate::player::skill::SkillTrait>::kill(
        &mut zombie,
        target_id,
        (7, &mut randomer, &mut updates, &storage),
    );
    assert!(merged);
    assert!(zombied);
    let target_ref = storage.get_player(&target_id).unwrap();
    let corpse = target_ref
        .get_state::<crate::player::skill::corpse::CorpseState>()
        .expect("zombie should overwrite corpse state");
    assert_eq!(corpse.kind, crate::player::skill::corpse::CorpseKind::Zombie);
}

#[test]
fn action_uses_fire_skill_when_available() {
    let storage = Storage::new_arc();
    let attacker = Player::new_from_namerena_raw("attacker".to_string(), storage.clone()).unwrap();
    let target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
    let attacker_id = storage.just_insert_player(attacker);
    let target_id = storage.just_insert_player(target);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();

    let attacker_mut = storage.just_get_player_mut(attacker_id).unwrap();
    attacker_mut.status.hp = 100;
    attacker_mut.status.max_hp = 100;
    attacker_mut.status.mp = 999;
    attacker_mut.skills.add_skill(Skill::new_with_id(255, 0));
    attacker_mut.skills.update_proc();
    let target_mut = storage.just_get_player_mut(target_id).unwrap();
    target_mut.status.hp = 100;
    target_mut.status.max_hp = 100;

    attacker_mut.action(
        &mut randomer,
        &mut updates,
        &storage,
        &ActionTargets::from_enemy_alive(&[target_id]),
    );
    assert!(updates.updates.iter().any(|x| x.message.contains("火球术")));
}

#[test]
fn heal_action_targets_injured_ally() {
    let storage = Storage::new_arc();
    let healer = Player::new_from_namerena_raw("healer@red".to_string(), storage.clone()).unwrap();
    let ally = Player::new_from_namerena_raw("ally@red".to_string(), storage.clone()).unwrap();
    let enemy = Player::new_from_namerena_raw("enemy@blue".to_string(), storage.clone()).unwrap();
    let healer_id = storage.just_insert_player(healer);
    let ally_id = storage.just_insert_player(ally);
    let enemy_id = storage.just_insert_player(enemy);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();

    let healer_mut = storage.just_get_player_mut(healer_id).unwrap();
    healer_mut.status.mp = 999;
    healer_mut.skills.add_skill(Skill::new_with_id(255, 15));
    healer_mut.skills.update_proc();
    let ally_mut = storage.just_get_player_mut(ally_id).unwrap();
    ally_mut.status.max_hp = 240;
    ally_mut.status.hp = 40;
    let old_ally_hp = ally_mut.status.hp;

    let targets = ActionTargets {
        enemy_alive: vec![enemy_id].into(),
        ally_alive: vec![healer_id, ally_id].into(),
        ally_all: vec![healer_id, ally_id].into(),
        ally_dead: vec![].into(),
        all_alive: vec![healer_id, ally_id, enemy_id].into(),
    };
    healer_mut.action(&mut randomer, &mut updates, &storage, &targets);

    let healed_hp = storage.get_player(&ally_id).unwrap().get_status().hp;
    assert!(healed_hp > old_ally_hp);
    assert!(updates.updates.iter().any(|u| u.message.contains("治愈魔法") && u.target == ally_id));
}

#[test]
fn revive_action_targets_dead_ally() {
    let storage = Storage::new_arc();
    let healer = Player::new_from_namerena_raw("reviver@red".to_string(), storage.clone()).unwrap();
    let ally = Player::new_from_namerena_raw("corpse@red".to_string(), storage.clone()).unwrap();
    let enemy = Player::new_from_namerena_raw("enemy@blue".to_string(), storage.clone()).unwrap();
    let healer_id = storage.just_insert_player(healer);
    let ally_id = storage.just_insert_player(ally);
    let enemy_id = storage.just_insert_player(enemy);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();

    let healer_mut = storage.just_get_player_mut(healer_id).unwrap();
    healer_mut.status.mp = 999;
    healer_mut.skills.add_skill(Skill::new_with_id(255, 16));
    healer_mut.skills.update_proc();
    let ally_mut = storage.just_get_player_mut(ally_id).unwrap();
    ally_mut.status.max_hp = 200;
    ally_mut.status.hp = 0;
    ally_mut.status.set_alive(false);

    let targets = ActionTargets {
        enemy_alive: vec![enemy_id].into(),
        ally_alive: vec![healer_id].into(),
        ally_all: vec![healer_id, ally_id].into(),
        ally_dead: vec![ally_id].into(),
        all_alive: vec![healer_id, enemy_id].into(),
    };
    healer_mut.action(&mut randomer, &mut updates, &storage, &targets);

    let revived = storage.get_player(&ally_id).unwrap();
    assert!(revived.alive());
    assert!(revived.get_status().hp > 0);
    assert!(updates.updates.iter().any(|u| u.message.contains("苏生术") && u.target == ally_id));
}

#[test]
fn protect_redirects_damage_to_protector() {
    let storage = Storage::new_arc();
    let protector = Player::new_from_namerena_raw("protector@red".to_string(), storage.clone()).unwrap();
    let ally = Player::new_from_namerena_raw("ally@red".to_string(), storage.clone()).unwrap();
    let enemy = Player::new_from_namerena_raw("enemy@blue".to_string(), storage.clone()).unwrap();
    let protector_id = storage.just_insert_player(protector);
    let ally_id = storage.just_insert_player(ally);
    let enemy_id = storage.just_insert_player(enemy);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();

    let protector_mut = storage.just_get_player_mut(protector_id).unwrap();
    protector_mut.status.mp = 999;
    protector_mut.status.hp = 300;
    protector_mut.status.max_hp = 300;
    protector_mut.skills.add_skill(Skill::new_with_id(255, 26));
    protector_mut.skills.update_proc();
    let ally_mut = storage.just_get_player_mut(ally_id).unwrap();
    ally_mut.status.hp = 280;
    ally_mut.status.max_hp = 280;

    let targets = ActionTargets {
        enemy_alive: vec![enemy_id].into(),
        ally_alive: vec![protector_id, ally_id].into(),
        ally_all: vec![protector_id, ally_id].into(),
        ally_dead: vec![].into(),
        all_alive: vec![protector_id, ally_id, enemy_id].into(),
    };
    protector_mut.action(&mut randomer, &mut updates, &storage, &targets);
    assert!(
        storage
            .get_player(&ally_id)
            .unwrap()
            .has_state::<crate::player::skill::protect::ProtectState>()
    );

    let protector_hp_before = storage.get_player(&protector_id).unwrap().get_status().hp;
    let ally_hp_before = storage.get_player(&ally_id).unwrap().get_status().hp;
    let mut damage_updates = RunUpdates::new();
    storage.just_get_player_mut(ally_id).unwrap().attacked(
        260.0,
        false,
        enemy_id,
        noop_on_damage,
        &mut randomer,
        &mut damage_updates,
        &storage,
    );

    let protector_hp_after = storage.get_player(&protector_id).unwrap().get_status().hp;
    let ally_hp_after = storage.get_player(&ally_id).unwrap().get_status().hp;
    assert!(protector_hp_after < protector_hp_before);
    assert_eq!(ally_hp_after, ally_hp_before);
    assert!(damage_updates.updates.iter().any(|u| u.message.contains("[守护]")));
}

#[test]
fn action_falls_back_to_default_attack() {
    let storage = Storage::new_arc();
    let attacker = Player::new_from_namerena_raw("attacker".to_string(), storage.clone()).unwrap();
    let target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
    let attacker_id = storage.just_insert_player(attacker);
    let target_id = storage.just_insert_player(target);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();

    let attacker_mut = storage.just_get_player_mut(attacker_id).unwrap();
    attacker_mut.status.hp = 100;
    attacker_mut.status.max_hp = 100;
    attacker_mut.status.mp = 999;
    let target_mut = storage.just_get_player_mut(target_id).unwrap();
    target_mut.status.hp = 100;
    target_mut.status.max_hp = 100;

    attacker_mut.action(
        &mut randomer,
        &mut updates,
        &storage,
        &ActionTargets::from_enemy_alive(&[target_id]),
    );
    assert!(updates.updates.iter().any(|x| x.message.contains("发起攻击")));
}

#[test]
fn reraise_skill_prevents_death() {
    let storage = Storage::new_arc();
    let caster = Player::new_from_namerena_raw("caster".to_string(), storage.clone()).unwrap();
    let target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
    let caster_id = storage.just_insert_player(caster);
    let target_id = storage.just_insert_player(target);
    let mut randomer = RC4 {
        i: 0,
        j: 0,
        main_val: [0u8; 256],
    };
    let mut updates = RunUpdates::new();

    let target_mut = storage.just_get_player_mut(target_id).unwrap();
    target_mut.status.hp = 20;
    target_mut.status.max_hp = 100;
    target_mut.skills.add_skill(Skill::new_with_id(255, 28));
    target_mut.skills.update_proc();

    target_mut.damage(120, caster_id, noop_on_damage, &mut randomer, &mut updates, &storage);
    assert!(target_mut.alive());
    assert!(target_mut.get_status().hp > 0);
    assert!(updates.updates.iter().any(|x| x.message.contains("护身符")));
}

#[test]
fn assassinate_preaction_forces_backstab() {
    let storage = Storage::new_arc();
    let attacker = Player::new_from_namerena_raw("attacker".to_string(), storage.clone()).unwrap();
    let target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
    let attacker_id = storage.just_insert_player(attacker);
    let target_id = storage.just_insert_player(target);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();

    let attacker_mut = storage.just_get_player_mut(attacker_id).unwrap();
    attacker_mut.status.hp = 120;
    attacker_mut.status.max_hp = 120;
    attacker_mut.status.mp = 999;
    attacker_mut.skills.add_skill(Skill::new_with_id(255, 21));
    attacker_mut.skills.update_proc();

    attacker_mut.action(
        &mut randomer,
        &mut updates,
        &storage,
        &ActionTargets::from_enemy_alive(&[target_id]),
    );
    assert!(updates.updates.iter().any(|x| x.message.contains("潜行")));

    let mut updates2 = RunUpdates::new();
    attacker_mut.action(
        &mut randomer,
        &mut updates2,
        &storage,
        &ActionTargets::from_enemy_alive(&[target_id]),
    );
    assert!(updates2.updates.iter().any(|x| x.message.contains("背刺")));
}

#[test]
fn damage_marks_high_damage_thresholds() {
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();
    player.status.hp = 500;
    player.status.max_hp = 500;

    player.damage(130, player.as_ptr(), noop_on_damage, &mut randomer, &mut updates, &storage);
    let hit120 = updates.updates.last().expect("120 damage update missing");
    assert!(hit120.message.contains("s_dmg120"));
    assert_eq!(hit120.delay0, 1260);

    player.status.hp = 500;
    updates.updates.clear();
    player.damage(170, player.as_ptr(), noop_on_damage, &mut randomer, &mut updates, &storage);
    let hit160 = updates.updates.last().expect("160 damage update missing");
    assert!(hit160.message.contains("s_dmg160"));
    assert_eq!(hit160.delay0, 1340);
}

#[test]
fn build_applies_s11_weapon_bonus() {
    let storage = Storage::new_arc();
    let mut base = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
    let mut with_weapon = Player::new_from_namerena_raw("aaa+剁手刀".to_string(), storage.clone()).unwrap();
    base.build();
    with_weapon.build();
    assert_eq!(with_weapon.attr[0], base.attr[0] + 11);
    assert_eq!(with_weapon.attr[2], base.attr[2] + 11);
}

#[test]
fn build_generates_expected_attr_for_help_and_aaaaa() {
    let storage = Storage::new_arc();
    let mut help = Player::new_from_namerena_raw("help".to_string(), storage.clone()).unwrap();
    let mut aaaaa = Player::new_from_namerena_raw("aaaaa".to_string(), storage.clone()).unwrap();

    help.build();
    aaaaa.build();

    assert_eq!(help.attr, [28, 51, 21, 32, 25, 43, 40, 261]);
    assert_eq!(aaaaa.attr, [31, 36, 17, 30, 50, 50, 47, 315]);
}

#[test]
fn boss_has_higher_state_immunity() {
    let storage = Storage::new_arc();
    let boss = Player::new_from_namerena_raw("saitama@!".to_string(), storage.clone()).unwrap();
    let normal = Player::new_from_namerena_raw("normal".to_string(), storage.clone()).unwrap();
    let mut randomer = RC4 {
        i: 0,
        j: 0,
        main_val: [0u8; 256],
    };
    assert!(boss.check_immune("fire", &mut randomer));
    assert!(!normal.check_immune("fire", &mut randomer));
}

#[test]
fn merge_kill_applies_owner_growth() {
    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner".to_string(), storage.clone()).unwrap();
    let target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);
    let target_id = storage.just_insert_player(target);
    let mut randomer = RC4 {
        i: 0,
        j: 0,
        main_val: [0u8; 256],
    };
    let mut updates = RunUpdates::new();

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.status.hp = 200;
        owner_mut.status.max_hp = 200;
        owner_mut.skills.add_skill(Skill::new_with_id(255, 31));
        owner_mut.skills.update_proc();
        owner_mut.update_states();
        owner_mut.skills.update_state((owner_id, &mut randomer, &mut updates, &storage));
    }
    let base_attack = storage.get_player(&owner_id).unwrap().get_status().attack;

    {
        let target_mut = storage.just_get_player_mut(target_id).unwrap();
        target_mut.status.hp = 120;
        target_mut.status.max_hp = 240;
        target_mut.attr = [90, 80, 170, 70, 75, 65, 60, 240];
        target_mut.status.attack = 90;
        target_mut.status.defense = 80;
        target_mut.status.speed = 170;
        target_mut.status.agility = 70;
        target_mut.status.magic = 75;
        target_mut.status.resistance = 65;
        target_mut.status.wisdom = 60;
        target_mut.status.mp = 64;
        target_mut.status.move_point = 512;
        target_mut.status.set_alive(true);
    }

    storage
        .just_get_player_mut(target_id)
        .unwrap()
        .damage(999, owner_id, noop_on_damage, &mut randomer, &mut updates, &storage);

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.update_states();
        owner_mut.skills.update_state((owner_id, &mut randomer, &mut updates, &storage));
        assert!(owner_mut.get_status().attack > base_attack);
        assert!(!owner_mut.has_state::<crate::player::skill::corpse::CorpseState>());
    }
}

#[test]
fn revive_rejects_merge_corpse_target() {
    let storage = Storage::new_arc();
    let reviver = Player::new_from_namerena_raw("reviver@red".to_string(), storage.clone()).unwrap();
    let corpse = Player::new_from_namerena_raw("corpse@red".to_string(), storage.clone()).unwrap();
    let reviver_id = storage.just_insert_player(reviver);
    let corpse_id = storage.just_insert_player(corpse);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();
    let revive = crate::player::skill::revive::ReviveSkill::new();

    {
        let corpse_mut = storage.just_get_player_mut(corpse_id).unwrap();
        corpse_mut.status.hp = 0;
        corpse_mut.status.set_alive(false);
        corpse_mut.set_state(crate::player::skill::corpse::CorpseState::merge());
    }

    let valid = <crate::player::skill::revive::ReviveSkill as crate::player::skill::SkillTrait>::valid_target_with_level(
        &revive,
        32,
        corpse_id,
        false,
        (reviver_id, &mut randomer, &mut updates, &storage),
    );
    assert!(!valid);
}

#[test]
fn zombie_kill_marks_corpse_and_queues_minion_spawn() {
    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner".to_string(), storage.clone()).unwrap();
    let target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);
    let target_id = storage.just_insert_player(target);
    let mut randomer = RC4 {
        i: 0,
        j: 0,
        main_val: [0u8; 256],
    };
    let mut updates = RunUpdates::new();

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.status.hp = 160;
        owner_mut.status.max_hp = 160;
        owner_mut.skills.add_skill(Skill::new_with_id(255, 32));
        owner_mut.skills.update_proc();
        owner_mut.status.mp = 999;
    }

    {
        let target_mut = storage.just_get_player_mut(target_id).unwrap();
        target_mut.status.hp = 100;
        target_mut.status.max_hp = 200;
        target_mut.status.wisdom = 80;
        target_mut.status.set_alive(true);
    }

    storage
        .just_get_player_mut(target_id)
        .unwrap()
        .damage(999, owner_id, noop_on_damage, &mut randomer, &mut updates, &storage);

    {
        let target_mut = storage.just_get_player_mut(target_id).unwrap();
        assert!(!target_mut.alive());
        assert_eq!(target_mut.get_status().hp, 0);
        let corpse = target_mut
            .get_state::<crate::player::skill::corpse::CorpseState>()
            .expect("zombie kill should mark corpse");
        assert_eq!(corpse.kind, crate::player::skill::corpse::CorpseKind::Zombie);
    }
    let pending = storage.take_pending_spawns();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].owner, owner_id);
    assert!(pending[0].player.has_state::<crate::player::skill::act::minion::MinionRuntimeState>());
    assert!(updates.updates.iter().any(|x| x.message.contains("变成了")));
}

#[test]
fn owner_death_marks_linked_minion_for_cleanup() {
    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);
    let mut minion = storage.get_player(&owner_id).expect("cannot get owner").clone();
    minion.id = storage.new_plr_id();
    minion.name = "owner?m".to_string();
    minion.status.hp = 1;
    minion.status.max_hp = 1;
    minion.status.set_alive(true);
    minion.set_state(crate::player::skill::act::minion::MinionRuntimeState {
        owner: Some(owner_id),
        kind: crate::player::skill::act::minion::MinionKind::Summon,
    });
    let minion_id = storage.just_insert_player(minion);
    let mut randomer = RC4 {
        i: 0,
        j: 0,
        main_val: [0u8; 256],
    };
    let mut updates = RunUpdates::new();

    storage
        .just_get_player_mut(owner_id)
        .unwrap()
        .damage(999, owner_id, noop_on_damage, &mut randomer, &mut updates, &storage);

    assert!(!storage.get_player(&minion_id).expect("minion should exist").alive());
    let pending_remove = storage.take_pending_remove_players();
    assert!(pending_remove.contains(&minion_id));
}

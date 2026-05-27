use super::*;

#[test]
fn merge_and_zombie_kill_write_target_states() {
    let storage = Storage::new_arc();
    let target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
    let target_id = storage.just_insert_player(target);
    let mut randomer = RC4 {
        i: 0,
        j: 0,
        main_val: [0u8; 256],
        #[cfg(not(feature = "no_debug"))]
        byte_count: 0,
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
        #[cfg(not(feature = "no_debug"))]
        byte_count: 0,
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
        target_mut.status.magic_point = 64;
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
fn zombie_kill_marks_corpse_and_queues_minion_spawn() {
    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner@same".to_string(), storage.clone()).unwrap();
    let target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);
    let target_id = storage.just_insert_player(target);
    let mut randomer = RC4 {
        i: 0,
        j: 0,
        main_val: [0u8; 256],
        #[cfg(not(feature = "no_debug"))]
        byte_count: 0,
    };
    let mut updates = RunUpdates::new();

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.status.hp = 160;
        owner_mut.status.max_hp = 160;
        owner_mut.skills.add_skill(Skill::new_with_id(255, 32));
        owner_mut.skills.update_proc();
        owner_mut.status.magic_point = 999;
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
    assert_eq!(pending[0].player.id_name(), "owner?0");
    assert_eq!(pending[0].player.id_key_name(), "owner?0@same");
    assert_eq!(pending[0].player.base_name(), "owner?zombie");
    assert_eq!(pending[0].player.display_name(), "丧尸");
    assert_eq!(pending[0].player.get_name_factor(), 0.0);
    assert!(updates.updates.iter().any(|x| x.message.contains("变成了")));
}

#[test]
fn minion_name_counter_uses_root_clone_owner() {
    use crate::player::skill::act::minion::{MinionKind, MinionRuntimeState, alloc_minion_name};

    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner@same".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);

    assert_eq!(alloc_minion_name(&storage, owner_id), "owner?0");
    assert_eq!(alloc_minion_name(&storage, owner_id), "owner?1");

    let mut clone = storage.get_player(&owner_id).expect("cannot get owner").clone();
    clone.id = storage.new_plr_id();
    clone.name = "owner?0".to_string();
    clone.set_state(MinionRuntimeState {
        owner: Some(owner_id),
        kind: MinionKind::Clone,
    });
    let clone_id = storage.just_insert_player(clone);

    assert_eq!(alloc_minion_name(&storage, clone_id), "owner?2");
}

#[test]
fn summon_merge_uses_fixed_skill_slots() {
    let storage = Storage::new_arc();
    let mut summoner = Player::new_from_namerena_raw("地狱之轮 #mW88BamWo@Shabby_fish".to_string(), storage.clone()).unwrap();
    summoner.build();
    let summoner_id = storage.just_insert_player(summoner);

    let mut merge_owner = Player::new_from_namerena_raw("owner".to_string(), storage.clone()).unwrap();
    merge_owner.build();
    let merge_owner_id = storage.just_insert_player(merge_owner);

    {
        let owner_mut = storage.just_get_player_mut(merge_owner_id).unwrap();
        // 这些 skill id 在旧实现里会被“按 id 继承”错误抬高；
        // 现在固定槽位对齐后，应当保持不变。
        owner_mut.skills.skill_by_id_mut(0).set_level(0);
        owner_mut.skills.skill_by_id_mut(1).set_level(0);
        owner_mut.skills.skill_by_id_mut(3).set_level(0);
    }

    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();
    let mut summon = crate::player::skill::summon::SummonSkill::new();
    <crate::player::skill::summon::SummonSkill as crate::player::skill::SkillTrait>::act_with_level(
        &mut summon,
        255,
        vec![summoner_id],
        false,
        (summoner_id, &mut randomer, &mut updates, &storage),
    );

    let pending = storage.take_pending_spawns();
    assert_eq!(pending.len(), 1);
    let summoned = pending.into_iter().next().unwrap().player;
    assert_eq!(summoned.id_key_name(), "地狱之轮 #mW88BamWo?0@Shabby_fish");
    assert_eq!(summoned.base_name(), "地狱之轮 #mW88BamWo?summon");
    assert_eq!(summoned.display_name(), "使魔");
    assert_eq!(summoned.get_name_factor(), 0.0);
    assert!(summoned.skills.store.contains_key(&255));
    assert!(!summoned.skills.store.contains_key(&3));
    let summoned_id = storage.just_insert_player(summoned);

    let mut merge = crate::player::skill::merge::MergeSkill::new();
    assert!(
        <crate::player::skill::merge::MergeSkill as crate::player::skill::SkillTrait>::kill_with_level(
            &mut merge,
            255,
            summoned_id,
            (merge_owner_id, &mut randomer, &mut updates, &storage),
        )
    );

    let owner_after = storage.get_player(&merge_owner_id).unwrap();
    assert_eq!(owner_after.skills.skill_by_id(0).level(), 6);
    assert_eq!(owner_after.skills.skill_by_id(1).level(), 0);
    assert_eq!(owner_after.skills.skill_by_id(3).level(), 0);
}

#[test]
fn merge_inherits_levels_by_fixed_slot_even_when_runtime_kind_differs() {
    let storage = Storage::new_arc();
    let mut owner = Player::new_from_namerena_raw("owner".to_string(), storage.clone()).unwrap();
    owner.build();
    let owner_id = storage.just_insert_player(owner);

    let mut target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
    target.build();
    let target_id = storage.just_insert_player(target);

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.skills.slot_skill = vec![0, 1, 2];
        owner_mut.skills.skill = vec![0, 1, 2];
        owner_mut.skills.skill_by_id_mut(0).set_level(1);
        owner_mut.skills.skill_by_id_mut(1).set_level(1);
        owner_mut.skills.skill_by_id_mut(2).set_level(1);
        owner_mut.skills.skill_by_id_mut(34).set_level(0);
        owner_mut.skills.update_proc();
    }
    {
        let target_mut = storage.just_get_player_mut(target_id).unwrap();
        target_mut.skills.slot_skill = vec![34, 1, 2];
        target_mut.skills.skill = vec![34, 1, 2];
        target_mut.skills.skill_by_id_mut(34).set_level(64);
        target_mut.skills.skill_by_id_mut(1).set_level(7);
        target_mut.skills.skill_by_id_mut(2).set_level(9);
        target_mut.skills.update_proc();
    }

    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();
    let mut merge = crate::player::skill::merge::MergeSkill::new();
    assert!(
        <crate::player::skill::merge::MergeSkill as crate::player::skill::SkillTrait>::kill_with_level(
            &mut merge,
            255,
            target_id,
            (owner_id, &mut randomer, &mut updates, &storage),
        )
    );

    let owner_after = storage.get_player(&owner_id).unwrap();
    assert_eq!(owner_after.skills.skill_by_id(0).level(), 64);
    assert_eq!(owner_after.skills.skill_by_id(34).level(), 0);
    assert_eq!(owner_after.skills.skill_by_id(1).level(), 7);
    assert_eq!(owner_after.skills.skill_by_id(2).level(), 9);
}

#[test]
fn merge_inherits_fengshen_shield_and_registers_pre_action() {
    let storage = Storage::new_arc();
    let mut owner = Player::new_from_namerena_raw("测707640862046T，烦恼立刻消失@爱".to_string(), storage.clone()).unwrap();
    owner.build();
    let owner_id = storage.just_insert_player(owner);

    let mut target = Player::new_from_namerena_raw("Fengshen ONVWTGMPNCKV@nan".to_string(), storage.clone()).unwrap();
    target.build();
    let target_id = storage.just_insert_player(target);

    let owner_before = storage.get_player(&owner_id).unwrap();
    assert_eq!(owner_before.skills.skill_by_id(29).level(), 0);
    assert!(!owner_before.skills.pre_action.contains(&29));
    let target_before = storage.get_player(&target_id).unwrap();
    assert_eq!(target_before.skills.skill_by_id(29).level(), 4);
    assert!(target_before.skills.pre_action.contains(&29));

    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();
    let mut merge = crate::player::skill::merge::MergeSkill::new();
    assert!(
        <crate::player::skill::merge::MergeSkill as crate::player::skill::SkillTrait>::kill_with_level(
            &mut merge,
            255,
            target_id,
            (owner_id, &mut randomer, &mut updates, &storage),
        )
    );

    let owner_after = storage.get_player(&owner_id).unwrap();
    assert_eq!(owner_after.skills.skill_by_id(29).level(), 4);
    assert!(owner_after.skills.pre_action.contains(&29));
}

#[test]
fn post_kill_runs_when_only_remaining_enemy_is_pending_spawn() {
    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner@ally".to_string(), storage.clone()).unwrap();
    let target = Player::new_from_namerena_raw("target@enemy".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);
    let target_id = storage.just_insert_player(target);
    let mut randomer = RC4 {
        i: 0,
        j: 0,
        main_val: [0u8; 256],
        #[cfg(not(feature = "no_debug"))]
        byte_count: 0,
    };
    let mut updates = RunUpdates::new();

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.skills.add_skill(Skill::new_with_id(255, 31));
        owner_mut.skills.update_proc();
        owner_mut.status.hp = 200;
        owner_mut.status.max_hp = 200;
    }

    {
        let target_mut = storage.just_get_player_mut(target_id).unwrap();
        target_mut.status.hp = 30;
        target_mut.status.max_hp = 200;
        target_mut.status.set_alive(true);
        target_mut.attr = [90, 80, 70, 60, 50, 40, 30, 200];
    }

    let mut pending_enemy = Player::new_from_namerena_raw("pending@enemy".to_string(), storage.clone()).unwrap();
    pending_enemy.status.hp = 50;
    pending_enemy.status.max_hp = 50;
    pending_enemy.status.set_alive(true);
    storage.queue_spawn(target_id, pending_enemy);

    storage
        .just_get_player_mut(target_id)
        .unwrap()
        .damage(999, owner_id, noop_on_damage, &mut randomer, &mut updates, &storage);

    assert!(
        updates.updates.iter().any(|update| update.message.contains("吞噬")),
        "pending enemy spawn should keep post_kill callbacks alive",
    );
    assert!(
        storage
            .get_player(&target_id)
            .and_then(|player| player.get_state::<crate::player::skill::corpse::CorpseState>())
            .map(|corpse| corpse.kind == crate::player::skill::corpse::CorpseKind::Merge)
            .unwrap_or(false)
    );
}

#[test]
fn summon_recast_reuses_dead_minion_and_advances_boost_progress() {
    let storage = Storage::new_arc();
    let mut summoner = Player::new_from_namerena_raw("昊寵 #9fzRs7Z1l@Shabby_fish".to_string(), storage.clone()).unwrap();
    summoner.build();
    let summoner_id = storage.just_insert_player(summoner);

    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();
    let mut summon = crate::player::skill::summon::SummonSkill::new();

    <crate::player::skill::summon::SummonSkill as crate::player::skill::SkillTrait>::act_with_level(
        &mut summon,
        255,
        vec![summoner_id],
        false,
        (summoner_id, &mut randomer, &mut updates, &storage),
    );

    let pending = storage.take_pending_spawns();
    assert_eq!(pending.len(), 1);
    let summoned = pending.into_iter().next().unwrap().player;
    let initial_skills = [
        (summoned.skills.skill_by_id(0).level(), summoned.skills.skill_by_id(0).boosted),
        (summoned.skills.skill_by_id(1).level(), summoned.skills.skill_by_id(1).boosted),
        (summoned.skills.skill_by_id(2).level(), summoned.skills.skill_by_id(2).boosted),
    ];
    assert_eq!(
        summoned.skills.skill_by_id(0).level(),
        6,
        "initial summon skills: {:?}",
        initial_skills
    );
    assert_eq!(
        summoned.skills.skill_by_id(1).level(),
        12,
        "initial summon skills: {:?}",
        initial_skills
    );
    assert_eq!(
        summoned.skills.skill_by_id(2).level(),
        22,
        "initial summon skills: {:?}",
        initial_skills
    );
    assert!(summoned.skills.skill_by_id(0).boosted);
    assert!(!summoned.skills.skill_by_id(2).boosted);
    let summoned_id = storage.just_insert_player(summoned);

    {
        let summoned_mut = storage.just_get_player_mut(summoned_id).unwrap();
        summoned_mut.status.hp = 0;
        summoned_mut.status.set_alive(false);
    }

    <crate::player::skill::summon::SummonSkill as crate::player::skill::SkillTrait>::act_with_level(
        &mut summon,
        255,
        vec![summoner_id],
        false,
        (summoner_id, &mut randomer, &mut updates, &storage),
    );

    assert_eq!(storage.pending_spawn_count(), 0);
    assert_eq!(storage.take_pending_revivals(), vec![summoned_id]);

    let summoned_after = storage.get_player(&summoned_id).unwrap();
    let recast_skills = [
        (
            summoned_after.skills.skill_by_id(0).level(),
            summoned_after.skills.skill_by_id(0).boosted,
        ),
        (
            summoned_after.skills.skill_by_id(1).level(),
            summoned_after.skills.skill_by_id(1).boosted,
        ),
        (
            summoned_after.skills.skill_by_id(2).level(),
            summoned_after.skills.skill_by_id(2).boosted,
        ),
    ];
    assert!(summoned_after.alive());
    assert_eq!(
        summoned_after.skills.skill_by_id(0).level(),
        6,
        "recast summon skills: {:?}",
        recast_skills
    );
    assert_eq!(
        summoned_after.skills.skill_by_id(1).level(),
        12,
        "recast summon skills: {:?}",
        recast_skills
    );
    assert_eq!(
        summoned_after.skills.skill_by_id(2).level(),
        44,
        "recast summon skills: {:?}",
        recast_skills
    );
    assert!(summoned_after.skills.skill_by_id(0).boosted);
    assert!(summoned_after.skills.skill_by_id(2).boosted);
}

#[test]
fn shadow_skill_queues_named_shadow_minion() {
    let storage = Storage::new_arc();
    let mut owner = Player::new_from_namerena_raw("owner@same".to_string(), storage.clone()).unwrap();
    owner.build();
    let owner_id = storage.just_insert_player(owner);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();
    let mut shadow = crate::player::skill::act::shadow::ShadowSkill::new();

    <crate::player::skill::act::shadow::ShadowSkill as crate::player::skill::SkillTrait>::act_with_level(
        &mut shadow,
        255,
        Vec::new(),
        false,
        (owner_id, &mut randomer, &mut updates, &storage),
    );

    let pending = storage.take_pending_spawns();
    assert_eq!(pending.len(), 1);
    let shadow = &pending[0].player;
    assert_eq!(shadow.id_name(), "owner?0");
    assert_eq!(shadow.id_key_name(), "owner?0@same");
    assert_eq!(shadow.base_name(), "owner?shadow");
    assert_eq!(shadow.display_name(), "幻影");
    assert_eq!(shadow.get_name_factor(), 0.0);
    assert!(
        updates
            .updates
            .iter()
            .any(|update| update.message == "召唤出[1]" && update.target == shadow.as_ptr())
    );
}

#[test]
fn ol_overlay_customizes_shadow_minion_attrs_and_skills() {
    let storage = Storage::new_arc();
    let raw = r#"owner@same+ol:{"attrs":[86,86,86,86,86,86,86,300],"shadow":{"attrs":[46,47,48,49,50,51,52,200],"skills":{"sklpossess":9}}}"#;
    let mut owner = Player::new_from_namerena_raw(raw.to_string(), storage.clone()).unwrap();
    owner.build();
    let owner_id = storage.just_insert_player(owner);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();
    let mut shadow = crate::player::skill::act::shadow::ShadowSkill::new();

    <crate::player::skill::act::shadow::ShadowSkill as crate::player::skill::SkillTrait>::act_with_level(
        &mut shadow,
        255,
        Vec::new(),
        false,
        (owner_id, &mut randomer, &mut updates, &storage),
    );

    let pending = storage.take_pending_spawns();
    assert_eq!(pending.len(), 1);
    let shadow = &pending[0].player;
    assert_eq!(shadow.attr, [10, 11, 12, 13, 14, 15, 16, 200]);
    assert_eq!(shadow.skills.skill_by_id(0).level(), 9);
    assert_eq!(shadow.skills.skill[0], 0);
    assert!(shadow.skills.is_diy);
}

#[test]
fn ol_overlay_customizes_summon_minion_attrs_and_skills() {
    let storage = Storage::new_arc();
    let raw = r#"owner@same+ol:{"attrs":[86,86,86,86,86,86,86,300],"summon":{"attrs":[50,51,52,53,54,55,56,180],"skills":{"sklfire2":12,"sklexplode":3,"sklfire1":"2*4"}}}"#;
    let mut owner = Player::new_from_namerena_raw(raw.to_string(), storage.clone()).unwrap();
    owner.build();
    let owner_id = storage.just_insert_player(owner);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();
    let mut summon = crate::player::skill::summon::SummonSkill::new();

    <crate::player::skill::summon::SummonSkill as crate::player::skill::SkillTrait>::act_with_level(
        &mut summon,
        255,
        vec![owner_id],
        false,
        (owner_id, &mut randomer, &mut updates, &storage),
    );

    let pending = storage.take_pending_spawns();
    assert_eq!(pending.len(), 1);
    let summoned = &pending[0].player;
    assert_eq!(summoned.attr, [14, 15, 16, 17, 18, 19, 20, 180]);
    assert_eq!(summoned.skills.skill_by_id(0).level(), 8);
    assert_eq!(summoned.skills.skill_by_id(1).level(), 12);
    assert_eq!(summoned.skills.skill_by_id(2).level(), 3);
    assert_eq!(summoned.skills.slot_skill, vec![0, 1, 2]);
    assert_eq!(summoned.skills.skill, vec![1, 2, 0]);
    assert!(summoned.skills.store.contains_key(&255));
    assert!(summoned.skills.is_diy);
}

#[test]
fn exported_summon_overlay_can_inherit_owner_def_res() {
    let storage = Storage::new_arc();
    let raw = r#"owner@same+ol:{"attrs":[86,86,86,86,86,86,86,300],"summon":{"attrs":[50,51,52,53,54,55,56,180],"inherit_owner_def_res":true,"skills":{"sklfire1":12,"sklexplode":3}}}"#;
    let mut owner = Player::new_from_namerena_raw(raw.to_string(), storage.clone()).unwrap();
    owner.build();
    owner.attr[1] = 7;
    owner.attr[5] = 11;
    let owner_id = storage.just_insert_player(owner);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();
    let mut summon = crate::player::skill::summon::SummonSkill::new();

    <crate::player::skill::summon::SummonSkill as crate::player::skill::SkillTrait>::act_with_level(
        &mut summon,
        255,
        vec![owner_id],
        false,
        (owner_id, &mut randomer, &mut updates, &storage),
    );

    let pending = storage.take_pending_spawns();
    assert_eq!(pending.len(), 1);
    let summoned = &pending[0].player;
    assert_eq!(summoned.attr, [14, 7, 16, 17, 18, 11, 20, 180]);
}

#[test]
fn ol_overlay_customizes_zombie_minion_attrs_and_skills() {
    let storage = Storage::new_arc();
    let raw = r#"owner@same+ol:{"attrs":[86,86,86,86,86,86,86,300],"skills":{"sklzombie":255},"zombie":{"attrs":[40,41,42,43,44,45,46,90],"skills":{"sklrapid":7}}}"#;
    let mut owner = Player::new_from_namerena_raw(raw.to_string(), storage.clone()).unwrap();
    owner.build();
    let owner_id = storage.just_insert_player(owner);
    let target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
    let target_id = storage.just_insert_player(target);
    let mut randomer = RC4 {
        i: 0,
        j: 0,
        main_val: [0u8; 256],
        #[cfg(not(feature = "no_debug"))]
        byte_count: 0,
    };
    let mut updates = RunUpdates::new();
    let mut zombie_skill = crate::player::skill::zombie::ZombieSkill::new();

    assert!(
        <crate::player::skill::zombie::ZombieSkill as crate::player::skill::SkillTrait>::kill_with_level(
            &mut zombie_skill,
            255,
            target_id,
            (owner_id, &mut randomer, &mut updates, &storage),
        )
    );

    let pending = storage.take_pending_spawns();
    assert_eq!(pending.len(), 1);
    let zombie = &pending[0].player;
    assert_eq!(zombie.attr, [4, 5, 6, 7, 8, 9, 10, 90]);
    assert_eq!(zombie.skills.skill_by_id(0).level(), 7);
    assert_eq!(zombie.skills.skill, vec![0]);
    assert!(zombie.skills.is_diy);
}

#[test]
fn summon_recast_refreshes_charge_dependent_state_on_reused_minion() {
    let storage = Storage::new_arc();
    let mut summoner = Player::new_from_namerena_raw("昊寵 #9fzRs7Z1l@Shabby_fish".to_string(), storage.clone()).unwrap();
    summoner.build();
    let summoner_id = storage.just_insert_player(summoner);

    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();
    let mut summon = crate::player::skill::summon::SummonSkill::new();

    <crate::player::skill::summon::SummonSkill as crate::player::skill::SkillTrait>::act_with_level(
        &mut summon,
        255,
        vec![summoner_id],
        false,
        (summoner_id, &mut randomer, &mut updates, &storage),
    );

    let pending = storage.take_pending_spawns();
    assert_eq!(pending.len(), 1);
    let summoned = pending.into_iter().next().unwrap().player;
    assert_eq!(summoned.skills.skill_by_id(255).level(), 1);
    let summoned_id = storage.just_insert_player(summoned);

    {
        let owner_mut = storage.just_get_player_mut(summoner_id).unwrap();
        owner_mut.status.at_boost = 3.0;
    }
    {
        let summoned_mut = storage.just_get_player_mut(summoned_id).unwrap();
        summoned_mut.status.hp = 0;
        summoned_mut.status.set_alive(false);
    }

    <crate::player::skill::summon::SummonSkill as crate::player::skill::SkillTrait>::act_with_level(
        &mut summon,
        255,
        vec![summoner_id],
        false,
        (summoner_id, &mut randomer, &mut updates, &storage),
    );

    assert_eq!(storage.pending_spawn_count(), 0);
    assert_eq!(storage.take_pending_revivals(), vec![summoned_id]);

    let summoned_after = storage.get_player(&summoned_id).unwrap();
    assert!(summoned_after.alive());
    assert_eq!(summoned_after.skills.skill_by_id(255).level(), 0);
    assert_eq!(summoned_after.status.move_point, 2048);
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
        #[cfg(not(feature = "no_debug"))]
        byte_count: 0,
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

#[test]
fn owner_death_marks_pending_linked_minion_dead_before_sync() {
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

    let mut pending_shadow = storage.get_player(&clone_id).expect("cannot get clone").clone();
    pending_shadow.id = storage.new_plr_id();
    pending_shadow.name = "owner?1".to_string();
    pending_shadow.sort_int = 0;
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

    storage
        .just_get_player_mut(clone_id)
        .unwrap()
        .damage(999, owner_id, noop_on_damage, &mut randomer, &mut updates, &storage);

    let pending_shadow = storage
        .get_pending_spawn_player(pending_shadow_id)
        .expect("pending shadow should remain available until sync");
    assert!(!pending_shadow.alive());
    assert_eq!(pending_shadow.get_status().hp, 0);
    assert_eq!(pending_shadow.sort_int, 0);
    assert!(
        updates
            .updates
            .iter()
            .any(|update| update.message == "[1]消失了" && update.target == pending_shadow_id)
    );
}

#[test]
fn pending_shadow_uses_runtime_sort_int_zero() {
    use crate::player::skill::act::minion::{MinionKind, MinionRuntimeState};

    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner@team".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);

    let mut shadow = storage.get_player(&owner_id).expect("cannot get owner").clone();
    shadow.id = storage.new_plr_id();
    shadow.name = "owner?shadow".to_string();
    shadow.sort_int = 0;
    shadow.status.set_alive(true);
    shadow.set_state(MinionRuntimeState {
        owner: Some(owner_id),
        kind: MinionKind::Shadow,
    });
    let shadow_id = shadow.as_ptr();
    storage.queue_spawn(owner_id, shadow);

    let pending_shadow = storage
        .get_pending_spawn_player(shadow_id)
        .expect("pending shadow should exist before sync");
    assert_eq!(pending_shadow.sort_int, 0);
}

#[test]
fn owner_death_removes_linked_minions_in_roster_order() {
    use crate::player::skill::act::minion::{MinionKind, MinionRuntimeState};

    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner@team".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);

    // 按 ?0 -> ?1 的顺序插入，同一队伍 roster 顺序应保持该顺序。
    let mut minion0 = storage.get_player(&owner_id).expect("cannot get owner").clone();
    minion0.id = storage.new_plr_id();
    minion0.name = "owner?0".to_string();
    minion0.status.hp = 1;
    minion0.status.max_hp = 1;
    minion0.status.set_alive(true);
    minion0.set_state(MinionRuntimeState {
        owner: Some(owner_id),
        kind: MinionKind::Summon,
    });
    let minion0_id = storage.just_insert_player(minion0);

    let mut minion1 = storage.get_player(&owner_id).expect("cannot get owner").clone();
    minion1.id = storage.new_plr_id();
    minion1.name = "owner?1".to_string();
    minion1.status.hp = 1;
    minion1.status.max_hp = 1;
    minion1.status.set_alive(true);
    minion1.set_state(MinionRuntimeState {
        owner: Some(owner_id),
        kind: MinionKind::Summon,
    });
    let minion1_id = storage.just_insert_player(minion1);

    // 构造与实际运行一致的分组（owner, ?0, ?1）。
    storage.sync_groups(&[vec![owner_id, minion0_id, minion1_id]]);
    storage.sync_alive_groups(&[vec![owner_id, minion0_id, minion1_id]]);

    let mut randomer = RC4 {
        i: 0,
        j: 0,
        main_val: [0u8; 256],
        #[cfg(not(feature = "no_debug"))]
        byte_count: 0,
    };
    let mut updates = RunUpdates::new();

    storage
        .just_get_player_mut(owner_id)
        .unwrap()
        .damage(999, owner_id, noop_on_damage, &mut randomer, &mut updates, &storage);

    let pending_remove = storage.take_pending_remove_players();
    assert_eq!(pending_remove, vec![minion0_id, minion1_id]);
}

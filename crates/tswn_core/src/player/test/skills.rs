use super::*;

#[test]
fn protect_post_action_consumes_smart_roll_for_empty_charm_alive_group() {
    use crate::player::skill::Skill;

    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner@red".to_string(), storage.clone()).unwrap();
    let enemy = Player::new_from_namerena_raw("enemy@blue".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);
    let enemy_id = storage.just_insert_player(enemy);
    storage.sync_groups(&[vec![owner_id], vec![enemy_id]]);
    storage.sync_alive_groups(&[vec![owner_id], vec![]]);

    let protect_key;
    {
        let enemy_mut = storage.just_get_player_mut(enemy_id).unwrap();
        enemy_mut.status.hp = 0;
        enemy_mut.status.set_alive(false);

        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.set_state(crate::player::skill::act::charm::CharmState {
            group_id: enemy_id,
            effective_team_idx: None,
            target: Some(enemy_id),
            on_post_action: None,
            step: 1,
        });
        protect_key = owner_mut.skills.skill.len();
        owner_mut.skills.add_skill(Skill::new_with_id(7, 26));
    }

    let mut randomer = RC4::default();
    let before = (randomer.i, randomer.j);
    let mut updates = RunUpdates::new();
    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut
            .skills
            .skill_by_id_mut(protect_key)
            .post_action((owner_id, &mut randomer, &mut updates, &storage));
    }

    assert_eq!(randomer.i, before.0.wrapping_add(1));
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
        ..Default::default()
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
fn hide_requires_real_alive_group_instead_of_clan_fallback() {
    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner@same".to_string(), storage.clone()).unwrap();
    let ally = Player::new_from_namerena_raw("ally@same".to_string(), storage.clone()).unwrap();
    let attacker = Player::new_from_namerena_raw("attacker@other".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);
    let ally_id = storage.just_insert_player(ally);
    let attacker_id = storage.just_insert_player(attacker);

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.status.hp = 120;
        owner_mut.status.max_hp = 120;
        owner_mut.skills.add_skill(Skill::new_with_id(127, 34));
        owner_mut.skills.update_proc();
    }

    storage.sync_groups(&[vec![ally_id], vec![attacker_id]]);
    storage.sync_alive_groups(&[vec![ally_id], vec![attacker_id]]);

    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();
    storage
        .just_get_player_mut(owner_id)
        .unwrap()
        .damage(8, attacker_id, noop_on_damage, &mut randomer, &mut updates, &storage);

    assert!(!updates.updates.iter().any(|x| x.message.contains("发动隐匿")));
}

#[test]
fn hide_counts_pending_shadow_as_alive_ally_before_sync() {
    use crate::player::skill::act::minion::{MinionKind, MinionRuntimeState};

    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner@same".to_string(), storage.clone()).unwrap();
    let attacker = Player::new_from_namerena_raw("attacker@other".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);
    let attacker_id = storage.just_insert_player(attacker);

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.status.hp = 120;
        owner_mut.status.max_hp = 120;
        owner_mut.skills.add_skill(Skill::new_with_id(64, 34));
        owner_mut.skills.update_proc();
    }

    storage.sync_groups(&[vec![owner_id], vec![attacker_id]]);
    storage.sync_alive_groups(&[vec![owner_id], vec![attacker_id]]);

    let mut pending_shadow = storage.get_player(&owner_id).expect("cannot get owner").clone();
    pending_shadow.id = storage.new_plr_id();
    pending_shadow.name = "owner?0".to_string();
    pending_shadow.status.hp = 1;
    pending_shadow.status.max_hp = 1;
    pending_shadow.status.set_alive(true);
    pending_shadow.set_state(MinionRuntimeState {
        owner: Some(owner_id),
        kind: MinionKind::Shadow,
    });
    storage.queue_spawn(owner_id, pending_shadow);

    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();
    storage
        .just_get_player_mut(owner_id)
        .unwrap()
        .damage(8, attacker_id, noop_on_damage, &mut randomer, &mut updates, &storage);

    assert!(updates.updates.iter().any(|x| x.message.contains("[隐匿]")));
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
        effective_team_idx: None,
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
        ..Default::default()
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
fn curse_does_not_apply_to_reraise_survivor() {
    let storage = Storage::new_arc();
    let attacker = Player::new_from_namerena_raw("attacker".to_string(), storage.clone()).unwrap();
    let target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
    let attacker_id = storage.just_insert_player(attacker);
    let target_id = storage.just_insert_player(target);
    let mut randomer = RC4 {
        i: 0,
        j: 0,
        main_val: [0u8; 256],
        ..Default::default()
    };
    let mut updates = RunUpdates::new();

    let attacker_mut = storage.just_get_player_mut(attacker_id).unwrap();
    attacker_mut.status.mp = 999;
    attacker_mut.skills.add_skill(Skill::new_with_id(127, 14));
    attacker_mut.skills.update_proc();

    let target_mut = storage.just_get_player_mut(target_id).unwrap();
    target_mut.status.hp = 1;
    target_mut.status.max_hp = 100;
    target_mut.skills.add_skill(Skill::new_with_id(127, 28));
    target_mut.skills.update_proc();

    attacker_mut.action(
        &mut randomer,
        &mut updates,
        &storage,
        &ActionTargets::from_enemy_alive(&[target_id]),
    );

    let target_plr = storage.get_player(&target_id).unwrap();
    assert!(target_plr.alive());
    assert!(target_plr.get_status().hp > 0);
    assert!(!target_plr.has_state::<crate::player::skill::curse::CurseState>());
    assert!(!updates.updates.iter().any(|x| x.message.contains("被诅咒了")));
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
fn skill_act_preserves_level_upgrades_happening_mid_act() {
    #[derive(Debug, Clone)]
    struct MidActUpgradeSkill {
        skill_key: usize,
        upgraded_level: u32,
    }

    impl crate::player::skill::SkillTrait for MidActUpgradeSkill {
        fn destroy(&self, _plr: PlrId, _args: crate::player::skill::SkillArgs) {}

        fn clone_box(&self) -> Box<dyn crate::player::skill::SkillTrait> { Box::new(self.clone()) }

        fn act_with_level(&mut self, _level: u32, _targets: Vec<PlrId>, _smart: bool, args: crate::player::skill::SkillArgs) {
            args.3
                .just_get_player_mut(args.0)
                .unwrap()
                .skills
                .skill_by_id_mut(self.skill_key)
                .set_level(self.upgraded_level);
        }
    }

    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();

    let skill_key = {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        let skill_key = owner_mut.skills.skill.len();
        owner_mut.skills.add_skill(crate::player::skill::Skill::new(
            76,
            Box::new(MidActUpgradeSkill {
                skill_key,
                upgraded_level: 100,
            }),
        ));
        skill_key
    };

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut
            .skills
            .skill_by_id_mut(skill_key)
            .act(Vec::new(), true, (owner_id, &mut randomer, &mut updates, &storage));
    }

    let owner_after = storage.get_player(&owner_id).unwrap();
    assert_eq!(owner_after.skills.skill_by_id(skill_key).level(), 100);
}

#[test]
fn iron_break_refreshes_attract_immediately() {
    let storage = Storage::new_arc();
    let mut owner = Player::new_from_namerena_raw("owner".to_string(), storage.clone()).unwrap();
    let mut caster = Player::new_from_namerena_raw("caster".to_string(), storage.clone()).unwrap();
    owner.build();
    caster.build();
    let owner_id = storage.just_insert_player(owner);
    let caster_id = storage.just_insert_player(caster);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.set_state(crate::player::skill::act::iron::IronState { protect: 5, step: 1 });
    }

    let boosted_attract = storage.get_player(&owner_id).unwrap().get_status().attract;
    assert!(boosted_attract > 32768.0);

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        let dmg = owner_mut.post_defend(10, caster_id, noop_on_damage, &mut randomer, &mut updates, &storage);
        assert_eq!(dmg, 5);
    }

    let owner_after = storage.get_player(&owner_id).unwrap();
    let iron = owner_after.get_state::<crate::player::skill::act::iron::IronState>().unwrap();
    assert_eq!(iron.protect, 0);
    assert_eq!(iron.step, 0);
    assert_eq!(owner_after.get_status().attract, 32768.0);
}

#[test]
fn accumulate_with_charge_gains_bonus_move_point_and_boost() {
    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.skills.add_skill(Skill::new_with_id(1, 19)); // Charge
        owner_mut.skills.add_skill(Skill::new_with_id(1, 20)); // Accumulate
        owner_mut.skills.update_proc();
        owner_mut.update_states();
        owner_mut.status.move_point = 100;
        owner_mut.status.mp = 100;
    }

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut
            .skills
            .skill_by_id_mut(0)
            .act(vec![owner_id], true, (owner_id, &mut randomer, &mut updates, &storage));
        owner_mut.skills.post_action((owner_id, &mut randomer, &mut updates, &storage));
    }

    let move_before_accumulate = storage.get_player(&owner_id).unwrap().move_point();
    assert_eq!(move_before_accumulate, 100);

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut
            .skills
            .skill_by_id_mut(1)
            .act(vec![owner_id], true, (owner_id, &mut randomer, &mut updates, &storage));
    }

    assert_eq!(
        storage.get_player(&owner_id).unwrap().move_point(),
        move_before_accumulate + 900
    );

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.skills.post_action((owner_id, &mut randomer, &mut updates, &storage));
    }

    let boosted = storage.get_player(&owner_id).unwrap().get_status().at_boost;
    assert!((boosted - 2.7000000476837158).abs() < 1e-6);

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        let cleared =
            owner_mut
                .skills
                .skill_by_id_mut(1)
                .clear_positive_runtime((owner_id, &mut randomer, &mut updates, &storage));
        assert_eq!(cleared, Some("[1]的[聚气]被打消了"));
    }

    let cleared_boost = storage.get_player(&owner_id).unwrap().get_status().at_boost;
    assert!((cleared_boost - 1.0).abs() < 1e-6);
}

#[test]
fn accumulate_reuse_after_clear_uses_js_reset_multiplier() {
    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.skills.add_skill(Skill::new_with_id(1, 20)); // Accumulate
        owner_mut.skills.update_proc();
        owner_mut.update_states();
        owner_mut.status.move_point = 100;
        owner_mut.status.mp = 100;
    }

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut
            .skills
            .skill_by_id_mut(0)
            .act(vec![owner_id], true, (owner_id, &mut randomer, &mut updates, &storage));
    }

    let first_boost = storage.get_player(&owner_id).unwrap().get_status().at_boost;
    assert!((first_boost - 1.7000000476837158).abs() < 1e-6);

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        let cleared =
            owner_mut
                .skills
                .skill_by_id_mut(0)
                .clear_positive_runtime((owner_id, &mut randomer, &mut updates, &storage));
        assert_eq!(cleared, Some("[1]的[聚气]被打消了"));
    }

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut
            .skills
            .skill_by_id_mut(0)
            .act(vec![owner_id], true, (owner_id, &mut randomer, &mut updates, &storage));
    }

    let second_boost = storage.get_player(&owner_id).unwrap().get_status().at_boost;
    assert!((second_boost - 1.600000023841858).abs() < 1e-6);
}

#[test]
fn charge_post_action_tail_runs_after_state_ticks() {
    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);
    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.skills.add_skill(Skill::new_with_id(1, 19)); // Charge
        owner_mut.skills.update_proc();
        owner_mut.update_states();
        owner_mut
            .skills
            .skill_by_id_mut(0)
            .act(vec![owner_id], true, (owner_id, &mut randomer, &mut updates, &storage));
        owner_mut.skills.post_action((owner_id, &mut randomer, &mut updates, &storage));
        owner_mut.set_state(ObserveChargeBoostState);
    }

    assert!(storage.get_player(&owner_id).unwrap().get_status().at_boost > 1.0);

    let mut final_updates = RunUpdates::new();
    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.run_post_action_chain(&mut randomer, &mut final_updates, &storage);
    }

    let observed: Vec<&str> = final_updates.updates.iter().map(|update| update.message.as_ref()).collect();
    assert_eq!(observed, vec!["observe_charge_boost=boosted"]);
    assert!((storage.get_player(&owner_id).unwrap().get_status().at_boost - 1.0).abs() < 1e-6);
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
fn ice_score_halves_already_frozen_targets() {
    let storage = Storage::new_arc();
    let caster = Player::new_from_namerena_raw("caster@a".to_string(), storage.clone()).unwrap();
    let iced = Player::new_from_namerena_raw("iced@b".to_string(), storage.clone()).unwrap();
    let fresh = Player::new_from_namerena_raw("fresh@b".to_string(), storage.clone()).unwrap();
    let plain = Player::new_from_namerena_raw("plain@b".to_string(), storage.clone()).unwrap();
    let caster_id = storage.just_insert_player(caster);
    let iced_id = storage.just_insert_player(iced);
    let fresh_id = storage.just_insert_player(fresh);
    let plain_id = storage.just_insert_player(plain);
    storage.sync_groups(&[vec![caster_id], vec![iced_id, fresh_id, plain_id]]);
    storage.sync_alive_groups(&[vec![caster_id], vec![iced_id, fresh_id, plain_id]]);

    {
        let iced_mut = storage.just_get_player_mut(iced_id).unwrap();
        iced_mut.set_state(crate::player::skill::act::ice::IceState {
            target: Some(iced_id),
            pre_step_impl: None,
            frozen_step: 1024,
        });
        iced_mut.status.hp = 100;
        iced_mut.status.max_hp = 100;
        iced_mut.status.atk_sum = 200;
        iced_mut.status.attract = 1.0;
        iced_mut.status.set_alive(true);
    }

    {
        let fresh_mut = storage.just_get_player_mut(fresh_id).unwrap();
        fresh_mut.status.hp = 100;
        fresh_mut.status.max_hp = 100;
        fresh_mut.status.atk_sum = 200;
        fresh_mut.status.attract = 1.0;
        fresh_mut.status.set_alive(true);
    }

    {
        let plain_mut = storage.just_get_player_mut(plain_id).unwrap();
        plain_mut.status.hp = 100;
        plain_mut.status.max_hp = 100;
        plain_mut.status.atk_sum = 150;
        plain_mut.status.attract = 1.0;
        plain_mut.status.set_alive(true);
    }

    let mut randomer = RC4::default();
    let mut updates = RunUpdates::new();
    let ice = crate::player::skill::act::ice::IceSkill::new();
    let iced_score = <crate::player::skill::act::ice::IceSkill as crate::player::skill::SkillTrait>::score_target(
        &ice,
        iced_id,
        true,
        (caster_id, &mut randomer, &mut updates, &storage),
    );
    let fresh_score = <crate::player::skill::act::ice::IceSkill as crate::player::skill::SkillTrait>::score_target(
        &ice,
        fresh_id,
        true,
        (caster_id, &mut randomer, &mut updates, &storage),
    );
    let plain_score = <crate::player::skill::act::ice::IceSkill as crate::player::skill::SkillTrait>::score_target(
        &ice,
        plain_id,
        true,
        (caster_id, &mut randomer, &mut updates, &storage),
    );

    assert!((fresh_score - iced_score * 2.0).abs() < 1e-9);
    assert!(plain_score > iced_score);
}

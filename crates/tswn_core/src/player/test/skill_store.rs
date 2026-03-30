use super::*;

#[test]
fn register_skill_proc_appends_late_post_damage_skill() {
    use crate::player::skill::{Skill, store::SkillStorage};

    let mut skills = SkillStorage::new();
    skills.add_skill(Skill::new_with_id(1, 21)); // Assassinate
    skills.add_skill(Skill::new_with_id(1, 30)); // Counter
    skills.add_skill(Skill::new_with_id(0, 33)); // Upgrade, gained later
    skills.add_skill(Skill::new_with_id(1, 34)); // Hide

    skills.update_proc();
    assert_eq!(skills.update_states, vec![3]);
    assert_eq!(skills.post_damage, vec![1, 3, 0]);

    skills.skill_by_id_mut(2).set_level(1);
    skills.register_skill_proc(2);

    assert_eq!(skills.update_states, vec![3, 2]);
    assert_eq!(skills.post_damage, vec![1, 3, 2, 0]);
}

#[test]
fn protect_state_runs_after_existing_pre_defend_skills() {
    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner".to_string(), storage.clone()).unwrap();
    let protector = Player::new_from_namerena_raw("protector@red".to_string(), storage.clone()).unwrap();
    let caster = Player::new_from_namerena_raw("caster@blue".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);
    let protector_id = storage.just_insert_player(protector);
    let caster_id = storage.just_insert_player(caster);
    storage.sync_groups(&[vec![owner_id, protector_id], vec![caster_id]]);
    let mut randomer = RC4::default();
    let mut probe = randomer.clone();
    let first = probe.next_u8();
    let mut updates = RunUpdates::new();

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.skills.add_skill(Skill::new(1, Box::new(ObservePreDefendByteSkill)));
        owner_mut.skills.update_proc();
        owner_mut.set_state(crate::player::skill::protect::ProtectState {
            target: Some(owner_id),
            protect_from: vec![crate::player::skill::protect::ProtectLink {
                owner: protector_id,
                level: 0,
            }],
            pre_defend_skill_count: owner_mut.skills.pre_defend.len(),
        });

        let atp = owner_mut.pre_defend(100.0, false, caster_id, noop_on_damage, &mut randomer, &mut updates, &storage);
        assert_eq!(atp, 100.0);
    }

    let observed: Vec<&str> = updates.updates.iter().map(|update| update.message.as_ref()).collect();
    assert_eq!(observed, vec![format!("pre_defend_skill_byte={first}")]);
}

#[test]
fn protect_state_runs_before_late_registered_pre_defend_skills() {
    let storage = Storage::new_arc();
    let owner = Player::new_from_namerena_raw("owner".to_string(), storage.clone()).unwrap();
    let protector = Player::new_from_namerena_raw("protector@red".to_string(), storage.clone()).unwrap();
    let caster = Player::new_from_namerena_raw("caster@blue".to_string(), storage.clone()).unwrap();
    let owner_id = storage.just_insert_player(owner);
    let protector_id = storage.just_insert_player(protector);
    let caster_id = storage.just_insert_player(caster);
    storage.sync_groups(&[vec![owner_id, protector_id], vec![caster_id]]);
    let mut randomer = RC4::default();
    let mut probe = randomer.clone();
    let _first = probe.next_u8();
    let second = probe.next_u8();
    let mut updates = RunUpdates::new();

    {
        let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
        owner_mut.skills.add_skill(Skill::new(1, Box::new(ObservePreDefendByteSkill)));
        owner_mut.skills.update_proc();
        owner_mut.set_state(crate::player::skill::protect::ProtectState {
            target: Some(owner_id),
            protect_from: vec![crate::player::skill::protect::ProtectLink {
                owner: protector_id,
                level: 0,
            }],
            pre_defend_skill_count: 0,
        });

        let atp = owner_mut.pre_defend(100.0, false, caster_id, noop_on_damage, &mut randomer, &mut updates, &storage);
        assert_eq!(atp, 100.0);
    }

    let observed: Vec<&str> = updates.updates.iter().map(|update| update.message.as_ref()).collect();
    assert_eq!(observed, vec![format!("pre_defend_skill_byte={second}")]);
}


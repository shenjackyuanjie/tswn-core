//! 验证第二批八个 Boss 的实际伤害、状态推进、召唤归属和回放复用。
use super::super::*;
fn battle(name: &str) -> (RuntimeRunner, EntityIdx, EntityIdx) {
    let c = default_custom_runtime_import_config().unwrap();
    let r = RuntimeRunner::from_custom_mixed_namerena_raw(format!("{name}@!\n\nalpha\n\nseed:2"), c).unwrap();
    let b = r.runtime().entities.iter().find(|(_, e)| e.template.name == name).unwrap().0;
    let t = r.runtime().entities.iter().find(|(_, e)| e.template.name == "alpha").unwrap().0;
    (r, b, t)
}
fn durable(rt: &mut CombatRuntime, id: EntityIdx) {
    let e = rt.entities.get_mut(id).unwrap();
    e.template.max_hp = 1000000;
    e.runtime.hp = 1000000;
    e.runtime.agility = 0;
}
fn hit(rt: &mut CombatRuntime, a: EntityIdx, b: EntityIdx, n: i32, k: DamageKind, u: &mut RunUpdates) -> i32 {
    let defense = rt.entities.get(b).unwrap().runtime.defense + 64;
    rt.drain_plain_attack_after_dodge_into(a, b, false, f64::from(n * defense), k, None, PlainAttackOnDamage::None, u)
}
#[test]
fn flandre_splits_into_three_owned_clones_and_executes_low_hp() {
    let (mut r, b, t) = battle("flandre");
    let rt = r.runtime_mut();
    let mut u = RunUpdates::new();
    let max = rt.entities.get(b).unwrap().template.max_hp;
    rt.ros_cast(b, 0, vec![b], &mut u);
    let children: Vec<_> = rt
        .entities
        .iter()
        .filter(|(_, e)| e.runtime.owner == b && e.runtime.rosmontic.as_ref().is_some_and(|s| s.child_kind == 4))
        .map(|(id, _)| id)
        .collect();
    assert_eq!(children.len(), 3);
    assert_eq!(rt.entities.get(b).unwrap().template.max_hp, (f64::from(max) * 0.6) as i32);
    for id in children {
        assert_eq!(rt.entities.get(id).unwrap().runtime.hp, (f64::from(max) * 0.4) as i32);
    }
    assert_eq!(rt.ros_state(b).levels[0], 0);
    rt.entities.get_mut(t).unwrap().runtime.hp = 1;
    rt.pcb_destroy(b, t, &mut u);
    assert!(!rt.entities.get(t).unwrap().runtime.alive);
}
#[test]
fn flandre_charged_sword_releases_on_next_action() {
    let (mut r, b, t) = battle("flandre");
    let rt = r.runtime_mut();
    let mut u = RunUpdates::new();
    durable(rt, t);
    rt.ros_cast(b, 3, vec![t], &mut u);
    assert_eq!(rt.ros_state(b).charged_skill, Some(3));
    rt.drain_touhou_action_into(b, &mut u);
    assert_eq!(rt.ros_state(b).charged_skill, None);
    assert_eq!(rt.ros_state(b).energy, 0);
    assert!(u.updates.iter().any(|x| x.message.contains("莱瓦汀")));
}
#[test]
fn letty_blocks_ice_but_not_normal_and_winter_is_once() {
    let (mut r, b, t) = battle("letty");
    let rt = r.runtime_mut();
    let mut u = RunUpdates::new();
    let hp = rt.entities.get(b).unwrap().runtime.hp;
    assert_eq!(hit(rt, t, b, 100, DamageKind::Ice, &mut u), 0);
    assert_eq!(rt.entities.get(b).unwrap().runtime.hp, hp);
    assert!(hit(rt, t, b, 100, DamageKind::Default, &mut u) > 0);
    rt.entities.get_mut(b).unwrap().runtime.hp = 300;
    assert!(rt.ros_skill_allowed(b, 3));
    durable(rt, t);
    rt.ros_cast(b, 3, vec![t], &mut u);
    assert_eq!(rt.ros_state(b).levels[3], 0);
    assert!(rt.entities.get(b).unwrap().runtime.hp <= 210);
}
#[test]
fn letty_generates_missing_hp_shield_and_long_ice() {
    let (mut r, b, t) = battle("letty");
    let rt = r.runtime_mut();
    let mut u = RunUpdates::new();
    rt.entities.get_mut(b).unwrap().runtime.hp = 370;
    for _ in 0..100 {
        rt.pcb_letty_shield(b, &mut u);
        if rt.entities.get(b).unwrap().runtime.shield > 0 {
            break;
        }
    }
    assert_eq!(rt.entities.get(b).unwrap().runtime.shield, 70);
    rt.ros_state(b).frost_mode = 13;
    rt.apply_ice_on_damage(b, t, &mut u);
    assert!(rt.entities.get(t).unwrap().states.is_frozen());
}
#[test]
fn chen_cat_switches_stats_and_limits_spells_and_reverts() {
    let (mut r, b, _) = battle("chen");
    let rt = r.runtime_mut();
    let mut u = RunUpdates::new();
    let base = rt.entities.get(b).unwrap().runtime.speed;
    for _ in 0..1000 {
        rt.begin_rosmontic_action(b, &mut u);
        if rt.ros_state(b).cat {
            break;
        }
    }
    assert!(rt.ros_state(b).cat);
    assert_eq!(rt.entities.get(b).unwrap().runtime.speed, base * 2);
    assert!(!rt.ros_skill_allowed(b, 2));
    assert!(!rt.ros_skill_allowed(b, 4));
    for _ in 0..1000 {
        rt.begin_rosmontic_action(b, &mut u);
        if !rt.ros_state(b).cat {
            break;
        }
    }
    assert!(!rt.ros_state(b).cat);
    assert_eq!(rt.entities.get(b).unwrap().runtime.speed, base);
    rt.ros_cast(b, 1, vec![b], &mut u);
    rt.entities.get_mut(b).unwrap().refresh_runtime_stats_from_template();
    assert_eq!(rt.entities.get(b).unwrap().runtime.speed, (f64::from(base) * 1.3) as i32);
}
#[test]
fn alice_all_dolls_have_stats_can_act_and_can_be_resummoned() {
    let (mut r, b, t) = battle("alice");
    let rt = r.runtime_mut();
    let mut u = RunUpdates::new();
    durable(rt, t);
    for skill in [0, 3, 4, 5, 6, 7, 8, 9, 10] {
        assert!(rt.ros_skill_allowed(b, skill));
        rt.ros_cast(b, skill, vec![b], &mut u);
        assert!(!rt.ros_skill_allowed(b, skill));
    }
    let dolls = rt.pcb_dolls(b);
    assert_eq!(dolls.len(), 10);
    for id in dolls {
        let role = rt.ros_state(id).child_kind;
        assert_eq!(rt.entities.get(id).unwrap().runtime.owner, b);
        rt.pcb_child_action(id, role, &mut u);
    }
    let shanghai = rt
        .pcb_dolls(b)
        .into_iter()
        .find(|id| rt.entities.get(*id).unwrap().runtime.rosmontic.as_ref().unwrap().child_kind == 10)
        .unwrap();
    assert_eq!(rt.entities.get(shanghai).unwrap().template.max_hp, 200);
    rt.entities.get_mut(shanghai).unwrap().runtime.hp = 0;
    rt.drain_summon_explode_self_death_into(shanghai, &mut u);
    assert!(rt.ros_skill_allowed(b, 3));
    rt.ros_cast(b, 3, vec![b], &mut u);
    assert_eq!(rt.pcb_dolls(b).len(), 10);
    assert!(u.updates.iter().any(|x| x.message.contains("激光")));
}
#[test]
fn alice_stage_hits_and_explosion_removes_all_dolls() {
    let (mut r, b, t) = battle("alice");
    let rt = r.runtime_mut();
    let mut u = RunUpdates::new();
    durable(rt, t);
    for skill in [3, 4, 5, 6] {
        rt.ros_cast(b, skill, vec![b], &mut u);
    }
    assert!(rt.ros_skill_allowed(b, 2));
    rt.ros_cast(b, 2, vec![t], &mut u);
    assert!(rt.entities.get(t).unwrap().runtime.hp < 1000000);
    rt.entities.get_mut(b).unwrap().runtime.hp = 300;
    assert!(rt.ros_skill_allowed(b, 1));
    let resistance = rt.entities.get(t).unwrap().runtime.resistance;
    rt.ros_cast(b, 1, vec![t], &mut u);
    assert!(rt.pcb_dolls(b).is_empty());
    assert_eq!(rt.entities.get(t).unwrap().runtime.resistance, resistance);
}
#[test]
fn phantom_loses_members_in_stages_and_reduces_damage() {
    let (mut r, b, t) = battle("phantom");
    let rt = r.runtime_mut();
    let mut u = RunUpdates::new();
    assert_eq!(hit(rt, t, b, 100, DamageKind::Default, &mut u), 55);
    rt.entities.get_mut(b).unwrap().runtime.hp = 900;
    rt.begin_rosmontic_action(b, &mut u);
    assert_eq!(rt.ros_state(b).phantom_members.count_ones(), 2);
    assert!(!rt.ros_skill_allowed(b, 0));
    assert_eq!(hit(rt, t, b, 100, DamageKind::Default, &mut u), 70);
    rt.entities.get_mut(b).unwrap().runtime.hp = 400;
    rt.begin_rosmontic_action(b, &mut u);
    assert_eq!(rt.ros_state(b).phantom_members.count_ones(), 1);
    assert_eq!(hit(rt, t, b, 100, DamageKind::Default, &mut u), 85);
}
#[test]
fn phantom_concerto_heals_shields_and_accelerates_friend() {
    let (mut r, b, _) = battle("phantom");
    let rt = r.runtime_mut();
    let mut u = RunUpdates::new();
    rt.entities.get_mut(b).unwrap().runtime.hp = 500;
    let points = rt.entities.get(b).unwrap().runtime.move_state.speed_points;
    rt.ros_cast(b, 0, vec![b], &mut u);
    let e = rt.entities.get(b).unwrap();
    assert!(e.runtime.hp > 500);
    assert_eq!(e.runtime.shield, 80);
    assert_eq!(e.runtime.move_state.speed_points, points + 1024);
}
#[test]
fn youmu_guard_counters_once_and_expires_when_unused() {
    let (mut r, b, t) = battle("youmu");
    let rt = r.runtime_mut();
    let mut u = RunUpdates::new();
    durable(rt, t);
    let hp = rt.entities.get(b).unwrap().runtime.hp;
    rt.ros_cast(b, 0, vec![b], &mut u);
    assert_eq!(hit(rt, t, b, 100, DamageKind::Default, &mut u), 0);
    assert_eq!(rt.entities.get(b).unwrap().runtime.hp, hp);
    assert!(rt.entities.get(t).unwrap().runtime.hp < 1000000);
    assert_eq!(rt.ros_state(b).guard, 2);
    assert!(!rt.ros_skill_allowed(b, 0));
    assert_eq!(hit(rt, t, b, 100, DamageKind::Default, &mut u), 100);
    rt.ros_state(b).guard = 1;
    rt.begin_rosmontic_action(b, &mut u);
    assert_eq!(rt.ros_state(b).guard, 0);
}
#[test]
fn yuyuko_high_hp_charge_interrupts_low_hp_charge_does_not() {
    let (mut r, b, t) = battle("yuyuko");
    let rt = r.runtime_mut();
    let mut u = RunUpdates::new();
    durable(rt, t);
    rt.ros_cast(b, 0, vec![t], &mut u);
    hit(rt, t, b, 1, DamageKind::Default, &mut u);
    assert_eq!(rt.ros_state(b).charged_skill, None);
    assert_eq!(rt.ros_state(b).energy, 0);
    rt.entities.get_mut(b).unwrap().runtime.hp = 200;
    rt.ros_cast(b, 0, vec![t], &mut u);
    hit(rt, t, b, 1, DamageKind::Default, &mut u);
    assert_eq!(rt.ros_state(b).charged_skill, Some(0));
    rt.drain_touhou_action_into(b, &mut u);
    assert_eq!(rt.ros_state(b).energy, 0);
    assert!(rt.entities.get(t).unwrap().runtime.hp < 1000000);
}
#[test]
fn yuyuko_prepares_three_turns_revives_once_and_immunity_expires() {
    let (mut r, b, t) = battle("yuyuko");
    let rt = r.runtime_mut();
    let mut u = RunUpdates::new();
    rt.ros_cast(b, 1, vec![b], &mut u);
    for _ in 0..4 {
        rt.begin_rosmontic_action(b, &mut u);
    }
    assert_eq!(rt.ros_state(b).butterfly_turns, 3);
    hit(rt, t, b, 10000, DamageKind::Default, &mut u);
    assert_eq!(rt.entities.get(b).unwrap().runtime.hp, 10);
    assert!(rt.entities.get(b).unwrap().runtime.alive);
    assert_eq!(hit(rt, t, b, 10000, DamageKind::Default, &mut u), 0);
    for _ in 0..4 {
        rt.begin_rosmontic_action(b, &mut u);
    }
    assert_eq!(rt.ros_state(b).butterfly_turns, -1);
    hit(rt, t, b, 10000, DamageKind::Default, &mut u);
    assert!(!rt.entities.get(b).unwrap().runtime.alive);
}
#[test]
fn ran_shikigami_return_stats_once_and_illusions_grow() {
    let (mut r, b, _) = battle("ran");
    let rt = r.runtime_mut();
    let mut u = RunUpdates::new();
    rt.ros_cast(b, 0, vec![b], &mut u);
    rt.ros_cast(b, 1, vec![b], &mut u);
    let children: Vec<_> = rt
        .entities
        .iter()
        .filter(|(_, e)| e.runtime.owner == b && e.runtime.rosmontic.as_ref().is_some_and(|s| (20..=22).contains(&s.child_kind)))
        .map(|(id, _)| id)
        .collect();
    assert_eq!(children.len(), 3);
    let attack = rt.entities.get(b).unwrap().runtime.attack;
    let speed = rt.entities.get(b).unwrap().runtime.speed;
    for id in children {
        rt.entities.get_mut(id).unwrap().runtime.hp = 0;
        rt.drain_summon_explode_self_death_into(id, &mut u);
        rt.pcb_child_died(id);
    }
    assert_eq!(rt.entities.get(b).unwrap().runtime.attack, attack + 20);
    assert_eq!(rt.entities.get(b).unwrap().runtime.speed, speed + 12);
    rt.ros_cast(b, 3, vec![b], &mut u);
    rt.ros_cast(b, 3, vec![b], &mut u);
    let hp: Vec<_> = rt
        .entities
        .iter()
        .filter(|(_, e)| e.runtime.rosmontic.as_ref().is_some_and(|s| s.child_kind == 23))
        .map(|(_, e)| e.runtime.hp)
        .collect();
    assert_eq!(hp, vec![100, 112]);
}
#[test]
fn ran_divine_doubles_six_stats_for_three_actions_and_is_dispellable() {
    let (mut r, b, _) = battle("ran");
    let rt = r.runtime_mut();
    let mut u = RunUpdates::new();
    let base = rt.entities.get(b).unwrap().runtime.attack;
    let speed = rt.entities.get(b).unwrap().effective_speed();
    let wis = rt.entities.get(b).unwrap().runtime.wisdom;
    rt.ros_cast(b, 2, vec![b], &mut u);
    assert_eq!(rt.entities.get(b).unwrap().runtime.attack, base * 2);
    assert_eq!(rt.entities.get(b).unwrap().effective_speed(), speed * 2);
    assert_eq!(rt.entities.get(b).unwrap().runtime.wisdom, wis);
    assert!(!rt.ros_skill_allowed(b, 2));
    for _ in 0..2 {
        rt.end_rosmontic_action(b, &mut u);
        assert_eq!(rt.entities.get(b).unwrap().runtime.attack, base * 2);
    }
    rt.end_rosmontic_action(b, &mut u);
    assert_eq!(rt.entities.get(b).unwrap().runtime.attack, base);
    rt.ros_cast(b, 2, vec![b], &mut u);
    rt.dispel_rosmontic_into(b, &mut u);
    assert_eq!(rt.ros_state(b).divine_turns, 0);
    assert_eq!(rt.entities.get(b).unwrap().runtime.attack, base);
}
#[test]
fn eight_bosses_finish_and_capture_matches_fast_and_clone_reset() {
    for name in ["flandre", "letty", "chen", "alice", "phantom", "youmu", "yuyuko", "ran"] {
        let (mut r, b, _) = battle(name);
        let initial = r.runtime().entities.get(b).unwrap().runtime.clone();
        let mut copy = r.clone();
        let full = r.run_until_winner(20000);
        let fast = copy.run_to_completion(20000);
        assert!(!full.guard_exhausted, "{name}");
        assert_eq!(full.winner_team, fast.winner_team, "{name}");
        assert_eq!(full.rounds.len(), fast.rounds, "{name}");
        assert_eq!(r.runtime().entities, copy.runtime().entities, "{name}");
        r.runtime_mut().entities.get_mut(b).unwrap().runtime.reset_battle_state_from(&initial);
        assert_eq!(r.runtime().entities.get(b).unwrap().runtime.rosmontic, initial.rosmontic);
    }
}
#[test]
fn yuyuko_long_preparation_preserves_a_live_entity_and_midbattle_clone() {
    let (mut runner, boss, _) = battle("yuyuko");
    let mut updates = RunUpdates::new();
    runner.runtime_mut().ros_cast(boss, 1, vec![boss], &mut updates);
    for _ in 0..100 {
        runner.runtime_mut().begin_rosmontic_action(boss, &mut updates);
    }
    assert_eq!(runner.runtime().entities.get(boss).unwrap().runtime.hp, 1);
    assert_eq!(runner.runtime().entities.get(boss).unwrap().template.max_hp, 1);
    let mut copy = runner.clone();
    let full = runner.run_until_winner(20000);
    let fast = copy.run_to_completion(20000);
    assert!(!full.guard_exhausted);
    assert_eq!(full.winner_team, fast.winner_team);
    assert_eq!(runner.runtime().entities, copy.runtime().entities);
}
#[test]
fn squall_dialogue_is_hidden_but_skill_and_damage_remain() {
    let (mut r, b, t) = battle("Squall");
    let rt = r.runtime_mut();
    let mut u = RunUpdates::new();
    durable(rt, t);
    rt.entities.get_mut(b).unwrap().runtime.squall.as_mut().unwrap().samples = 6;
    rt.squall_weighted(b, &mut u);
    assert!(u.updates.iter().any(|x| x.message.contains("建立模型")));
    assert!(u.updates.iter().any(|x| x.message.contains("伤害")));
    assert!(!u.updates.iter().any(|x| x.dialogue));
}

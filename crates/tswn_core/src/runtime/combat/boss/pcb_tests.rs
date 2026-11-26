//! 验证第三批八个 Boss 的实际伤害、状态推进、召唤归属和回放复用。
//! 来源：KODU16（`kodu16/main`，`cf460c84`）。
use super::super::*;
use crate::namerena::eval_name::WIN_RATE_EVAL_RQ;

fn give_next_action(runner: &mut RuntimeRunner, actor: EntityIdx) {
    let rt = runner.runtime_mut();
    for i in (0..rt.entities.len()).map(|i| EntityIdx(i as u32)) {
        if let Some(e) = rt.entities.get_mut(i) {
            e.runtime.move_state.speed_points = if actor == i {
                MOVE_POINT_THRESHOLD + 1
            } else {
                -100 * MOVE_POINT_THRESHOLD
            };
        }
    }
}

fn only_skill(rt: &mut CombatRuntime, actor: EntityIdx, skill: Option<usize>) {
    let levels = &mut rt.ros_state(actor).levels;
    levels.fill(0);
    if let Some(skill) = skill {
        levels[skill] = 128;
    }
    rt.entities.get_mut(actor).unwrap().runtime.magic_point = 100;
}

fn act(runner: &mut RuntimeRunner, actor: EntityIdx) -> RunUpdates {
    give_next_action(runner, actor);
    let outcome = runner.run_round();
    assert_eq!(outcome.action.unwrap().actor, actor);
    outcome.frame.unwrap().updates
}

fn find_entity(rt: &CombatRuntime, name: &str) -> EntityIdx {
    rt.entities.iter().find(|(_, e)| e.template.name == name).unwrap().0
}

/// 只选定测试分支用到的随机字节；不重复执行被测行为。
fn select_rng(rt: &mut CombatRuntime, rule: impl Fn(&mut RC4) -> bool) {
    for _ in 0..128 {
        if rule(&mut rt.rng.clone()) {
            return;
        }
        rt.rng.next_u8();
    }
    panic!("固定 seed 的随机窗口不满足测试前置条件");
}
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
    rt.drain_touhou_action_into(b, true, &mut u);
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
    assert!(rt.ros_skill_allowed(b, 3, true));
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
    select_rng(rt, |rng| rng.next_u8() < 128);
    rt.drain_plain_attack_with_atp_and_on_damage_into(t, b, false, 1.0, DamageKind::Ice, PlainAttackOnDamage::Ice, &mut u);
    assert_eq!(rt.entities.get(b).unwrap().runtime.shield, 70);
    assert_eq!(rt.entities.get(b).unwrap().states.ice_frozen_step(PLAIN_ICE_STATE_KEY), None);

    rt.apply_ice_on_damage(b, t, &mut u);
    assert_eq!(
        rt.entities.get(t).unwrap().states.ice_frozen_step(PLAIN_ICE_STATE_KEY),
        Some(1024)
    );
    let e = rt.entities.get_mut(t).unwrap();
    e.runtime.hp = 100_000;
    e.template.max_hp = 100_000;
    rt.ros_cast(b, 3, vec![t], &mut u);
    assert_eq!(
        rt.entities.get(t).unwrap().states.ice_frozen_step(PLAIN_ICE_STATE_KEY),
        Some(15 * 1024)
    );
    rt.entities.get_mut(b).unwrap().runtime.charge.active = true;
    rt.apply_ice_on_damage(b, t, &mut u);
    assert_eq!(
        rt.entities.get(t).unwrap().states.ice_frozen_step(PLAIN_ICE_STATE_KEY),
        Some(31 * 1024)
    );
    rt.apply_ice_on_damage(t, b, &mut u);
    assert_eq!(
        rt.entities.get(b).unwrap().states.ice_frozen_step(PLAIN_ICE_STATE_KEY),
        Some(1024)
    );
}
#[test]
fn chen_cat_switches_stats_and_limits_spells_and_reverts() {
    let (mut r, b, _) = battle("chen");
    let rt = r.runtime_mut();
    let mut u = RunUpdates::new();
    let base = rt.entities.get(b).unwrap().runtime.speed;
    let agility = rt.entities.get(b).unwrap().runtime.agility;
    let same_rng = rt.rng.clone();
    let base_atp = rt.entities.get(b).unwrap().runtime.get_at(false, &mut same_rng.clone());
    select_rng(rt, |rng| rng.next_u8() & 15 == 0);
    rt.begin_rosmontic_action(b, &mut u);
    assert!(rt.ros_state(b).cat);
    let e = rt.entities.get_mut(b).unwrap();
    e.apply_runtime_stats_from_template(false);
    assert_eq!(e.runtime.agility, agility * 2);
    assert_eq!(e.runtime.at_boost_millionths, 2_000_000);
    assert_eq!(e.runtime.get_at(false, &mut same_rng.clone()), base_atp * 2.0);
    assert_eq!(rt.entities.get(b).unwrap().runtime.speed, base * 2);
    assert!(!rt.ros_skill_allowed(b, 2, true));
    assert!(!rt.ros_skill_allowed(b, 4, true));
    select_rng(rt, |rng| {
        rng.next_u8();
        rng.next_u8() & 7 == 0
    });
    rt.begin_rosmontic_action(b, &mut u);
    assert!(!rt.ros_state(b).cat);
    let e = rt.entities.get(b).unwrap();
    assert_eq!(e.runtime.get_at(false, &mut same_rng.clone()), base_atp);
    assert_eq!(e.runtime.at_boost_millionths, 1_000_000);
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
        assert!(rt.ros_skill_allowed(b, skill, true));
        rt.ros_cast(b, skill, vec![b], &mut u);
        assert!(!rt.ros_skill_allowed(b, skill, true));
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
    assert!(rt.ros_skill_allowed(b, 3, true));
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
    assert!(rt.ros_skill_allowed(b, 2, true));
    rt.ros_cast(b, 2, vec![t], &mut u);
    assert!(rt.entities.get(t).unwrap().runtime.hp < 1000000);
    rt.entities.get_mut(b).unwrap().runtime.hp = 300;
    assert!(rt.ros_skill_allowed(b, 1, true));
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
    assert!(!rt.ros_skill_allowed(b, 0, true));
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
    assert!(!rt.ros_skill_allowed(b, 0, true));
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
    rt.drain_touhou_action_into(b, true, &mut u);
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
    let (mut r, b, enemy) = battle("ran");
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
    let base = rt.entities.get(b).unwrap().runtime.clone();
    for (index, &id) in children.iter().enumerate() {
        if index == 0 {
            hit(rt, enemy, id, 10_000, DamageKind::Default, &mut u);
        } else {
            rt.entities.get_mut(id).unwrap().runtime.hp = 0;
            rt.drain_summon_explode_self_death_into(id, &mut u);
        }
        assert!(rt.ros_state(id).child_death_notified);
        assert!(!rt.world.flat_alive().contains(&id));
        assert!(!rt.world.round_order().contains(&id));
        let notified = rt.entities.get(b).unwrap().runtime.clone();
        rt.drain_die_hooks_into(id, &mut u);
        assert_eq!(rt.entities.get(b).unwrap().runtime, notified);
    }
    let owner = rt.entities.get_mut(b).unwrap();
    owner.apply_runtime_stats_from_template(false);
    assert_eq!(owner.runtime.attack, base.attack + 20);
    assert_eq!(owner.runtime.defense, base.defense + 10);
    assert_eq!(owner.runtime.resistance, base.resistance + 10);
    assert_eq!(owner.runtime.speed, base.speed + 12);
    assert_eq!(owner.runtime.agility, base.agility + 12);
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
    assert!(!rt.ros_skill_allowed(b, 2, true));
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
fn eight_bosses_finish_and_capture_matches_fast() {
    for name in ["flandre", "letty", "chen", "alice", "phantom", "youmu", "yuyuko", "ran"] {
        let (mut r, _, _) = battle(name);
        let mut copy = r.clone();
        let full = r.run_until_winner(20000);
        let fast = copy.run_to_completion(20000);
        assert!(!full.guard_exhausted, "{name}");
        assert_eq!(full.winner_team, fast.winner_team, "{name}");
        assert_eq!(full.rounds.len(), fast.rounds, "{name}");
        assert_eq!(r.runtime().entities, copy.runtime().entities, "{name}");
        assert_eq!(r.runtime().world, copy.runtime().world, "{name}");
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
fn letty_spells_use_ice_kind_but_table_turn_is_not_ice() {
    let config = default_custom_runtime_import_config().unwrap();
    let mut runner = RuntimeRunner::from_custom_mixed_namerena_raw("letty@!\n\ncirno@!\n\nseed:2".into(), config).unwrap();
    let rt = runner.runtime_mut();
    let letty = find_entity(rt, "letty");
    let cirno = find_entity(rt, "cirno");
    durable(rt, cirno);
    let mut updates = RunUpdates::new();
    let hp = rt.entities.get(cirno).unwrap().runtime.hp;
    rt.ros_cast(letty, 4, vec![cirno], &mut updates);
    assert!(updates.updates.iter().any(|u| u.message.contains("[免疫]冰冻伤害")));
    assert_eq!(rt.entities.get(cirno).unwrap().runtime.hp, hp);
    assert_eq!(
        rt.entities.get(cirno).unwrap().states.ice_frozen_step(PLAIN_ICE_STATE_KEY),
        None
    );
    rt.ros_cast(letty, 5, vec![cirno], &mut updates);
    assert!(rt.entities.get(cirno).unwrap().runtime.hp < hp);
}

#[test]
fn two_allied_alices_can_summon_the_same_kind_independently() {
    let config = default_custom_runtime_import_config().unwrap();
    let mut runner = RuntimeRunner::from_custom_mixed_namerena_raw("alice@!\nalice@!\n\nalpha\n\nseed:2".into(), config).unwrap();
    let rt = runner.runtime_mut();
    let alices: Vec<_> = rt
        .entities
        .iter()
        .filter_map(|(id, e)| (e.template.name == "alice").then_some(id))
        .collect();
    let [a, b] = alices[..] else {
        panic!("需要两个爱丽丝实体")
    };
    assert_eq!(
        rt.entities.get(a).unwrap().runtime.team,
        rt.entities.get(b).unwrap().runtime.team
    );
    only_skill(rt, a, Some(3));
    only_skill(rt, b, Some(3));
    act(&mut runner, a);
    let rt = runner.runtime_mut();
    assert!(!rt.ros_skill_allowed(a, 3, true));
    assert!(rt.ros_skill_allowed(b, 3, true));
    act(&mut runner, a);
    act(&mut runner, b);
    let rt = runner.runtime();
    let left = rt.pcb_dolls(a);
    let right = rt.pcb_dolls(b);
    assert_eq!(left.len(), 1);
    assert_eq!(right.len(), 1);
    assert_ne!(left, right);
    for (owner, doll) in [(a, left[0]), (b, right[0])] {
        let e = rt.entities.get(doll).unwrap();
        assert_eq!(e.runtime.owner, owner);
        assert_eq!(e.runtime.root_owner, owner);
        assert_eq!(e.runtime.rosmontic.as_ref().unwrap().child_kind, 10);
        assert!(rt.world.team_roster(e.runtime.team).unwrap().contains(&doll));
        assert!(rt.world.team_alive(e.runtime.team).unwrap().contains(&doll));
    }
}

#[test]
fn youmu_counter_cannot_consume_other_youmus_guard() {
    let config = default_custom_runtime_import_config().unwrap();
    let mut runner = RuntimeRunner::from_custom_mixed_namerena_raw("youmu@!\n\nyoumu@!\n\nseed:2".into(), config).unwrap();
    let rt = runner.runtime_mut();
    let actors: Vec<_> = rt.entities.iter().map(|(id, _)| id).collect();
    let [attacker, defender] = actors[..] else {
        panic!("需要两个妖梦实体")
    };
    assert_ne!(rt.plain_effective_team(attacker), rt.plain_effective_team(defender));
    let mut updates = RunUpdates::new();
    for id in [attacker, defender] {
        durable(rt, id);
        rt.ros_cast(id, 0, vec![id], &mut updates);
    }
    let inbound_points = rt.entities.get(defender).unwrap().runtime.move_state.speed_points;
    assert_eq!(hit(rt, attacker, defender, 100, DamageKind::Default, &mut updates), 0);
    assert_eq!(rt.ros_state(defender).guard, 2);
    assert_eq!(rt.ros_state(attacker).guard, 1);
    assert_eq!(
        rt.entities.get(defender).unwrap().runtime.move_state.speed_points,
        inbound_points + 2048
    );
    assert_eq!(rt.entities.get(defender).unwrap().runtime.hp, 1_000_000);
    assert!(rt.entities.get(attacker).unwrap().runtime.hp < 1_000_000);
    assert_eq!(updates.updates.iter().filter(|u| u.message.contains("发动[空观剑")).count(), 1);
}

#[test]
fn ran_owner_death_removes_shikigami_without_returning_stats() {
    let (mut runner, ran, enemy) = battle("ran");
    let rt = runner.runtime_mut();
    let mut updates = RunUpdates::new();
    rt.ros_cast(ran, 0, vec![ran], &mut updates);
    let children: Vec<_> = rt
        .entities
        .iter()
        .filter_map(|(id, e)| (id != ran && e.runtime.owner == ran).then_some(id))
        .collect();
    assert_eq!(children.len(), 2);
    hit(rt, enemy, ran, 10_000, DamageKind::Default, &mut updates);
    assert!(!rt.entities.get(ran).unwrap().runtime.alive);
    for id in children {
        assert!(!rt.entities.get(id).unwrap().runtime.alive);
        assert!(!rt.world.flat_alive().contains(&id));
        assert!(!rt.ros_state(id).child_death_notified);
    }
    assert_eq!(rt.ros_state(ran).stat_add, [0; 7]);
}

#[test]
fn flandre_split_at_one_hp_keeps_live_owner_and_clones() {
    let (mut runner, flandre, _) = battle("flandre");
    let rt = runner.runtime_mut();
    let e = rt.entities.get_mut(flandre).unwrap();
    e.runtime.hp = 1;
    e.runtime.wisdom = 0;
    only_skill(rt, flandre, Some(0));
    assert!(!rt.ros_skill_allowed(flandre, 0, true));
    let updates = act(&mut runner, flandre);
    assert!(updates.updates.iter().any(|u| u.message.contains("四重存在")));
    let rt = runner.runtime();
    let clones: Vec<_> = rt
        .entities
        .iter()
        .filter(|(id, e)| *id != flandre && e.runtime.owner == flandre)
        .collect();
    assert_eq!(clones.len(), 3);
    for (id, entity) in clones.into_iter().chain([(flandre, rt.entities.get(flandre).unwrap())]) {
        assert_eq!(entity.runtime.hp, 1);
        assert!(entity.runtime.alive);
        assert!(rt.world.flat_alive().contains(&id));
    }
}

fn assert_prepared_reset(prepared: &PreparedRuntimeRunner, runner: &mut RuntimeRunner, boss: EntityIdx, children: &[EntityIdx]) {
    // 第二次复位复用第一次交换回来的世界缓冲，必须再次没有旧子体索引。
    for key in ["seed:3", "seed:5"] {
        let seed = [key.to_owned()];
        let mut fresh = prepared.new_with_seed(&seed).unwrap();
        prepared.reset_with_seed(runner, &seed).unwrap();
        let actual = runner.runtime();
        let expected = fresh.runtime();
        assert_eq!(
            actual.entities.get(boss).unwrap().template,
            expected.entities.get(boss).unwrap().template
        );
        assert_eq!(actual.entities, expected.entities);
        assert_eq!(actual.world, expected.world);
        assert_eq!(actual.world.roster_entity_count(), expected.world.roster_entity_count());
        assert_eq!(actual.round, 0);
        assert_eq!(actual.world.winner_team(), None);
        assert_eq!(runner.input_groups(), fresh.input_groups());
        for &child in children {
            assert!(actual.entities.get(child).is_none());
            assert!(!actual.world.round_order().contains(&child));
            assert!(!actual.world.flat_alive().contains(&child));
            assert_eq!(actual.world.alive_group_len_containing(child), 0);
            for team in 0..runner.input_groups().len() {
                assert!(!actual.world.team_roster(team).unwrap().contains(&child));
                assert!(!actual.world.team_alive(team).unwrap().contains(&child));
            }
        }
        for (id, _) in actual.entities.iter() {
            assert_eq!(
                actual.world.alive_group_len_containing(id),
                expected.world.alive_group_len_containing(id)
            );
        }
        let reused = runner.run_to_completion(20000);
        let clean = fresh.run_to_completion(20000);
        assert!(!reused.guard_exhausted && !clean.guard_exhausted);
        assert_eq!((reused.winner_team, reused.rounds), (clean.winner_team, clean.rounds));
        assert_eq!(runner.runtime().entities, fresh.runtime().entities);
        assert_eq!(runner.runtime().world, fresh.runtime().world);
    }
}

fn shrink_and_reset_prepared_battle(name: &str) {
    let config = default_custom_runtime_import_config().unwrap();
    let groups = vec![vec![format!("{name}@!"), "alice@!".into()], vec!["alpha".into(), "beta".into()]];
    let prepared = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(&groups, WIN_RATE_EVAL_RQ, config).unwrap();
    let mut runner = prepared.new_with_seed(&["seed:2".into()]).unwrap();
    let boss = find_entity(runner.runtime(), name);
    let alice = find_entity(runner.runtime(), "alice");
    let alpha = find_entity(runner.runtime(), "alpha");
    let initial = runner.runtime().entities.get(boss).unwrap().template.max_hp;
    let expected = match name {
        "phantom" => {
            let rt = runner.runtime_mut();
            rt.entities.get_mut(boss).unwrap().runtime.hp = initial / 2;
            only_skill(rt, boss, None);
            let updates = act(&mut runner, boss);
            assert!(updates.updates.iter().any(|u| u.message.contains("失去了")));
            (f64::from(initial) * 0.66) as i32
        }
        "yuyuko" => {
            only_skill(runner.runtime_mut(), boss, Some(1));
            let updates = act(&mut runner, boss);
            assert!(updates.updates.iter().any(|u| u.message.contains("埋骨于弘川")));
            only_skill(runner.runtime_mut(), boss, None);
            let updates = act(&mut runner, boss);
            assert!(updates.updates.iter().any(|u| u.message.contains("生命上限减少")));
            (f64::from(initial) * 0.8) as i32
        }
        "flandre" => {
            only_skill(runner.runtime_mut(), boss, Some(0));
            let updates = act(&mut runner, boss);
            assert!(updates.updates.iter().any(|u| u.message.contains("四重存在")));
            (f64::from(initial) * 0.6) as i32
        }
        _ => unreachable!(),
    };
    assert_ne!(expected, initial);
    assert_eq!(runner.runtime().entities.get(boss).unwrap().template.max_hp, expected);
    for skill in [3, 4] {
        only_skill(runner.runtime_mut(), alice, Some(skill));
        act(&mut runner, alice);
    }
    let rt = runner.runtime_mut();
    let dolls = rt.pcb_dolls(alice);
    let [dead, living] = dolls[..] else {
        panic!("需要一活一死两个人偶")
    };
    let team = rt.entities.get(alice).unwrap().runtime.team;
    hit(rt, alpha, dead, 10_000, DamageKind::Default, &mut RunUpdates::new());
    assert!(!rt.entities.get(dead).unwrap().runtime.alive);
    assert!(!rt.world.round_order().contains(&dead));
    assert!(!rt.world.team_alive(team).unwrap().contains(&dead));
    assert!(rt.world.team_roster(team).unwrap().contains(&dead));
    assert!(rt.world.round_order().contains(&living));
    assert!(rt.world.team_roster(team).unwrap().contains(&living));
    assert!(rt.world.team_alive(team).unwrap().contains(&living));
    assert!(rt.world.flat_alive().contains(&living));
    assert_ne!(rt.world.alive_group_len_containing(living), 0);
    let children: Vec<_> = rt.entities.iter().filter_map(|(id, e)| (e.runtime.owner != id).then_some(id)).collect();
    assert_prepared_reset(&prepared, &mut runner, boss, &children);
}

#[test]
fn phantom_team_shrink_and_children_reset_through_prepared_runner() { shrink_and_reset_prepared_battle("phantom"); }

#[test]
fn yuyuko_butterfly_shrink_and_children_reset_through_prepared_runner() { shrink_and_reset_prepared_battle("yuyuko"); }

#[test]
fn flandre_clones_and_owner_shrink_reset_through_prepared_runner() { shrink_and_reset_prepared_battle("flandre"); }

#[test]
fn charm_expires_synchronously_before_ongoing_burn_in_same_turn() {
    let (mut runner, boss, enemy) = battle("yuyuko");
    let rt = runner.runtime_mut();
    let physical_team = rt.entities.get(boss).unwrap().runtime.team;
    let hp = rt.entities.get(boss).unwrap().runtime.hp;
    only_skill(rt, boss, None);
    let mut setup = RunUpdates::new();
    rt.apply_plain_charm_hit_into(enemy, boss, &mut setup);
    rt.apply_rosmontic_burn(enemy, boss, 10, &mut setup);
    assert_ne!(rt.plain_effective_team(boss), physical_team);
    assert_eq!(rt.ros_state(boss).burn_turns, 1);
    let updates = act(&mut runner, boss);
    let release = updates.updates.iter().position(|u| u.message.contains("从[魅惑]中解除")).unwrap();
    let burn = updates.updates.iter().position(|u| u.message.contains("[烧伤]")).unwrap();
    assert!(release < burn);
    let rt = runner.runtime_mut();
    assert_eq!(rt.plain_effective_team(boss), physical_team);
    assert_eq!(rt.ros_state(boss).burn_turns, 0);
    let entity = rt.entities.get(boss).unwrap();
    assert!(entity.states.entry(PLAIN_CHARM_STATE_KEY).is_none());
    assert!(entity.runtime.hp < hp);
}

#[test]
fn yuyuko_cleanses_charm_and_burn_before_that_turns_expiry() {
    let (mut runner, boss, enemy) = battle("yuyuko");
    only_skill(runner.runtime_mut(), boss, Some(1));
    act(&mut runner, boss);
    let rt = runner.runtime_mut();
    let physical_team = rt.entities.get(boss).unwrap().runtime.team;
    let hp = rt.entities.get(boss).unwrap().runtime.hp;
    only_skill(rt, boss, Some(3));
    let mut setup = RunUpdates::new();
    rt.apply_plain_charm_hit_into(enemy, boss, &mut setup);
    rt.apply_rosmontic_burn(enemy, boss, 10, &mut setup);
    assert_ne!(rt.plain_effective_team(boss), physical_team);
    assert_eq!(rt.ros_state(boss).burn_turns, 1);
    let updates = act(&mut runner, boss);
    assert!(updates.updates.iter().any(|u| u.message.contains("通向黄泉的诱蛾灯")));
    assert!(
        !updates
            .updates
            .iter()
            .any(|u| u.message.contains("从[魅惑]中解除") || u.message.contains("[烧伤]"))
    );
    let rt = runner.runtime_mut();
    assert_eq!(rt.plain_effective_team(boss), physical_team);
    assert_eq!(rt.ros_state(boss).burn_turns, 0);
    let e = rt.entities.get(boss).unwrap();
    assert!(e.states.entry(PLAIN_CHARM_STATE_KEY).is_none());
    assert_eq!(e.runtime.hp, (f64::from(hp) * 0.8) as i32);
}

#[test]
fn divine_stats_refresh_does_not_commit_recharm() {
    let config = default_custom_runtime_import_config().unwrap();
    let mut runner = RuntimeRunner::from_custom_mixed_namerena_raw("ran@!\n\nalpha\n\nbeta\n\nseed:2".into(), config).unwrap();
    let rt = runner.runtime_mut();
    let ran = find_entity(rt, "ran");
    let alpha = find_entity(rt, "alpha");
    let beta = find_entity(rt, "beta");
    let alpha_team = rt.plain_effective_team(alpha);
    let beta_team = rt.plain_effective_team(beta);
    let mut updates = RunUpdates::new();
    rt.apply_plain_charm_hit_into(alpha, ran, &mut updates);
    rt.apply_plain_charm_hit_into(beta, ran, &mut updates);
    assert_eq!(rt.plain_effective_team(ran), alpha_team);
    rt.ros_cast(ran, 2, vec![ran], &mut updates);
    assert_eq!(rt.plain_effective_team(ran), alpha_team);
    let charm = rt
        .entities
        .get(ran)
        .unwrap()
        .states
        .entry(PLAIN_CHARM_STATE_KEY)
        .and_then(StateEntry::charm_value)
        .unwrap();
    assert_eq!(charm.1, Some(alpha_team));
    assert_eq!(charm.2, Some(beta_team));
    rt.entities.get_mut(ran).unwrap().apply_runtime_stats_from_template(true);
    assert_eq!(rt.plain_effective_team(ran), beta_team);
}

#[test]
fn frozen_skip_does_not_advance_pcb_pre_or_post_action_timers() {
    let (mut runner, boss, enemy) = battle("yuyuko");
    let rt = runner.runtime_mut();
    only_skill(rt, boss, None);
    durable(rt, enemy);
    rt.entities.get_mut(enemy).unwrap().template.skills = SkillLoadout::default();
    let mut setup = RunUpdates::new();
    let state = rt.ros_state(boss);
    state.butterfly_revived = true;
    state.butterfly_turns = 3;
    state.divine_turns = 3;
    rt.apply_rosmontic_burn(enemy, boss, 10, &mut setup);
    rt.entities
        .get_mut(boss)
        .unwrap()
        .states
        .add_ice_frozen_step(PLAIN_ICE_STATE_KEY, 16 * 1024);
    give_next_action(&mut runner, boss);
    let skipped = runner.run_round();
    assert!(skipped.action.is_none());
    let rt = runner.runtime();
    assert_eq!(rt.entities.get(boss).unwrap().runtime.move_state.speed_points, 1);
    assert!(rt.entities.get(boss).unwrap().states.is_frozen());
    let timers = |rt: &CombatRuntime| {
        let state = rt.entities.get(boss).unwrap().runtime.rosmontic.as_ref().unwrap();
        (state.butterfly_turns, state.divine_turns, state.burn_turns)
    };
    assert_eq!(timers(rt), (3, 3, 1));
    act(&mut runner, enemy);
    assert_eq!(timers(runner.runtime()), (3, 3, 1));
    let rt = runner.runtime_mut();
    rt.entities
        .get_mut(boss)
        .unwrap()
        .states
        .set_payload(PLAIN_ICE_STATE_KEY, StatePayload::Ice { frozen_step: 0 });
    select_rng(rt, |rng| (0..4).all(|_| rng.next_u8() & 3 != 0));
    let updates = act(&mut runner, boss);
    assert!(updates.updates.iter().any(|u| u.message.contains("从[冰冻]中解除")));
    assert_eq!(timers(runner.runtime()), (2, 2, 0));
}

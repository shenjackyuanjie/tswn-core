//! 验证重制后的实际状态、伤害、固定出勤行动及快照，不冻结源码写法。
use super::super::*;
fn battle() -> (RuntimeRunner, EntityIdx, Vec<EntityIdx>) {
    let config = default_custom_runtime_import_config().unwrap();
    let runner =
        RuntimeRunner::from_custom_mixed_namerena_raw("Squall@!\nally@red\n\nalpha\nbeta\ngamma\ndelta\n\nseed:7".into(), config)
            .unwrap();
    let boss = runner.runtime().entities.iter().find(|(_, e)| e.runtime.squall.is_some()).unwrap().0;
    let team = runner.runtime().plain_effective_team(boss);
    let enemies = runner
        .runtime()
        .world
        .flat_alive()
        .iter()
        .copied()
        .filter(|id| runner.runtime().plain_effective_team(*id) != team)
        .collect();
    (runner, boss, enemies)
}
#[test]
fn attendance_occupies_first_two_actions_and_defense_expires_after_second() {
    let (mut runner, boss, _) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    let base = rt.entities.get(boss).unwrap().runtime.defense;
    rt.begin_squall_action(boss, &mut u);
    rt.drain_squall_action_into(boss, &mut u);
    let e = rt.entities.get(boss).unwrap();
    assert_eq!(e.runtime.shield, 100);
    assert_eq!(e.runtime.defense, base * 2);
    rt.finish_squall_action(Some(boss), &mut u);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.defense, base * 2);
    rt.entities.get_mut(boss).unwrap().refresh_runtime_stats_from_template();
    assert_eq!(rt.entities.get(boss).unwrap().runtime.resistance, base * 2);
    rt.begin_squall_action(boss, &mut u);
    rt.drain_squall_action_into(boss, &mut u);
    assert!(u.updates.iter().any(|u| u.message == "[0][正在出勤回来的路上]"));
    assert!(
        !u.updates
            .iter()
            .any(|u| u.message.contains("使用[建立模型]") || u.message.contains("使用[开喷]"))
    );
    rt.finish_squall_action(Some(boss), &mut u);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.defense, base);
}
#[test]
fn return_always_uses_action_and_spawns_one_or_two_only_on_half_the_draws() {
    let (runner, boss, _) = battle();
    let mut none = 0;
    let mut one = 0;
    let mut two = 0;
    for seed in 0..256 {
        let mut r = runner.clone();
        let rt = r.runtime_mut();
        rt.rng = crate::rc4::RC4::new(seed.to_string().as_bytes(), 1);
        rt.entities.get_mut(boss).unwrap().runtime.squall.as_mut().unwrap().turn = 2;
        let mut u = RunUpdates::new();
        rt.drain_squall_action_into(boss, &mut u);
        let count = rt.entities.iter().filter(|(_, e)| e.template.name == "猎豹").count();
        match count {
            0 => none += 1,
            1 => one += 1,
            2 => two += 1,
            _ => panic!("wrong count"),
        }
        assert!(u.updates.iter().any(|u| u.message == "[0][正在出勤回来的路上]"));
    }
    assert!(none > 80 && none < 180);
    assert!(one > 20 && two > 20);
}
#[test]
fn every_enemy_main_action_gives_sample_and_summons_do_not() {
    let (mut runner, boss, enemies) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    for enemy in enemies.iter().cycle().take(9) {
        rt.finish_squall_action(Some(*enemy), &mut u);
    }
    assert_eq!(rt.entities.get(boss).unwrap().runtime.squall.as_ref().unwrap().samples, 6);
    rt.entities.get_mut(boss).unwrap().runtime.squall.as_mut().unwrap().samples = 0;
    rt.tiger_spawn_leopard(enemies[0], &mut u);
    let child = rt.entities.iter().find(|(_, e)| e.template.name == "猎豹").unwrap().0;
    rt.finish_squall_action(Some(child), &mut u);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.squall.as_ref().unwrap().samples, 0);
}
#[test]
fn model_consumes_two_samples_and_keeps_separate_hits() {
    let (mut runner, boss, _) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.entities.get_mut(boss).unwrap().runtime.squall.as_mut().unwrap().samples = 6;
    rt.squall_weighted(boss, &mut u);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.squall.as_ref().unwrap().samples, 4);
    assert!(u.updates.iter().any(|u| u.message.contains("建立模型")));
    assert!(u.updates.iter().any(|u| u.message.contains("伤害")));
}
#[test]
fn pager_accelerates_both_teams_and_forces_permanent_red_next_action() {
    let (mut runner, boss, _) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.entities.get_mut(boss).unwrap().runtime.squall.as_mut().unwrap().turn = 3;
    let speeds: Vec<_> = rt.entities.iter().map(|(id, e)| (id, e.runtime.speed)).collect();
    rt.squall_pager(boss, &mut u);
    for (id, speed) in speeds {
        assert_eq!(rt.entities.get(id).unwrap().runtime.speed, speed + 50);
    }
    let attack = rt.entities.get(boss).unwrap().runtime.attack;
    rt.begin_squall_action(boss, &mut u);
    assert!(rt.entities.get(boss).unwrap().runtime.squall.as_ref().unwrap().red);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.attack, attack * 3 / 2);
    rt.dispel_squall_effects(boss, &mut u);
    rt.try_squall_red(boss, &mut u);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.attack, attack * 3 / 2);
}
#[test]
fn team_helpers_have_requested_hp_and_skill_roles() {
    let (mut runner, boss, _) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.squall_team(boss, &mut u);
    let helpers: Vec<_> = rt.entities.iter().filter(|(_, e)| e.runtime.squall_helper.is_some()).collect();
    assert_eq!(helpers.len(), 2);
    for (_, e) in helpers {
        assert_eq!(e.runtime.hp, 168);
        assert_eq!(e.runtime.speed, 212);
        assert_eq!(e.runtime.magic, 128);
        assert!(e.runtime.squall.is_none());
    }
}
#[test]
fn weaken_halves_highest_stats_and_absorbs_remaining_half_hp() {
    let (mut runner, boss, enemies) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    let target = enemies[0];
    {
        let e = rt.entities.get_mut(target).unwrap();
        e.template.attack = 10000;
        e.runtime.attack = 10000;
        e.runtime.hp = 200;
    }
    rt.entities.get_mut(boss).unwrap().runtime.hp = 100;
    rt.squall_weaken(boss, &mut u);
    let e = rt.entities.get_mut(target).unwrap();
    assert_eq!(e.runtime.attack, 5000);
    assert_eq!(e.runtime.hp, 100);
    e.refresh_runtime_stats_from_template();
    assert_eq!(e.runtime.attack, 5000);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.hp, 200);
    assert!(!rt.has_rosmontic_action(target));
}
#[test]
fn barrage_uses_all_samples_and_is_unavoidable() {
    let (mut runner, boss, enemies) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.entities.get_mut(boss).unwrap().runtime.squall.as_mut().unwrap().samples = 6;
    for id in enemies {
        let e = rt.entities.get_mut(id).unwrap();
        e.runtime.hp = 100000;
        e.template.max_hp = 100000;
        e.runtime.agility = 100000;
    }
    rt.squall_barrage(boss, &mut u);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.squall.as_ref().unwrap().samples, 0);
    assert_eq!(u.updates.iter().filter(|u| u.message.contains("受到[2]点伤害")).count(), 6);
    assert!(!u.updates.iter().any(|u| u.message.contains("回避")));
}
#[test]
fn tyranny_affects_one_to_three_and_adds_attack() {
    let (mut runner, boss, enemies) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    let before: Vec<_> = enemies
        .iter()
        .map(|id| (*id, rt.entities.get(*id).unwrap().runtime.move_state.speed_points))
        .collect();
    let attack = rt.entities.get(boss).unwrap().runtime.attack;
    rt.squall_tyranny(boss, &mut u);
    let marked: Vec<_> = enemies
        .into_iter()
        .filter(|id| rt.entities.get(*id).unwrap().runtime.magic_point == -64)
        .collect();
    assert!((1..=3).contains(&marked.len()));
    for id in marked {
        let e = rt.entities.get(id).unwrap();
        assert_eq!(
            e.runtime.move_state.speed_points,
            before.iter().find(|(idx, _)| *idx == id).unwrap().1 - 2048
        );
        assert!(e.states.is_frozen());
    }
    assert_eq!(rt.entities.get(boss).unwrap().runtime.attack, attack + 30);
}
#[test]
fn red_threshold_is_strict_and_refresh_or_dispel_cannot_remove_it() {
    let (mut runner, boss, _) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.entities.get_mut(boss).unwrap().runtime.hp = 180;
    rt.try_squall_red(boss, &mut u);
    assert!(!rt.entities.get(boss).unwrap().runtime.squall.as_ref().unwrap().red);
    rt.entities.get_mut(boss).unwrap().runtime.hp = 179;
    rt.try_squall_red(boss, &mut u);
    rt.dispel_squall_effects(boss, &mut u);
    rt.entities.get_mut(boss).unwrap().refresh_runtime_stats_from_template();
    assert_eq!(rt.entities.get(boss).unwrap().runtime.attack, 159);
}
#[test]
fn full_battle_clone_and_no_capture_keep_same_result() {
    let (mut runner, _, _) = battle();
    let mut copy = runner.clone();
    let a = runner.run_until_winner(20000);
    let b = copy.run_to_completion(20000);
    assert_eq!(a.winner_team, b.winner_team);
    assert_eq!(a.rounds.len(), b.rounds);
    assert!(runner.runtime().world.winner_team().is_some());
}

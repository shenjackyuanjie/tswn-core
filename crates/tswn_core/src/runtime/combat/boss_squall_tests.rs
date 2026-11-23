//! 验证科研技能的实际状态、伤害链、日志顺序及普通/批量执行一致性。
use super::*;
use crate::runtime::update::UpdateType;

fn battle() -> (RuntimeRunner, EntityIdx, Vec<EntityIdx>) {
    let config = default_custom_runtime_import_config().unwrap();
    let runner = RuntimeRunner::from_custom_mixed_namerena_raw("Squall@!\n\nalpha\nbeta\n\nseed:1".into(), config).unwrap();
    let boss = runner.runtime().entities.iter().find(|(_, e)| e.runtime.squall.is_some()).unwrap().0;
    let enemies = runner.runtime().world.flat_alive().iter().copied().filter(|idx| *idx != boss).collect();
    (runner, boss, enemies)
}
fn messages(updates: &RunUpdates) -> Vec<&str> {
    updates
        .updates
        .iter()
        .filter(|u| u.update_type != UpdateType::NextLine)
        .map(|u| u.message.as_ref())
        .collect()
}

#[test]
fn fixed_stats_identity_and_icon_match_requested_character() {
    let (runner, boss, _) = battle();
    let e = runner.runtime().entities.get(boss).unwrap();
    let r = &e.runtime;
    assert_eq!(
        [r.attack, r.defense, r.speed, r.agility, r.magic, r.resistance, r.wisdom],
        [106; 7]
    );
    assert_eq!(r.hp, 360);
    assert_eq!(e.template.max_hp, 360);
    assert!(r.flags.contains(PlayerKindFlags::BOSS));
    assert_eq!(
        crate::namerena::icon_render::render_icon_vec_from_name("Squall@!"),
        crate::namerena::icon_render::render_icon_vec_from_name("Squall@Squall")
    );
    let config = default_custom_runtime_import_config().unwrap();
    let normal = RuntimeRunner::from_custom_mixed_namerena_raw("Squall@Squall\n\nalpha".into(), config).unwrap();
    assert!(normal.runtime().entities.iter().all(|(_, e)| e.runtime.squall.is_none()));
}

#[test]
fn samples_use_real_enemy_actions_cap_at_six_and_put_quote_next() {
    let (mut runner, boss, enemies) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    for _ in 0..9 {
        rt.finish_squall_action(Some(enemies[0]), &mut u);
        rt.finish_squall_action(Some(boss), &mut u);
    }
    assert_eq!(rt.entities.get(boss).unwrap().runtime.squall.as_ref().unwrap().samples, 6);
    let m = messages(&u);
    for update in &u.updates {
        if SAMPLE_QUOTES.contains(&update.message.as_ref()) {
            assert!(update.dialogue);
        }
        if update.message.contains("开始测试") {
            assert!(!update.dialogue);
        }
    }
    for (i, line) in m.iter().enumerate() {
        if line.contains("开始测试") {
            assert!(SAMPLE_QUOTES.contains(&m[i + 1]));
        }
    }
    let before = u.updates.len();
    rt.finish_squall_action(Some(boss), &mut u);
    assert_eq!(u.updates.len(), before);
}

#[test]
fn sample_quotes_vary_without_consuming_combat_randomness() {
    let (mut runner, boss, _) = battle();
    let rt = runner.runtime_mut();
    let before = rt.rng.clone();
    let mut seen = std::collections::HashSet::new();
    let mut previous = None;
    for turn in 0..100 {
        rt.entities.get_mut(boss).unwrap().runtime.squall.as_mut().unwrap().turn = turn;
        let quote = rt.next_squall_sample_quote(boss);
        assert!(SAMPLE_QUOTES.contains(&quote));
        assert_ne!(Some(quote), previous);
        previous = Some(quote);
        seen.insert(quote);
    }
    assert_eq!(seen.len(), 5);
    assert_eq!(rt.rng.i, before.i);
    assert_eq!(rt.rng.j, before.j);
    assert_eq!(rt.rng.main_val, before.main_val);
}

#[test]
fn weighted_attacks_consume_samples_and_report_real_damage() {
    let (mut runner, boss, enemies) = battle();
    let rt = runner.runtime_mut();
    let s = rt.entities.get_mut(boss).unwrap().runtime.squall.as_mut().unwrap();
    s.samples = 5;
    s.marked = Some(enemies[1]);
    let before: Vec<_> = enemies.iter().map(|idx| rt.entities.get(*idx).unwrap().runtime.hp).collect();
    let mut u = RunUpdates::new();
    rt.squall_weighted(boss, &mut u);
    let m = messages(&u);
    assert_eq!(m[0], "[0]使用[加权平均]");
    assert_eq!(m[1], "你这个分数很高，但实际打下来不是这么回事。");
    assert!(
        enemies
            .iter()
            .enumerate()
            .any(|(i, idx)| rt.entities.get(*idx).unwrap().runtime.hp < before[i])
    );
    assert_eq!(rt.entities.get(boss).unwrap().runtime.squall.as_ref().unwrap().samples, 3);
}

#[test]
fn taunt_overlay_restores_without_damaging_original_stats() {
    let (mut runner, boss, enemies) = battle();
    let rt = runner.runtime_mut();
    rt.entities.get_mut(enemies[0]).unwrap().runtime.attract_bits = 100.0f64.to_bits();
    rt.entities.get_mut(enemies[1]).unwrap().runtime.attract_bits = 900.0f64.to_bits();
    let mut u = RunUpdates::new();
    let before: Vec<_> = enemies
        .iter()
        .map(|idx| CombatRuntime::plain_smart_attack_score(&rt.world, *idx, &rt.entities.get(*idx).unwrap().runtime))
        .collect();
    rt.squall_taunt(boss, &mut u);
    for i in 0..2 {
        let after = CombatRuntime::plain_smart_attack_score(&rt.world, enemies[i], &rt.entities.get(enemies[i]).unwrap().runtime);
        assert!((after - before[1 - i]).abs() < 0.00001);
    }
    rt.dispel_squall_effects(enemies[0], &mut u);
    assert_eq!(rt.entities.get(enemies[0]).unwrap().runtime.attract(), 100.0);
    assert_eq!(rt.entities.get(enemies[1]).unwrap().runtime.attract(), 900.0);
    assert!(rt.entities.get(boss).unwrap().runtime.squall.as_ref().unwrap().taunt.is_none());
}

#[test]
fn iron_and_exposure_modify_real_defend_values_and_dispel_clears_boost() {
    let (mut runner, boss, enemies) = battle();
    let rt = runner.runtime_mut();
    let s = rt.entities.get_mut(boss).unwrap().runtime.squall.as_mut().unwrap();
    s.turn = 1;
    s.iron_until = 4;
    s.amplify = 75;
    let mut v = RuntimeDefendValue::Damage {
        value: 100,
        caster: enemies[0],
        target: boss,
        kind: DamageKind::Default,
    };
    rt.drain_post_defend_hooks_into(boss, &mut RunUpdates::new(), &mut v);
    assert_eq!(v.damage(), Some(60));
    rt.dispel_squall_effects(boss, &mut RunUpdates::new());
    let s = rt.entities.get_mut(boss).unwrap().runtime.squall.as_mut().unwrap();
    assert_eq!(s.amplify, 0);
    s.exposed = true;
    let mut v = RuntimeDefendValue::Damage {
        value: 100,
        caster: enemies[0],
        target: boss,
        kind: DamageKind::Default,
    };
    rt.apply_squall_defense(boss, &mut v);
    assert_eq!(v.damage(), Some(125));
}

#[test]
fn team_creates_two_distinct_limited_helpers_that_can_act() {
    let (mut runner, boss, _) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.squall_team(boss, &mut u);
    let helpers: Vec<_> = rt
        .entities
        .iter()
        .filter(|(_, e)| e.runtime.squall_helper.is_some())
        .map(|(idx, _)| idx)
        .collect();
    assert_eq!(helpers.len(), 2);
    for helper in helpers {
        let e = rt.entities.get(helper).unwrap();
        assert_eq!(e.template.max_hp, 43);
        assert_eq!(e.runtime.root_owner, boss);
        assert!(e.runtime.is_minion());
        assert!(e.runtime.squall.is_none());
        for _ in 0..2 {
            rt.drain_squall_action_into(helper, &mut u);
        }
    }
    assert!(rt.entities.get(boss).unwrap().runtime.squall.as_ref().unwrap().summoned);
}

#[test]
fn healing_is_limited_and_uses_actual_recovery_and_amplify_cap() {
    let (mut runner, boss, _) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    for i in 1..=3 {
        rt.entities.get_mut(boss).unwrap().runtime.hp = 100;
        rt.squall_heal(boss, &mut u);
        assert_eq!(rt.entities.get(boss).unwrap().runtime.hp, 143);
        assert_eq!(rt.entities.get(boss).unwrap().runtime.squall.as_ref().unwrap().heal_count, i);
    }
    let s = rt.entities.get_mut(boss).unwrap().runtime.squall.as_mut().unwrap();
    s.amplify = 500;
    assert_eq!(rt.squall_take_amplify(boss), 2.0);
    assert_eq!(rt.squall_take_amplify(boss), 1.0);
}

#[test]
fn fourth_active_misfires_and_exposure_expires_on_next_action() {
    let (mut runner, boss, _) = battle();
    let rt = runner.runtime_mut();
    let s = rt.entities.get_mut(boss).unwrap().runtime.squall.as_mut().unwrap();
    s.active_count = 3;
    s.samples = 1;
    let mut u = RunUpdates::new();
    rt.drain_squall_action_into(boss, &mut u);
    let s = rt.entities.get(boss).unwrap().runtime.squall.as_ref().unwrap();
    assert!(s.exposed);
    assert_eq!(s.samples, 0);
    assert_eq!(s.active_count, 0);
    assert!(messages(&u).iter().any(|m| m.contains("失去1层")));
    rt.begin_squall_action(boss, &mut u);
    assert!(!rt.entities.get(boss).unwrap().runtime.squall.as_ref().unwrap().exposed);
}

#[test]
fn burn_is_once_preserves_base_stats_and_can_kill_itself() {
    let (mut runner, boss, _) = battle();
    let rt = runner.runtime_mut();
    {
        let e = rt.entities.get_mut(boss).unwrap();
        e.runtime.hp = 90;
        e.runtime.squall.as_mut().unwrap().samples = 6;
    }
    let mut u = RunUpdates::new();
    rt.try_squall_burn(boss, &mut u);
    let e = rt.entities.get(boss).unwrap();
    assert_eq!(e.runtime.attack, 144);
    assert_eq!(e.runtime.speed, 156);
    assert_eq!(e.runtime.squall.as_ref().unwrap().samples, 0);
    rt.try_squall_burn(boss, &mut u);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.attack, 144);
    rt.entities.get_mut(boss).unwrap().refresh_runtime_stats_from_template();
    assert_eq!(rt.entities.get(boss).unwrap().runtime.attack, 144);
    rt.entities.get_mut(boss).unwrap().runtime.hp = 3;
    rt.finish_squall_action(Some(boss), &mut u);
    assert!(!rt.entities.get(boss).unwrap().runtime.alive);
}

#[test]
fn delayed_pruning_only_marks_summons_and_can_be_dispelled() {
    let (mut runner, boss, enemies) = battle();
    let rt = runner.runtime_mut();
    let kind = rt.registry.player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT).unwrap();
    let team = rt.entities.get(enemies[0]).unwrap().runtime.team;
    let template = PlayerTemplate::with_kind(0, "敌方使魔", kind, team, 100, 10);
    let target = rt
        .entities
        .spawn_from_template_with_owner(template, &rt.registry, Some(enemies[0]), Some(enemies[0]));
    rt.world.add_spawned_alive(target, team);
    let mut u = RunUpdates::new();
    rt.squall_prune(boss, &mut u);
    assert_eq!(
        rt.entities.get(boss).unwrap().runtime.squall.as_ref().unwrap().pruning,
        vec![target]
    );
    rt.dispel_squall_effects(target, &mut u);
    assert!(rt.entities.get(boss).unwrap().runtime.squall.as_ref().unwrap().pruning.is_empty());
}

#[test]
fn capture_and_fast_completion_have_identical_final_state() {
    let (mut captured, _, _) = battle();
    let (mut fast, _, _) = battle();
    let a = captured.run_until_winner(20000);
    let b = fast.run_to_completion(20000);
    assert!(captured.have_winner());
    assert!(fast.have_winner());
    let _ = (a, b);
    assert_eq!(captured.runtime().entities, fast.runtime().entities);
}

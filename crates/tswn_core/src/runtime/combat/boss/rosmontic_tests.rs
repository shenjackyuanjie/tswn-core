//! 验证退场、剑技打断、复活次数和低血阶段，避免只验证静态配置。
use super::super::*;
fn battle(name: &str) -> (RuntimeRunner, EntityIdx, EntityIdx) {
    let config = default_custom_runtime_import_config().unwrap();
    let runner = RuntimeRunner::from_custom_mixed_namerena_raw(format!("{name}@!\n\nalpha\n\nseed:2"), config).unwrap();
    let boss = runner.runtime().entities.iter().find(|(_, e)| e.template.name == name).unwrap().0;
    let enemy = runner.runtime().entities.iter().find(|(_, e)| e.template.name == "alpha").unwrap().0;
    (runner, boss, enemy)
}
#[test]
fn rin_retires_on_eighth_action_and_keeps_signed_stats() {
    let (mut runner, boss, _) = battle("rin");
    let rt = runner.runtime_mut();
    assert_eq!(rt.entities.get(boss).unwrap().runtime.attack, -26);
    let mut updates = RunUpdates::new();
    for _ in 0..7 {
        rt.drain_rosmontic_action_into(boss, &mut updates);
    }
    assert!(rt.entities.get(boss).unwrap().is_active());
    rt.drain_rosmontic_action_into(boss, &mut updates);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.hp, 0);
    assert!(!rt.entities.get(boss).unwrap().runtime.alive);
    assert!(updates.updates.iter().any(|u| u.message.contains("梦的沙堤")));
}
#[test]
fn megia_resolve_revives_only_once() {
    let (mut runner, boss, _) = battle("megia");
    let rt = runner.runtime_mut();
    let mut updates = RunUpdates::new();
    rt.entities.get_mut(boss).unwrap().runtime.hp = 0;
    rt.drain_die_hooks_into(boss, &mut updates);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.hp, 199);
    rt.entities.get_mut(boss).unwrap().runtime.hp = 0;
    rt.drain_die_hooks_into(boss, &mut updates);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.hp, 0);
}
#[test]
fn megia_high_charge_breaks_even_when_hit_crosses_low_threshold() {
    let (mut runner, boss, _) = battle("megia");
    let rt = runner.runtime_mut();
    let mut updates = RunUpdates::new();
    rt.drain_rosmontic_action_into(boss, &mut updates);
    assert!(rt.entities.get(boss).unwrap().runtime.rosmontic.as_ref().unwrap().charge_target.is_some());
    rt.entities.get_mut(boss).unwrap().runtime.hp = 190;
    rt.interrupt_megia_into(boss, &mut updates);
    assert!(rt.entities.get(boss).unwrap().runtime.rosmontic.as_ref().unwrap().charge_target.is_none());
    rt.drain_rosmontic_action_into(boss, &mut updates);
    rt.interrupt_megia_into(boss, &mut updates);
    assert!(rt.entities.get(boss).unwrap().runtime.rosmontic.as_ref().unwrap().charge_target.is_some());
    rt.drain_rosmontic_action_into(boss, &mut updates);
    assert!(updates.updates.iter().any(|u| u.message.contains("肃清的一刀")));
}

#[test]
fn rumia_destroy_triggers_once_and_survives_state_refresh() {
    let (mut runner, boss, _) = battle("rumia");
    let rt = runner.runtime_mut();
    let mut updates = RunUpdates::new();
    rt.entities.get_mut(boss).unwrap().runtime.hp = 100;
    rt.entities.get_mut(boss).unwrap().runtime.rosmontic.as_mut().unwrap().rumia_def_bonus = 50;
    rt.entities.get_mut(boss).unwrap().runtime.defense += 50;
    rt.entities.get_mut(boss).unwrap().runtime.resistance += 50;
    rt.upgrade_rumia_into(boss, &mut updates);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.attack, 245);
    rt.upgrade_rumia_into(boss, &mut updates);
    rt.entities.get_mut(boss).unwrap().refresh_runtime_stats_from_template();
    assert_eq!(rt.entities.get(boss).unwrap().runtime.attack, 245);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.defense, 570);
}

#[test]
fn rumia_black_mist_damages_attacker_without_recursive_reflection() {
    let (mut runner, boss, enemy) = battle("rumia");
    let rt = runner.runtime_mut();
    let before = rt.entities.get(enemy).unwrap().runtime.hp;
    let mut updates = RunUpdates::new();
    rt.reflect_rumia_into(boss, enemy, &mut updates);
    assert!(rt.entities.get(enemy).unwrap().runtime.hp < before);
    assert!(updates.updates.iter().any(|u| u.message.contains("黑雾")));
}

#[test]
fn new_bosses_finish_real_battles_and_preserve_snapshot_state() {
    for name in ["rin", "megia", "rumia"] {
        let (mut runner, boss, _) = battle(name);
        let original = runner.runtime().entities.get(boss).unwrap().runtime.clone();
        for _ in 0..5000 {
            if runner.runtime().world.winner_team().is_some() {
                break;
            }
            runner.runtime_mut().run_minimal_round();
        }
        assert!(runner.runtime().world.winner_team().is_some(), "{name}");
        runner
            .runtime_mut()
            .entities
            .get_mut(boss)
            .unwrap()
            .runtime
            .reset_battle_state_from(&original);
        assert_eq!(
            runner.runtime().entities.get(boss).unwrap().runtime.rosmontic,
            original.rosmontic
        );
    }
}

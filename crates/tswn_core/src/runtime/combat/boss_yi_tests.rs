//! 咦 Boss 的固定数值、召唤与拜谢压制。
use super::boss_yi::*;
use super::*;

fn battle() -> (RuntimeRunner, EntityIdx) {
    let config = default_custom_runtime_import_config().unwrap();
    let runner = RuntimeRunner::from_custom_mixed_namerena_raw("咦@!\n\nalpha\nbeta\n\nseed:1".into(), config).unwrap();
    let boss = runner.runtime().entities.iter().find(|(_, e)| e.template.name == ID).unwrap().0;
    (runner, boss)
}

#[test]
fn fixed_stats_and_custom_icon() {
    let (runner, boss) = battle();
    let entity = runner.runtime().entities.get(boss).unwrap();
    let r = &entity.runtime;
    assert_eq!(
        [r.attack, r.defense, r.agility, r.magic, r.resistance, r.wisdom],
        [99, 87, 82, 98, 89, 89]
    );
    assert_eq!(r.speed, 249);
    assert_eq!(r.hp, 487);
    assert_eq!(entity.template.display_name, "咦");
}

#[test]
fn first_action_summons_two_baixie() {
    let (mut runner, boss) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.drain_yi_action_into(boss, &mut u);
    let count = rt.entities.iter().filter(|(_, e)| e.template.name == BAIXIE && e.runtime.alive).count();
    assert_eq!(count, 2);
}

#[test]
fn baixie_drains_mp_and_marks_target() {
    let (mut runner, boss) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.drain_yi_action_into(boss, &mut u);
    let baixie = rt.entities.iter().find(|(_, e)| e.template.name == BAIXIE).map(|(idx, _)| idx).unwrap();
    let target = rt.entities.iter().find(|(_, e)| e.template.name == "alpha").map(|(idx, _)| idx).unwrap();
    let before = rt.entities.get(target).unwrap().runtime.magic_point;
    rt.drain_baixie_action_into(baixie, &mut u);
    assert!(rt.entities.get(target).unwrap().runtime.magic_point <= before.saturating_sub(20).max(0));
}

#[test]
fn baixie_heals_the_boss_only_after_everyone_is_thanked() {
    let (mut runner, boss) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.drain_yi_action_into(boss, &mut u);
    let baixie = rt.entities.iter().find(|(_, e)| e.template.name == BAIXIE).map(|(idx, _)| idx).unwrap();
    if let Some(entity) = rt.entities.get_mut(boss) {
        entity.runtime.hp = 300;
    }
    // 还有未被拜谢的敌人时不回血。
    rt.drain_baixie_action_into(baixie, &mut u);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.hp, 300);
    // 把所有敌人都标记为已拜谢后再行动，才会回 20 点。
    let enemies = rt.yi_enemies(baixie);
    if let Some(state) = rt.entities.get_mut(baixie).and_then(|e| e.runtime.baixie.as_mut()) {
        state.thanked.extend(enemies);
    }
    rt.drain_baixie_action_into(baixie, &mut u);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.hp, 320);
}

#[test]
fn baixie_has_level_seventy_reflect() {
    let (mut runner, boss) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.drain_yi_action_into(boss, &mut u);
    let baixie = rt.entities.iter().find(|(_, e)| e.template.name == BAIXIE).map(|(idx, _)| idx).unwrap();
    let levels = rt.entities.get(baixie).unwrap().template.skills.levels().to_vec();
    assert_eq!(levels, vec![70], "拜谢应当只有 70 级反弹");
}

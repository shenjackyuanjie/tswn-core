//! 琪露诺的数值、冰伤免疫、技能变化与跨局复位。
use super::super::*;
use crate::runtime::combat::boss::cirno::*;

fn battle() -> (RuntimeRunner, EntityIdx) {
    let config = default_custom_runtime_import_config().unwrap();
    let runner = RuntimeRunner::from_custom_mixed_namerena_raw("cirno@!\n\nalpha\nbeta\n\nseed:1".into(), config).unwrap();
    let boss = runner.runtime().entities.iter().find(|(_, e)| e.template.name == ID).unwrap().0;
    (runner, boss)
}

#[test]
fn initial_stats_follow_reference_initialization() {
    let (runner, boss) = battle();
    let e = runner.runtime().entities.get(boss).unwrap();
    let r = &e.runtime;
    assert_eq!(
        [r.attack, r.defense, r.speed, r.agility, r.magic, r.resistance, r.wisdom],
        [44, 39, 214, 44, 45, 54, 63]
    );
    assert_eq!(r.hp, 810);
    assert_eq!(e.template.display_name, "琪露诺");
}

#[test]
fn ice_damage_immunity_does_not_imply_freeze_immunity() {
    let (mut runner, boss) = battle();
    let rt = runner.runtime_mut();
    assert!(!rt.ice_immune(boss));
    let mut updates = RunUpdates::new();
    let mut damage = RuntimeDefendValue::Damage {
        value: 100,
        caster: EntityIdx(1),
        target: boss,
        kind: DamageKind::Ice,
    };
    rt.drain_post_defend_hooks_into(boss, &mut updates, &mut damage);
    assert_eq!(damage.damage(), Some(0));
    assert!(updates.updates.iter().any(|u| u.message.contains("免疫")));
}

#[test]
fn absolute_zero_updates_level_magic_and_wisdom() {
    let (mut runner, boss) = battle();
    let rt = runner.runtime_mut();
    let mut updates = RunUpdates::new();
    rt.cirno_absolute_zero(boss, &mut updates);
    let runtime = &rt.entities.get(boss).unwrap().runtime;
    assert_eq!(runtime.cirno.as_ref().unwrap().zero_level, 80);
    assert_eq!(runtime.magic, 54);
    assert_eq!(runtime.wisdom, 5);
    assert!(updates.updates.iter().any(|u| u.message.contains("变[⑨]")));
}

#[test]
fn battle_reset_restores_skill_level() {
    let (mut runner, boss) = battle();
    let template = runner.runtime().entities.get(boss).unwrap().runtime.clone();
    runner
        .runtime_mut()
        .entities
        .get_mut(boss)
        .unwrap()
        .runtime
        .cirno
        .as_mut()
        .unwrap()
        .zero_level = 0;
    runner
        .runtime_mut()
        .entities
        .get_mut(boss)
        .unwrap()
        .runtime
        .reset_battle_state_from(&template);
    assert_eq!(
        runner.runtime().entities.get(boss).unwrap().runtime.cirno.as_ref().unwrap().zero_level,
        90
    );
}

//! 新纪元 Boss 的固定数值、图标与技能轮转。
use super::boss_xjy::*;
use super::*;

fn battle() -> (RuntimeRunner, EntityIdx) {
    let config = default_custom_runtime_import_config().unwrap();
    let runner = RuntimeRunner::from_custom_mixed_namerena_raw("新纪元@!\n\nalpha\nbeta\n\nseed:1".into(), config).unwrap();
    let boss = runner.runtime().entities.iter().find(|(_, e)| e.template.name == ID).unwrap().0;
    (runner, boss)
}

#[test]
fn fixed_stats_and_icon() {
    let (runner, boss) = battle();
    let entity = runner.runtime().entities.get(boss).unwrap();
    let r = &entity.runtime;
    assert_eq!(
        [r.attack, r.defense, r.speed, r.agility, r.magic, r.resistance, r.wisdom],
        [106, 106, 266, 106, 106, 106, 106]
    );
    assert_eq!(r.hp, 360);
    assert_eq!(entity.template.display_name, "新纪元");
    assert_eq!(
        crate::namerena::icon_render::render_icon_vec_from_name("新纪元@!"),
        crate::namerena::icon_render::render_icon_vec_from_name("新纪元@新纪元")
    );
}

#[test]
fn leopard_fill_reaches_twice_the_enemy_count() {
    let (mut runner, boss) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.xjy_leopard_fill(boss, &mut u);
    let allies = rt.xjy_units(boss).len() + 1;
    let enemies = rt.xjy_enemies(boss).len();
    assert!(allies >= enemies * 2 || enemies == 0);
}

//! 沙冰鱼 Boss 的固定数值、魔高与全体普攻。
use super::super::*;
use super::fish::*;

fn battle() -> (RuntimeRunner, EntityIdx) {
    let config = default_custom_runtime_import_config().unwrap();
    let runner = RuntimeRunner::from_custom_mixed_namerena_raw("Shabby_fish@!\n\nalpha\nbeta\n\nseed:1".into(), config).unwrap();
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
    assert_eq!(entity.template.display_name, "沙冰鱼");
    assert_eq!(
        crate::namerena::icon_render::render_icon_vec_from_name("Shabby_fish@!"),
        crate::namerena::icon_render::render_icon_vec_from_name("Shabby_fish@Shabby_fish")
    );
}

#[test]
fn magic_zero_hits_two_and_restores_later() {
    let (mut runner, boss) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.fish_magic_zero(boss, &mut u);
    let zeroed = rt.entities.iter().filter(|(_, e)| e.runtime.magic == 0 && e.template.name != ID).count();
    assert_eq!(zeroed, 2);
    // kodu 原版的这条断言依赖 turn-1 的技能播种，原测试未触发 turn-1：
    // 净化熟练度 200 由 drain_fish_action_into 在 turn 0→1 时写入，fish_magic_zero 本身不碰技能。
    // 补一次 drain 让回合真正推进到 turn-1，再验证播种结果。
    rt.drain_fish_action_into(boss, &mut u);
    // 净化熟练度 200。
    assert_eq!(rt.entities.get(boss).unwrap().template.skills.levels(), &[200]);
}

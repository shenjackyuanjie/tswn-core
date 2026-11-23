//! 曙光 Boss 的固定数值与技能轮转。
use super::*;

fn battle() -> (RuntimeRunner, EntityIdx) {
    let config = default_custom_runtime_import_config().unwrap();
    let runner = RuntimeRunner::from_custom_mixed_namerena_raw("smdd@!\n\nalpha\nbeta\n\nseed:1".into(), config).unwrap();
    let boss = runner.runtime().entities.iter().find(|(_, e)| e.template.name == ID).unwrap().0;
    (runner, boss)
}

#[test]
fn fixed_stats_match_squall_and_use_squall_icon_rules() {
    let (runner, boss) = battle();
    let entity = runner.runtime().entities.get(boss).unwrap();
    let r = &entity.runtime;
    // 106 是外显值，实际战斗速度补上普通公式的 160。
    assert_eq!(
        [r.attack, r.defense, r.speed, r.agility, r.magic, r.resistance, r.wisdom],
        [106, 106, 266, 106, 106, 106, 106]
    );
    assert_eq!(r.hp, 360);
    assert_eq!(entity.template.display_name, "曙光");
    assert_eq!(
        crate::namerena::icon_render::render_icon_vec_from_name("smdd@!"),
        crate::namerena::icon_render::render_icon_vec_from_name("nan@nan")
    );
}

#[test]
fn skills_rotate_over_seven_turns() {
    let (mut runner, boss) = battle();
    let rt = runner.runtime_mut();
    let mut seen = std::collections::HashSet::new();
    for turn in 1..=7u32 {
        if let Some(state) = rt.entities.get_mut(boss).and_then(|e| e.runtime.smdd.as_mut()) {
            state.turn = turn;
        }
        seen.insert(format!("{:?}", rt.next_smdd_skill(boss)));
    }
    for expected in ["Rp", "Tiger", "Monkey", "Leopard", "Mining", "Win", "God"] {
        assert!(seen.contains(expected), "{expected} 应出现在轮转里");
    }
}

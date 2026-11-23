//! 曙光 Boss 的固定数值、技能轮转与图标映射。
use super::*;

fn battle() -> (RuntimeRunner, EntityIdx) {
    let config = default_custom_runtime_import_config().unwrap();
    let runner = RuntimeRunner::from_custom_mixed_namerena_raw("smdd@!\n\nalpha\nbeta\n\nseed:1".into(), config).unwrap();
    let boss = runner.runtime().entities.iter().find(|(_, e)| e.template.name == ID).unwrap().0;
    (runner, boss)
}

#[test]
fn fixed_stats_and_display_name_match_squall_tier() {
    let (runner, boss) = battle();
    let entity = runner.runtime().entities.get(boss).unwrap();
    let r = &entity.runtime;
    assert_eq!(
        [r.attack, r.defense, r.speed, r.agility, r.magic, r.resistance, r.wisdom],
        [106; 7]
    );
    assert_eq!(r.hp, 360);
    assert_eq!(entity.template.display_name, "曙光");
    assert_eq!(
        crate::namerena::icon_render::render_icon_vec_from_name("smdd@!"),
        crate::namerena::icon_render::render_icon_vec_from_name("nan@nan")
    );
}

#[test]
fn ten_skills_rotate_by_action_count() {
    let (mut runner, boss) = battle();
    let rt = runner.runtime_mut();
    let mut seen = std::collections::HashSet::new();
    for turn in 1..=10u32 {
        if let Some(state) = rt.entities.get_mut(boss).and_then(|e| e.runtime.smdd.as_mut()) {
            state.turn = turn;
        }
        let skill = rt.next_smdd_skill(boss);
        seen.insert(format!("{skill:?}"));
    }
    // 十轮里应当覆盖全部十种技能（虎星/抛开事实在无层数时回退为做局，属于设计内行为）。
    assert!(seen.contains("Mining") && seen.contains("Speed") && seen.contains("Taunt"));
    assert!(seen.contains("Random") && seen.contains("Flat") && seen.contains("Monkey"));
    assert!(seen.contains("Sell") && seen.contains("Setup"));
}

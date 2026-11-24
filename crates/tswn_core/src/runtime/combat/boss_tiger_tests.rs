//! TigerStar 的固定属性与主要技能。
use super::*;

fn battle() -> (RuntimeRunner, EntityIdx) {
    let config = default_custom_runtime_import_config().unwrap();
    let runner = RuntimeRunner::from_custom_mixed_namerena_raw("TigerStar@!\n\nalpha\nbeta\n\nseed:1".into(), config).unwrap();
    let boss = runner.runtime().entities.iter().find(|(_, e)| e.template.name == ID).unwrap().0;
    (runner, boss)
}

#[test]
fn fixed_stats_match_squall_tier() {
    let (runner, boss) = battle();
    let entity = runner.runtime().entities.get(boss).unwrap();
    let r = &entity.runtime;
    assert_eq!(
        [r.attack, r.defense, r.speed, r.agility, r.magic, r.resistance, r.wisdom],
        [106, 106, 266, 106, 106, 106, 106]
    );
    assert_eq!(r.hp, 360);
    assert_eq!(entity.template.display_name, "豹星");
}

#[test]
fn grass_drains_mp_and_spawns_leopards() {
    let (mut runner, boss) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.drain_tiger_action_into(boss, &mut u);
    let messages: Vec<&str> = u.updates.iter().map(|update| update.message.as_ref()).collect();
    assert!(messages.iter().any(|message| message.contains("草你")));
    // 第二轮是我是M。
    rt.drain_tiger_action_into(boss, &mut u);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.tiger.as_ref().unwrap().m_turns, 2);
}

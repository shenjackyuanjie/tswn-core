//! 被子 Boss 的数值、召唤、伤害传导与不可选中验证。
use super::*;

fn battle() -> (RuntimeRunner, EntityIdx, Vec<EntityIdx>) {
    let config = default_custom_runtime_import_config().unwrap();
    let runner = RuntimeRunner::from_custom_mixed_namerena_raw("quilt@!\n\nalpha\nbeta\n\nseed:1".into(), config).unwrap();
    let boss = runner.runtime().entities.iter().find(|(_, e)| e.template.name == ID).unwrap().0;
    let enemies = runner.runtime().world.flat_alive().iter().copied().filter(|idx| *idx != boss).collect();
    (runner, boss, enemies)
}

#[test]
fn fixed_stats_are_500_hp_and_100_attributes() {
    let (runner, boss, _) = battle();
    let e = runner.runtime().entities.get(boss).unwrap();
    let r = &e.runtime;
    assert_eq!(
        [r.attack, r.defense, r.speed, r.agility, r.magic, r.resistance, r.wisdom],
        [100; 7]
    );
    assert_eq!(r.hp, 500);
    assert_eq!(e.template.max_hp, 500);
    assert!(r.flags.contains(PlayerKindFlags::BOSS));
}

#[test]
fn summon_creates_child_that_shares_half_damage_with_the_quilt() {
    let (mut runner, boss, _) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.quilt_spawn(boss, Child::Monkey, &mut u);
    let child = rt.quilt_monkeys(boss)[0];
    assert!(rt.quilt_share_owner(child) == Some(boss));
    // 召唤会继承 Boss 的 @! 徽章名，必须还原成衍生物自己的图标键。
    assert_eq!(rt.entities.get(child).unwrap().template.id_key_name, "quilt-monkey");
    let before = rt.entities.get(boss).unwrap().runtime.hp;
    rt.apply_plain_attack_damage_into(EntityIdx(0), child, 40, &mut u);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.hp, before - 20);
}

#[test]
fn direct_hits_on_the_body_deal_no_damage() {
    let (mut runner, boss, enemies) = battle();
    let rt = runner.runtime_mut();
    let before = rt.entities.get(boss).unwrap().runtime.hp;
    let mut v = RuntimeDefendValue::Damage {
        value: 120,
        caster: enemies[0],
        target: boss,
        kind: DamageKind::Default,
    };
    rt.apply_squall_defense(boss, &mut v);
    // 科研钩子不处理被子，走 Boss 专属防御后伤害归零。
    let mut u = RunUpdates::new();
    rt.drain_post_defend_hooks_into(boss, &mut u, &mut v);
    assert_eq!(v.damage(), Some(0));
    assert_eq!(rt.entities.get(boss).unwrap().runtime.hp, before);
}

#[test]
fn merge_adds_stats_and_removes_one_monkey() {
    let (mut runner, boss, _) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.quilt_spawn(boss, Child::Monkey, &mut u);
    rt.quilt_spawn(boss, Child::Monkey, &mut u);
    let attack = rt.entities.get(rt.quilt_monkeys(boss)[0]).unwrap().runtime.attack;
    rt.quilt_merge(boss, &mut u);
    let monkeys = rt.quilt_monkeys(boss);
    assert_eq!(monkeys.len(), 1);
    assert_eq!(rt.entities.get(monkeys[0]).unwrap().runtime.attack, attack * 2);
}

#[test]
fn quilt_is_skipped_while_summons_live_and_targetable_afterwards() {
    let (mut runner, boss, enemies) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.quilt_spawn(boss, Child::Monkey, &mut u);
    let monkey = rt.quilt_monkeys(boss)[0];
    assert!(rt.quilt_untargetable(boss), "quilt should be protected");
    // 有召唤物时，普攻选敌永远不会落在被子上。
    for _ in 0..50 {
        let target = rt.select_plain_default_attack_target(enemies[0], true).unwrap();
        assert_eq!(target, monkey);
    }
    // 清掉召唤物后，本体重新可以被选中，战斗才能继续。
    rt.kill_entity_without_damage_into(monkey, &mut u);
    let target = rt.select_plain_default_attack_target(enemies[0], true).unwrap();
    assert_eq!(target, boss);
}

#[test]
fn direct_damage_to_the_quilt_stays_on_the_quilt() {
    let (mut runner, boss, enemies) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.quilt_spawn(boss, Child::Monkey, &mut u);
    let monkey = rt.quilt_monkeys(boss)[0];
    let before = rt.entities.get(boss).unwrap().runtime.hp;
    let monkey_before = rt.entities.get(monkey).unwrap().runtime.hp;
    rt.apply_plain_attack_damage_into(enemies[0], boss, 40, &mut u);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.hp, before - 40);
    assert_eq!(rt.entities.get(monkey).unwrap().runtime.hp, monkey_before);
}

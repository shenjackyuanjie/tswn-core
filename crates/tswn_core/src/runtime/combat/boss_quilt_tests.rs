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
fn charged_wheel_cannot_exchange_quilt_hp_or_action_points() {
    let (mut runner, boss, enemies) = battle();
    let rt = runner.runtime_mut();
    let actor = enemies[0];
    rt.entities.get_mut(actor).unwrap().runtime.charge.active = true;
    rt.entities.get_mut(boss).unwrap().runtime.move_state.speed_points = 500;
    let mut u = RunUpdates::new();
    rt.drain_plain_wheel_exchange_into(actor, boss, &mut u);
    let e = rt.entities.get(boss).unwrap();
    assert_eq!((e.runtime.hp, e.runtime.move_state.speed_points), (1000, 500));
    assert!(u.updates.iter().any(|line| line.message == "[0][回避]了攻击"));
}

#[test]
fn sleep_is_fifth_action_and_blocks_dot_until_next_action() {
    let (mut runner, boss, enemies) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.quilt_spawn(boss, Child::Fish, &mut u);
    let fish = rt.quilt_fish(boss)[0];
    for turn in 1..=6 {
        u = RunUpdates::new();
        rt.round += 7;
        rt.drain_quilt_minion_action_into(fish, enemies[0], &mut u);
        assert_eq!(u.updates.iter().any(|line| line.message == "[0]开始睡5觉"), turn == 5);
        if turn == 5 {
            let before = rt.entities.get(fish).unwrap().runtime.hp;
            rt.apply_poison_tick_damage_into(enemies[0], fish, 40, &mut u);
            assert_eq!(rt.entities.get(fish).unwrap().runtime.hp, before);
        }
    }
}

#[test]
fn occupation_applies_slow_to_the_entire_enemy_side() {
    let (mut runner, boss, enemies) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    for _ in 0..OCCUPY_MONKEYS {
        rt.quilt_spawn(boss, Child::Monkey, &mut u);
    }
    rt.drain_quilt_action_into(boss, &mut u);
    assert!(enemies.iter().all(|idx| rt.entities.get(*idx).unwrap().states.entry(78).is_some()));
    let line = u.updates.iter().find(|line| line.message == "猴区宣布占领此群。").unwrap();
    assert!(rt.quilt_monkeys(boss).contains(&EntityIdx(line.caster as u32)));
}

#[test]
fn fixed_stats_are_1000_hp_and_100_attributes() {
    let (runner, boss, _) = battle();
    let e = runner.runtime().entities.get(boss).unwrap();
    let r = &e.runtime;
    assert_eq!(
        [r.attack, r.defense, r.speed, r.agility, r.magic, r.resistance, r.wisdom],
        [100; 7]
    );
    assert_eq!(r.hp, 1000);
    assert_eq!(e.template.max_hp, 1000);
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
fn quilt_is_skipped_with_or_without_summons() {
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
    // 召唤物清空后，本体仍从目标池排除。
    rt.kill_entity_without_damage_into(monkey, &mut u);
    assert_eq!(rt.select_plain_default_attack_target(enemies[0], true), None);
}

#[test]
fn direct_damage_to_the_quilt_is_ignored() {
    let (mut runner, boss, enemies) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.quilt_spawn(boss, Child::Monkey, &mut u);
    let monkey = rt.quilt_monkeys(boss)[0];
    let before = rt.entities.get(boss).unwrap().runtime.hp;
    let monkey_before = rt.entities.get(monkey).unwrap().runtime.hp;
    rt.apply_plain_attack_damage_into(enemies[0], boss, 40, &mut u);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.hp, before);
    assert_eq!(rt.entities.get(monkey).unwrap().runtime.hp, monkey_before);
}

#[test]
fn monkey_arrest_keeps_its_own_status_and_removes_the_monkey() {
    let (mut runner, boss, enemies) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.quilt_spawn(boss, Child::Monkey, &mut u);
    let monkey = rt.quilt_monkeys(boss)[0];
    rt.minion_state_mut(monkey).unwrap().turn = 11;
    rt.drain_quilt_minion_action_into(monkey, enemies[0], &mut u);
    assert!(!rt.entities.get(monkey).unwrap().runtime.alive);
    assert_eq!(rt.entities.get(enemies[0]).unwrap().runtime.quilt_arrests.len(), 1);
    assert!(u.updates.iter().any(|line| line.message == "[1]消失了"));
}

#[test]
fn fish_dives_then_spits_a_monkey() {
    let (mut runner, boss, enemies) = battle();
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.quilt_spawn(boss, Child::Fish, &mut u);
    let fish = rt.quilt_fish(boss)[0];
    let before = rt.quilt_monkeys(boss).len();
    rt.drain_quilt_minion_action_into(fish, enemies[0], &mut u);
    assert!(rt.entities.get(fish).unwrap().runtime.quilt_minion.as_ref().unwrap().pending_spawn);
    // 下一次行动开始就召出猴区，不再额外等待一个行动。
    rt.drain_quilt_minion_action_into(fish, enemies[0], &mut u);
    assert_eq!(rt.quilt_monkeys(boss).len(), before + 1);
}

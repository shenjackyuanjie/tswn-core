//! 本批八个 Boss 的实际招式、计时、召唤、驱散、死亡与快照回归。
use super::super::*;
fn battle(name: &str) -> (RuntimeRunner, EntityIdx, EntityIdx) {
    let config = default_custom_runtime_import_config().unwrap();
    let runner = RuntimeRunner::from_custom_mixed_namerena_raw(format!("{name}@!\n\nalpha\n\nseed:2"), config).unwrap();
    let boss = runner.runtime().entities.iter().find(|(_, e)| e.template.name == name).unwrap().0;
    let enemy = runner.runtime().entities.iter().find(|(_, e)| e.template.name == "alpha").unwrap().0;
    (runner, boss, enemy)
}
fn durable(rt: &mut CombatRuntime, target: EntityIdx) {
    let e = rt.entities.get_mut(target).unwrap();
    e.template.max_hp = 1000000;
    e.runtime.hp = 1000000;
    e.runtime.agility = 0;
}
#[test]
fn reimu_barrier_absorbs_small_hits_breaks_on_large_and_expires() {
    let (mut runner, boss, enemy) = battle("reimu");
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.ros_cast(boss, 2, vec![boss], &mut u);
    let defense = rt.entities.get(boss).unwrap().runtime.resistance + 64;
    let hp = rt.entities.get(boss).unwrap().runtime.hp;
    rt.drain_plain_attack_after_dodge_into(
        enemy,
        boss,
        true,
        f64::from(defense * 50),
        DamageKind::Default,
        None,
        PlainAttackOnDamage::None,
        &mut u,
    );
    assert_eq!(rt.entities.get(boss).unwrap().runtime.hp, hp - 1);
    rt.drain_plain_attack_after_dodge_into(
        enemy,
        boss,
        true,
        f64::from(defense * 200),
        DamageKind::Default,
        None,
        PlainAttackOnDamage::None,
        &mut u,
    );
    assert_eq!(rt.entities.get(boss).unwrap().runtime.hp, hp - 61);
    assert_eq!(rt.ros_state(boss).barrier_turns, 0);
    rt.ros_cast(boss, 2, vec![boss], &mut u);
    for _ in 0..4 {
        rt.end_rosmontic_action(boss, &mut u);
    }
    assert_eq!(rt.ros_state(boss).barrier_turns, 0);
}
#[test]
fn dream_charges_six_actions_then_fires_once() {
    let (mut runner, boss, enemy) = battle("reimu");
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    durable(rt, enemy);
    rt.ros_cast(boss, 1, vec![enemy], &mut u);
    for _ in 0..5 {
        rt.begin_rosmontic_action(boss, &mut u);
        assert!(!rt.ros_skill_allowed(boss, 1));
        assert!(!rt.ros_skill_allowed(boss, 3));
    }
    rt.begin_rosmontic_action(boss, &mut u);
    assert!(rt.ros_skill_allowed(boss, 1));
    rt.ros_cast(boss, 1, vec![enemy], &mut u);
    assert_eq!(rt.ros_state(boss).dream, 0);
    assert_eq!(rt.ros_state(boss).levels[1], 0);
    assert!(rt.entities.get(enemy).unwrap().runtime.hp < 1000000);
}
#[test]
fn marisa_charges_caps_and_consumes_energy_and_comet_is_two_stage() {
    let (mut runner, boss, enemy) = battle("marisa");
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    durable(rt, enemy);
    let hp = rt.entities.get(enemy).unwrap().runtime.hp;
    for _ in 0..20 {
        rt.ros_default(boss, &mut u);
    }
    assert_eq!(rt.ros_state(boss).energy, 8);
    assert_eq!(rt.entities.get(enemy).unwrap().runtime.hp, hp);
    rt.ros_cast(boss, 4, vec![enemy], &mut u);
    assert_eq!(rt.ros_state(boss).energy, 0);
    let speed = rt.entities.get(boss).unwrap().effective_speed();
    rt.ros_cast(boss, 1, vec![enemy], &mut u);
    assert_eq!(rt.ros_state(boss).charged_skill, Some(1));
    assert!(rt.entities.get(boss).unwrap().effective_speed() > speed);
    rt.drain_touhou_action_into(boss, &mut u);
    assert!(rt.ros_state(boss).charged_skill.is_none());
    rt.ros_cast(boss, 0, vec![enemy], &mut u);
    assert_eq!(rt.ros_state(boss).levels[0], 0);
    assert_eq!(rt.ros_state(boss).energy, 6);
}
#[test]
fn daiyousei_sacrifice_revives_cirno_and_doubles_six_attributes() {
    let config = default_custom_runtime_import_config().unwrap();
    let mut runner =
        RuntimeRunner::from_custom_mixed_namerena_raw("daiyousei@!\ncirno@!\n\nalpha\n\nseed:3".into(), config).unwrap();
    let rt = runner.runtime_mut();
    let boss = rt.entities.iter().find(|(_, e)| e.template.name == "daiyousei").unwrap().0;
    let child = rt.entities.iter().find(|(_, e)| e.template.name == "cirno").unwrap().0;
    let mut u = RunUpdates::new();
    let attack = rt.entities.get(child).unwrap().runtime.attack;
    rt.entities.get_mut(child).unwrap().runtime.hp = 0;
    rt.drain_summon_explode_self_death_into(child, &mut u);
    rt.ros_cast(boss, 0, vec![child], &mut u);
    let c = rt.entities.get(child).unwrap();
    assert_eq!(c.runtime.hp, c.template.max_hp);
    assert_eq!(c.runtime.attack, attack * 2);
    assert!(c.runtime.alive);
    assert!(!rt.entities.get(boss).unwrap().runtime.alive);
}
#[test]
fn daiyousei_summons_two_fairies_and_support_cleans_burn() {
    let (mut runner, boss, enemy) = battle("daiyousei");
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.ros_cast(boss, 2, vec![boss], &mut u);
    let children: Vec<_> = rt
        .entities
        .iter()
        .filter(|(_, e)| e.runtime.rosmontic.as_ref().is_some_and(|s| s.child_kind == 1))
        .collect();
    assert_eq!(children.len(), 2);
    for (_, e) in children {
        assert_eq!(e.runtime.hp, 100);
        assert_eq!(e.runtime.move_state.speed_points, -2048);
    }
    rt.ros_state(boss).burn_owner = Some(enemy);
    rt.ros_state(boss).burn_turns = 2;
    rt.entities.get_mut(boss).unwrap().runtime.hp = 400;
    rt.ros_cast(boss, 1, vec![boss], &mut u);
    assert!(rt.entities.get(boss).unwrap().runtime.hp > 400);
    assert_eq!(rt.ros_state(boss).burn_turns, 0);
}
#[test]
fn hong_aura_qi_and_charge_restore_after_dispel() {
    let (mut runner, boss, enemy) = battle("hong");
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    durable(rt, enemy);
    let defense = rt.entities.get(boss).unwrap().runtime.defense;
    let boost = rt.entities.get(boss).unwrap().runtime.at_boost_bits;
    rt.ros_cast(boss, 1, vec![boss], &mut u);
    assert!(rt.entities.get(boss).unwrap().runtime.defense > defense);
    rt.dispel_rosmontic_into(boss, &mut u);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.defense, defense);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.at_boost_bits, boost);
    rt.ros_cast(boss, 2, vec![enemy], &mut u);
    let qi = f64::from_bits(rt.ros_state(boss).qi_bits);
    assert_eq!(qi, 1.2);
    rt.ros_cast(boss, 4, vec![boss], &mut u);
    assert_eq!(rt.ros_state(boss).charged_skill, Some(4));
    rt.drain_touhou_action_into(boss, &mut u);
    assert!(rt.ros_state(boss).charged_skill.is_none());
    assert_eq!(rt.ros_state(boss).levels[4], 0);
}
#[test]
fn koakuma_summon_book_and_nonempty_callback_defense() {
    let (mut runner, boss, enemy) = battle("koakuma");
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    durable(rt, enemy);
    assert!(!rt.ros_skill_allowed(boss, 1));
    rt.entities.get_mut(boss).unwrap().runtime.hp = 450;
    assert!(rt.ros_skill_allowed(boss, 1));
    rt.ros_cast(boss, 1, vec![boss], &mut u);
    let child = rt.ros_state(boss).child.unwrap();
    let c = rt.entities.get(child).unwrap();
    assert_eq!(c.runtime.hp, 666);
    assert_eq!(c.runtime.attack, 66);
    assert_eq!(c.runtime.magic, 99);
    rt.ros_cast(boss, 2, vec![enemy], &mut u);
    rt.ros_cast(boss, 2, vec![enemy], &mut u);
    assert_eq!(rt.ros_state(boss).levels[2], 0);
    let def = rt.entities.get(boss).unwrap().runtime.defense + 64;
    let a = rt.drain_plain_attack_after_dodge_into(
        enemy,
        boss,
        false,
        f64::from(def * 100),
        DamageKind::Default,
        None,
        PlainAttackOnDamage::None,
        &mut u,
    );
    let b = rt.drain_plain_attack_after_dodge_into(
        enemy,
        boss,
        false,
        f64::from(def * 100),
        DamageKind::Physical,
        None,
        PlainAttackOnDamage::None,
        &mut u,
    );
    assert_eq!(a, 100);
    assert_eq!(b, 65);
}
#[test]
fn patchouli_element_dependencies_unlock_stone_and_burn_ticks() {
    let (mut runner, boss, enemy) = battle("patchouli");
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    durable(rt, enemy);
    assert!(!rt.ros_skill_allowed(boss, 10));
    for element in 0..5 {
        let skill = if rt.ros_skill_allowed(boss, element * 2) {
            element * 2
        } else {
            element * 2 + 1
        };
        rt.ros_cast(boss, skill, vec![enemy], &mut u);
        assert!(!rt.ros_skill_allowed(boss, skill));
    }
    assert_eq!(rt.ros_state(boss).elements, 31);
    assert!(rt.ros_skill_allowed(boss, 10));
    rt.ros_cast(boss, 10, vec![enemy], &mut u);
    rt.apply_rosmontic_burn(boss, enemy, 10, &mut u);
    rt.apply_rosmontic_burn(boss, enemy, 10, &mut u);
    assert_eq!(rt.ros_state(enemy).burn_turns, 2);
    assert!(!rt.has_rosmontic_action(enemy));
    let hp = rt.entities.get(enemy).unwrap().runtime.hp;
    rt.end_rosmontic_action(enemy, &mut u);
    rt.end_rosmontic_action(enemy, &mut u);
    assert!(rt.entities.get(enemy).unwrap().runtime.hp < hp);
    assert_eq!(rt.ros_state(enemy).burn_turns, 0);
}
#[test]
fn burn_can_kill_and_uses_normal_death_chain() {
    let (mut runner, boss, enemy) = battle("patchouli");
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    rt.apply_rosmontic_burn(boss, enemy, 100, &mut u);
    rt.entities.get_mut(enemy).unwrap().runtime.hp = 1;
    rt.end_rosmontic_action(enemy, &mut u);
    assert!(!rt.entities.get(enemy).unwrap().runtime.alive);
}
#[test]
fn sakuya_world_exempts_remilia_shadow_and_temporary_defense_restore() {
    let (mut runner, boss, enemy) = battle("sakuya");
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    durable(rt, enemy);
    let agility = rt.entities.get(boss).unwrap().runtime.agility;
    rt.begin_rosmontic_action(boss, &mut u);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.agility, agility + 21);
    rt.ros_cast(boss, 0, vec![enemy], &mut u);
    assert!(rt.entities.get(enemy).unwrap().runtime.move_state.speed_points < -8192);
    rt.ros_cast(boss, 1, vec![boss], &mut u);
    let child = rt.ros_state(boss).child.unwrap();
    assert_eq!(rt.entities.get(child).unwrap().runtime.hp, 1);
    let def = rt.entities.get(enemy).unwrap().runtime.defense;
    rt.ros_cast(boss, 3, vec![enemy], &mut u);
    assert_eq!(rt.entities.get(enemy).unwrap().runtime.defense, def);
    assert_eq!(rt.entities.get(enemy).unwrap().runtime.agility, 0);
    for _ in 0..4 {
        rt.begin_rosmontic_action(boss, &mut u);
    }
    assert_eq!(rt.ros_state(boss).time_stop, 0);
}
#[test]
fn remilia_marks_increase_offense_and_incoming_damage_then_dispel() {
    let (mut runner, boss, enemy) = battle("remilia");
    let rt = runner.runtime_mut();
    let mut u = RunUpdates::new();
    durable(rt, enemy);
    let attack = rt.entities.get(boss).unwrap().runtime.attack;
    let speed = rt.entities.get(boss).unwrap().effective_speed();
    rt.ros_cast(boss, 1, vec![boss], &mut u);
    assert_eq!(rt.ros_state(boss).scarlet, 1);
    assert_eq!(rt.ros_state(boss).levels[1], 21);
    assert!(rt.entities.get(boss).unwrap().runtime.attack > attack);
    assert!(rt.entities.get(boss).unwrap().effective_speed() > speed);
    let def = rt.entities.get(boss).unwrap().runtime.defense + 64;
    let amount = rt.drain_plain_attack_after_dodge_into(
        enemy,
        boss,
        false,
        f64::from(def * 100),
        DamageKind::Default,
        None,
        PlainAttackOnDamage::None,
        &mut u,
    );
    assert_eq!(amount, 175);
    rt.dispel_rosmontic_into(boss, &mut u);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.attack, attack);
    assert_eq!(rt.ros_state(boss).scarlet, 0);
    rt.entities.get_mut(boss).unwrap().runtime.hp = 100;
    rt.apply_plain_attack_damage_with_covid_and_on_damage_into(
        boss,
        enemy,
        100,
        None,
        PlainAttackOnDamage::RosmonticAbsorbFifth,
        &mut u,
    );
    assert_eq!(rt.entities.get(boss).unwrap().runtime.hp, 120);
}
#[test]
fn all_eight_finish_battles_and_capture_matches_fast_execution_and_reset() {
    for name in [
        "reimu",
        "marisa",
        "daiyousei",
        "hong",
        "koakuma",
        "patchouli",
        "sakuya",
        "remilia",
    ] {
        let (mut runner, boss, _) = battle(name);
        let original = runner.runtime().entities.get(boss).unwrap().runtime.clone();
        let mut copy = runner.clone();
        let full = runner.run_until_winner(20000);
        let fast = copy.run_to_completion(20000);
        assert!(!full.guard_exhausted, "{name}");
        assert_eq!(full.winner_team, fast.winner_team, "{name}");
        assert_eq!(full.rounds.len(), fast.rounds, "{name}");
        assert_eq!(runner.runtime().entities, copy.runtime().entities, "{name}");
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

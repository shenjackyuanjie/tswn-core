//! 验证退场、剑技打断、复活次数和低血阶段，避免只验证静态配置。
//! 来源：KODU16（`kodu16/main`，`cf460c84`）。
use super::super::*;
fn battle(name: &str) -> (RuntimeRunner, EntityIdx, EntityIdx) {
    let config = default_custom_runtime_import_config().unwrap();
    let runner = RuntimeRunner::from_custom_mixed_namerena_raw(format!("{name}@!\n\nalpha\n\nseed:2"), config).unwrap();
    let boss = runner.runtime().entities.iter().find(|(_, e)| e.template.name == name).unwrap().0;
    let enemy = runner.runtime().entities.iter().find(|(_, e)| e.template.name == "alpha").unwrap().0;
    (runner, boss, enemy)
}
#[test]
fn rin_retires_on_eighth_action_and_keeps_signed_stats() {
    let (mut runner, boss, _) = battle("rin");
    let rt = runner.runtime_mut();
    assert_eq!(rt.entities.get(boss).unwrap().runtime.attack, -26);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.magic, -26);
    let mut updates = RunUpdates::new();
    for _ in 0..7 {
        rt.drain_rosmontic_action_into(boss, &mut updates);
    }
    assert!(rt.entities.get(boss).unwrap().is_active());
    rt.drain_rosmontic_action_into(boss, &mut updates);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.hp, 0);
    assert!(!rt.entities.get(boss).unwrap().runtime.alive);
    assert!(updates.updates.iter().any(|u| u.message.contains("梦的沙堤")));
}
#[test]
fn megia_resolve_revives_only_once() {
    let (mut runner, boss, enemy) = battle("megia");
    let rt = runner.runtime_mut();
    let mut updates = RunUpdates::new();
    assert!(rt.apply_plain_attack_damage_into(enemy, boss, 1000, &mut updates));
    rt.drain_plain_lethal_damage_into(enemy, boss, &mut updates);
    let revived = rt.entities.get(boss).unwrap();
    assert_eq!(revived.runtime.hp, 199);
    assert!(revived.runtime.alive);
    assert!(revived.runtime.rosmontic.as_ref().unwrap().resolve_used);
    assert!(rt.apply_plain_attack_damage_into(enemy, boss, 1000, &mut updates));
    rt.drain_plain_lethal_damage_into(enemy, boss, &mut updates);
    let dead = rt.entities.get(boss).unwrap();
    assert_eq!(dead.runtime.hp, 0);
    assert!(!dead.runtime.alive);
    assert_eq!(updates.updates.iter().filter(|u| u.message.contains("无名的决意")).count(), 1);
}
#[test]
fn megia_high_charge_breaks_even_when_hit_crosses_low_threshold() {
    let (mut runner, boss, enemy) = battle("megia");
    let rt = runner.runtime_mut();
    let mut updates = RunUpdates::new();
    rt.drain_rosmontic_action_into(boss, &mut updates);
    assert!(rt.entities.get(boss).unwrap().runtime.rosmontic.as_ref().unwrap().charge_target.is_some());
    let amount = rt.entities.get(boss).unwrap().runtime.hp - 190;
    assert!(!rt.apply_plain_attack_damage_into(enemy, boss, amount, &mut updates));
    assert!(rt.entities.get(boss).unwrap().runtime.rosmontic.as_ref().unwrap().charge_target.is_none());
    rt.drain_rosmontic_action_into(boss, &mut updates);
    assert!(!rt.apply_plain_attack_damage_into(enemy, boss, 1, &mut updates));
    assert!(rt.entities.get(boss).unwrap().runtime.rosmontic.as_ref().unwrap().charge_target.is_some());
    rt.drain_rosmontic_action_into(boss, &mut updates);
    assert!(updates.updates.iter().any(|u| u.message.contains("肃清的一刀")));
}

#[test]
fn rumia_destroy_triggers_once_and_survives_state_refresh() {
    let (mut runner, boss, _) = battle("rumia");
    let rt = runner.runtime_mut();
    let mut updates = RunUpdates::new();
    rt.entities.get_mut(boss).unwrap().runtime.hp = 100;
    rt.entities.get_mut(boss).unwrap().runtime.rosmontic.as_mut().unwrap().rumia_def_bonus = 50;
    rt.entities.get_mut(boss).unwrap().runtime.defense += 50;
    rt.entities.get_mut(boss).unwrap().runtime.resistance += 50;
    rt.upgrade_rumia_into(boss, &mut updates);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.attack, 245);
    rt.upgrade_rumia_into(boss, &mut updates);
    rt.entities.get_mut(boss).unwrap().refresh_runtime_stats_from_template();
    assert_eq!(rt.entities.get(boss).unwrap().runtime.attack, 245);
    assert_eq!(rt.entities.get(boss).unwrap().runtime.defense, 570);
}

#[test]
fn rumia_black_mist_damages_attacker_without_recursive_reflection() {
    let (mut runner, boss, enemy) = battle("rumia");
    let rt = runner.runtime_mut();
    let before = rt.entities.get(enemy).unwrap().runtime.hp;
    let mut updates = RunUpdates::new();
    rt.reflect_rumia_into(boss, enemy, &mut updates);
    assert!(rt.entities.get(enemy).unwrap().runtime.hp < before);
    assert!(updates.updates.iter().any(|u| u.message.contains("黑雾")));
}

#[test]
fn new_bosses_finish_real_battles_and_preserve_snapshot_state() {
    for name in ["rin", "megia", "rumia"] {
        let (mut runner, boss, _) = battle(name);
        let original = runner.runtime().entities.get(boss).unwrap().runtime.clone();
        for _ in 0..5000 {
            if runner.runtime().world.winner_team().is_some() {
                break;
            }
            runner.runtime_mut().run_minimal_round();
        }
        assert!(runner.runtime().world.winner_team().is_some(), "{name}");
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

#[test]
fn rin_immunity_also_covers_attacks_that_skip_dodge() {
    let (mut runner, boss, enemy) = battle("rin");
    let rt = runner.runtime_mut();
    let before = rt.entities.get(boss).unwrap().runtime.hp;
    let mut updates = RunUpdates::new();
    rt.drain_plain_attack_after_dodge_into(
        enemy,
        boss,
        true,
        100000.0,
        DamageKind::Default,
        None,
        PlainAttackOnDamage::None,
        &mut updates,
    );
    assert_eq!(rt.entities.get(boss).unwrap().runtime.hp, before);
}

#[test]
fn transformed_rumia_reflects_a_rumia_attack_only_once() {
    let config = default_custom_runtime_import_config().unwrap();
    let mut runner = RuntimeRunner::from_custom_mixed_namerena_raw("rumia@!\n\nrumia@!\n\nseed:2".into(), config).unwrap();
    let rt = runner.runtime_mut();
    let (attacker, target) = {
        let mut bosses = rt.entities.iter().map(|(idx, _)| idx);
        (bosses.next().unwrap(), bosses.next().unwrap())
    };
    rt.entities.get_mut(target).unwrap().runtime.hp = 100;
    let mut updates = RunUpdates::new();
    rt.upgrade_rumia_into(target, &mut updates);
    let before = rt.entities.get(attacker).unwrap().runtime.hp;
    rt.drain_plain_attack_with_atp_and_on_damage_into(
        attacker,
        target,
        false,
        1000.0,
        DamageKind::Default,
        PlainAttackOnDamage::None,
        &mut updates,
    );
    assert!(rt.entities.get(attacker).unwrap().runtime.hp < before);
    assert_eq!(updates.updates.iter().filter(|u| u.message.contains("黑雾")).count(), 1);
}

fn prepare_player(raw: &str) -> crate::namerena::PreparedPlayer {
    use crate::namerena::{NamerenaInput, PreparedRoster, eval_name::DEFAULT_EVAL_RQ};
    PreparedRoster::build(&NamerenaInput::parse(raw).unwrap(), DEFAULT_EVAL_RQ)
        .unwrap()
        .players
        .remove(0)
}

#[test]
fn canonical_rin_boss_accepts_the_signed_attack_and_magic() {
    let rin = prepare_player("rin@!");
    let template = PlayerTemplate::from_namerena_base(&rin, 1, 0, PlayerKindFlags::BOSS);
    assert_eq!((template.attack, template.magic), (-26, -26));
}

#[test]
fn signed_attack_and_magic_reject_other_identities_flags_and_values() {
    let rin = prepare_player("rin@!");
    let rejected = |player: &crate::namerena::PreparedPlayer, flags| {
        assert!(std::panic::catch_unwind(|| PlayerTemplate::from_namerena_base(player, 1, 0, flags)).is_err());
    };
    rejected(&rin, PlayerKindFlags::NONE);
    for raw in ["rin", "rin@other", "Rin@!"] {
        let mut ordinary = prepare_player(raw);
        let template = PlayerTemplate::from_namerena_base(&ordinary, 1, 0, PlayerKindFlags::NONE);
        assert!(template.attack >= 0 && template.magic >= 0, "{raw}");
        ordinary.status.attack = -26;
        ordinary.status.magic = -26;
        rejected(&ordinary, PlayerKindFlags::BOSS);
    }
    let mut wrong_value = rin;
    wrong_value.status.magic = -25;
    rejected(&wrong_value, PlayerKindFlags::BOSS);
    wrong_value.status.magic = -26;
    wrong_value.status.attack = -25;
    rejected(&wrong_value, PlayerKindFlags::BOSS);
}

#[test]
fn shared_runtime_state_does_not_take_over_an_ordinary_players_action() {
    let (mut runner, _, enemy) = battle("rin");
    let rt = runner.runtime_mut();
    rt.entities.get_mut(enemy).unwrap().runtime.rosmontic = Some(Box::default());
    assert!(!rt.has_rosmontic_action(enemy));
    assert!(!matches!(
        rt.prepare_plain_action(enemy, true, Default::default()),
        Some(PreparedPlainAction::BossSkill(_))
    ));
}

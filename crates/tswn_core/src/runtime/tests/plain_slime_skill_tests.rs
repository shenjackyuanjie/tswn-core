use super::*;

use crate::runtime::lang::{get_lang, keys};

fn slime_runtime() -> CombatRuntime {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let registry = config.registry;
    let boss_kind = registry
        .player_kind_id_by_export_name(DEFAULT_CORE_BOSS_KIND_EXPORT)
        .expect("default profile should register core boss kind");
    CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "slime", boss_kind, 0, 1_000, 3).with_magic(1_000_000),
            PlayerTemplate::new(2, "hero", 1, 100, 3),
        ],
        registry,
    ))
}

fn messages_of(updates: &RunUpdates) -> Vec<String> { updates.updates.iter().map(|update| update.message.to_string()).collect() }

fn kill(runtime: &mut CombatRuntime, target: EntityIdx) { runtime.entities.get_mut(target).unwrap().runtime.hp = 0; }

/// 0.5.5 `eV` 会给子体八围或上固定的位：第一代 16、第二代 32。
fn assert_slime_child_bit(child: &EntityRecord, bit: i32, max_hp: i32) {
    assert_eq!(child.template.max_hp, max_hp);
    assert_eq!(child.runtime.hp, max_hp);
    let attrs = [
        child.runtime.attack,
        child.runtime.defense,
        child.runtime.magic,
        child.runtime.resistance,
        child.runtime.agility,
        child.runtime.wisdom,
    ];
    for attr in attrs {
        assert_eq!(attr & bit, bit, "史莱姆子体属性必须带 eV 位: {attrs:?}");
        assert!((0..64).contains(&attr), "史莱姆子体属性必须来自名字底数: {attrs:?}");
    }
    assert_eq!(child.runtime.magic_point, child.runtime.wisdom >> 1);
    let speed_attr = child.runtime.speed - 160;
    assert_eq!(speed_attr & bit, bit, "史莱姆子体速度必须带 eV 位");
}

#[test]
fn slime_split_spawns_two_first_generation_children() {
    let mut runtime = slime_runtime();
    kill(&mut runtime, EntityIdx(0));
    let mut expected_rng = runtime.rng.clone();
    let first_speed = expected_rng.r255() as i32 * 4;
    let second_speed = expected_rng.r255() as i32 * 4;
    let mut updates = RunUpdates::new();

    assert!(runtime.drain_plain_slime_split_into(EntityIdx(0), &mut updates));

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(runtime.entities.len(), 4);

    let parent = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(parent.runtime.slime_split_generation, None);
    assert_eq!(parent.runtime.corpse, RuntimeCorpseKind::Slime);
    assert_eq!(parent.runtime.hp, 0);
    // 分裂发生在死亡链内，本体由外层流程标记死亡。
    assert!(parent.runtime.alive);

    for (idx, speed) in [(EntityIdx(2), first_speed), (EntityIdx(3), second_speed)] {
        let child = runtime.entities.get(idx).unwrap();
        assert_eq!(child.template.name, "slime");
        assert_eq!(child.template.clan_name, "slime");
        assert_eq!(child.template.display_name, get_lang(keys::BOSS_NAME_SLIME));
        assert_eq!(child.runtime.slime_split_generation, Some(1));
        assert_eq!(child.runtime.corpse, RuntimeCorpseKind::None);
        assert!(!child.runtime.flags.contains(PlayerKindFlags::BOSS));
        assert!(!child.runtime.is_combat_minion());
        assert!(child.template.skills.skills().is_empty());
        assert_eq!(child.runtime.move_state.speed_points, speed);
        assert_slime_child_bit(child, 16, 218);
    }
    assert_eq!(
        runtime.entities.get(EntityIdx(2)).unwrap().template.id_key_name,
        "slime?0@slime"
    );
    assert_eq!(
        runtime.entities.get(EntityIdx(3)).unwrap().template.id_key_name,
        "slime?1@slime"
    );

    assert_eq!(messages_of(&updates), ["\n", "[0][分裂]", "分成了[0] 和  [1]"]);
    assert_eq!(updates.updates[1].caster, 0);
    assert_eq!(updates.updates[1].target, 0);
    assert_eq!(updates.updates[1].score, 0);
    assert_eq!(updates.updates[2].caster, 2);
    assert_eq!(updates.updates[2].target, 3);
    assert_eq!(updates.updates[2].score, 0);
}

#[test]
fn slime_first_generation_splits_into_second_generation_with_half_and_heal() {
    let mut runtime = slime_runtime();
    kill(&mut runtime, EntityIdx(0));
    let mut updates = RunUpdates::new();
    assert!(runtime.drain_plain_slime_split_into(EntityIdx(0), &mut updates));

    kill(&mut runtime, EntityIdx(2));
    let mut expected_rng = runtime.rng.clone();
    let first_speed = expected_rng.r255() as i32 * 4;
    let second_speed = expected_rng.r255() as i32 * 4;
    let mut updates = RunUpdates::new();
    assert!(runtime.drain_plain_slime_split_into(EntityIdx(2), &mut updates));

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(runtime.entities.len(), 6);

    let parent = runtime.entities.get(EntityIdx(2)).unwrap();
    assert_eq!(parent.runtime.slime_split_generation, None);
    assert_eq!(parent.runtime.corpse, RuntimeCorpseKind::Slime);

    let half = runtime
        .registry
        .skill_id_by_export_name(BuiltinActiveSkill::Half.export_name())
        .expect("default profile should register core half skill");
    let heal = runtime
        .registry
        .skill_id_by_export_name(BuiltinActiveSkill::Heal.export_name())
        .expect("default profile should register core heal skill");
    for (idx, speed) in [(EntityIdx(4), first_speed), (EntityIdx(5), second_speed)] {
        let child = runtime.entities.get(idx).unwrap();
        assert_eq!(child.runtime.slime_split_generation, Some(2));
        assert_eq!(child.template.max_hp, 134);
        assert_eq!(child.runtime.move_state.speed_points, speed);
        assert_slime_child_bit(child, 32, 134);
        assert_eq!(child.template.skills.skills(), [half, heal]);
        assert_eq!(child.template.skills.level_at(0), Some(32));
        assert_eq!(child.template.skills.level_at(1), Some(32));
    }
    assert_eq!(
        runtime.entities.get(EntityIdx(4)).unwrap().template.id_key_name,
        "slime?2@slime"
    );
    assert_eq!(
        runtime.entities.get(EntityIdx(5)).unwrap().template.id_key_name,
        "slime?3@slime"
    );
}

#[test]
fn slime_second_generation_does_not_split_again() {
    let mut runtime = slime_runtime();
    kill(&mut runtime, EntityIdx(0));
    let mut updates = RunUpdates::new();
    assert!(runtime.drain_plain_slime_split_into(EntityIdx(0), &mut updates));
    kill(&mut runtime, EntityIdx(2));
    let mut updates = RunUpdates::new();
    assert!(runtime.drain_plain_slime_split_into(EntityIdx(2), &mut updates));

    kill(&mut runtime, EntityIdx(4));
    let rng_before = runtime.rng.clone();
    let mut updates = RunUpdates::new();

    assert!(!runtime.drain_plain_slime_split_into(EntityIdx(4), &mut updates));

    assert_rng_state_eq(&runtime.rng, &rng_before);
    assert_eq!(runtime.entities.len(), 6);
    let child = runtime.entities.get(EntityIdx(4)).unwrap();
    assert_eq!(child.runtime.slime_split_generation, Some(2));
    assert_eq!(child.runtime.corpse, RuntimeCorpseKind::None);
    assert!(updates.updates.is_empty());
}

#[test]
fn slime_lethal_chain_splits_after_death_message() {
    let mut runtime = slime_runtime();
    kill(&mut runtime, EntityIdx(0));
    let mut updates = RunUpdates::new();

    runtime.drain_plain_lethal_damage_into(EntityIdx(1), EntityIdx(0), &mut updates);

    assert_eq!(
        messages_of(&updates),
        ["\n", "[1]被击倒了", "\n", "[0][分裂]", "分成了[0] 和  [1]"]
    );
    let parent = runtime.entities.get(EntityIdx(0)).unwrap();
    assert!(!parent.runtime.alive);
    assert_eq!(parent.runtime.corpse, RuntimeCorpseKind::Slime);
    assert_eq!(parent.runtime.slime_split_generation, None);
    for idx in [EntityIdx(2), EntityIdx(3)] {
        let child = runtime.entities.get(idx).unwrap();
        assert!(child.runtime.alive);
        assert!(child.runtime.active());
        assert_eq!(child.runtime.team, 0);
    }
}

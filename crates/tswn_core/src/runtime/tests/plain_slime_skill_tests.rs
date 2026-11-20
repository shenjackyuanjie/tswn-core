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

/// 子体八围来自 `eV` 位改写后的名字底数，再按 nameDens 缩放（体力不缩放）。
fn assert_slime_child_scaled(child: &EntityRecord, generation: u32, max_hp: i32) {
    let (name_base, _) = PreparedBattleInit::plain_minion_name_base("slime", "slime");
    let attrs = crate::runtime::plain_slime::slime_child_attrs(&name_base, generation);
    let factor = crate::namerena::eval_name::eval_str_common_with_rq("slime", true, crate::namerena::eval_name::DEFAULT_EVAL_RQ)
        .max(
            crate::namerena::eval_name::eval_str_common_with_rq("slime", true, crate::namerena::eval_name::DEFAULT_EVAL_RQ) - 6.0,
        );
    let expected = PreparedBattleInit::plain_minion_status_with_factor(attrs, factor);
    assert_eq!(child.template.max_hp, max_hp);
    assert_eq!(child.runtime.hp, max_hp);
    assert_eq!(child.runtime.attack, expected.attack);
    assert_eq!(child.runtime.defense, expected.defense);
    assert_eq!(child.runtime.magic, expected.magic);
    assert_eq!(child.runtime.resistance, expected.resistance);
    assert_eq!(child.runtime.agility, expected.agility);
    assert_eq!(child.runtime.wisdom, expected.wisdom);
    assert_eq!(child.runtime.speed, expected.speed);
    assert_eq!(child.runtime.magic_point, expected.magic_point);
    assert_eq!(child.template.clone_build.as_ref().unwrap().attrs(), attrs);
}

const SLIME_MERGE_INPUT: &str = "slime@!\n\nlazy@!\nparity3a\nparity3b";

#[test]
fn slime_public_merge_keeps_the_original_battle_trajectory() {
    let mut runner = RuntimeRunner::new_from_namerena_raw(SLIME_MERGE_INPUT.to_owned()).unwrap();
    let mut batch = 0;
    for _ in 0..128 {
        let round = runner.run_round();
        if let Some(frame) = round.frame {
            batch += 1;
            let updates = &frame.updates.updates;
            match batch {
                38 => assert!(updates.iter().any(|event| event.message == "[0][吞噬]了[1]")),
                39 => assert!(updates.iter().any(|event| event.message.contains("点伤害") && event.score == 90)),
                41 => assert_eq!((runner.runtime().rng.i, runner.runtime().rng.j), (218, 19)),
                _ => {}
            }
        }
        if round.winner_team.is_some() {
            assert_eq!(batch, 45);
            assert!(runner.input_group_won(1));
            assert_eq!((runner.runtime().rng.i, runner.runtime().rng.j), (61, 27));
            return;
        }
    }
    panic!("对局应在预算内结束");
}

#[test]
fn slime_merge_takes_raw_attrs_and_skill_levels_by_position() {
    let mut runner = RuntimeRunner::new_from_namerena_raw(SLIME_MERGE_INPUT.to_owned()).unwrap();
    let runtime = runner.runtime_mut();
    let find =
        |runtime: &CombatRuntime, name: &str| runtime.entities.iter().find(|(_, p)| p.template.id_key_name == name).unwrap().0;
    let caster = find(runtime, "parity3a");
    let boss = find(runtime, "slime@!");
    kill(runtime, boss);
    assert!(runtime.drain_plain_slime_split_into(boss, &mut RunUpdates::new()));
    let first = find(runtime, "slime?0@!");
    kill(runtime, first);
    assert!(runtime.drain_plain_slime_split_into(first, &mut RunUpdates::new()));
    let second = find(runtime, "slime?2@!");
    let same = find(runtime, "slime?3@!");

    // 原版第一代分裂技能为 0 级；第二代的半血/治疗会分别抬高吞噬者的第 0/1 槽，
    // 即使吞噬者这两个槽的技能类型不同。第三次只有资源转移，不产生吞噬战报。
    let rng_before = runtime.rng.clone();
    for (target, attrs, levels, changed) in [
        (first, [49, 53, 48, 30, 22, 47, 50, 354], [0, 0, 0], true),
        (second, [49, 53, 48, 46, 38, 52, 50, 354], [32, 32, 0], true),
        (same, [49, 53, 48, 46, 38, 52, 50, 354], [32, 32, 0], false),
    ] {
        let owner = &mut runtime.entities.get_mut(caster).unwrap().runtime;
        owner.magic_point = 5;
        owner.move_state.speed_points = 100;
        let source = &mut runtime.entities.get_mut(target).unwrap().runtime;
        source.hp = 0;
        source.magic_point = 58;
        source.move_state.speed_points = 1000;
        let mut updates = RunUpdates::new();
        assert_eq!(runtime.apply_plain_merge_into(caster, target, &mut updates), changed);

        let owner = runtime.entities.get(caster).unwrap();
        assert_eq!(owner.template.clone_build.as_ref().unwrap().attrs(), attrs);
        assert_eq!((owner.runtime.hp, owner.runtime.attack, owner.runtime.speed), (354, 49, 208));
        let skills = &owner.template.skills;
        assert_eq!(
            skills.merge_lane_order()[..3]
                .iter()
                .map(|&lane| skills.level_at(lane).unwrap())
                .collect::<Vec<_>>(),
            levels
        );
        assert_eq!((owner.runtime.magic_point, owner.runtime.move_state.speed_points), (58, 1100));
        let source = &runtime.entities.get(target).unwrap().runtime;
        assert_eq!((source.magic_point, source.move_state.speed_points), (0, 0));
        if changed {
            assert_eq!(source.corpse, RuntimeCorpseKind::Merge);
            assert_eq!(messages_of(&updates), ["\n", "[0][吞噬]了[1]", "[0]属性上升"]);
        } else {
            assert_eq!(source.corpse, RuntimeCorpseKind::None);
            assert!(updates.updates.is_empty());
        }
    }
    assert_rng_state_eq(&runtime.rng, &rng_before);
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
    assert_eq!(parent.runtime.slime_split_generation, Some(0));
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
        assert_slime_child_scaled(child, 1, 218);
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
    assert_eq!(parent.runtime.slime_split_generation, Some(1));
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
        assert_slime_child_scaled(child, 2, 134);
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
    assert_eq!(parent.runtime.slime_split_generation, Some(0));
    for idx in [EntityIdx(2), EntityIdx(3)] {
        let child = runtime.entities.get(idx).unwrap();
        assert!(child.runtime.alive);
        assert!(child.runtime.active());
        assert_eq!(child.runtime.team, 0);
    }
}

//! 史莱姆分裂/合并：公开对局轨迹、分裂代数封顶、护身符复活链与确定性。
use super::*;

use crate::runtime::lang::{get_lang, keys};

/// 用默认 profile 构造史莱姆 boss 对英雄的 CombatRuntime，用于史莱姆技能测试。
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

/// 提取 RunUpdates 中所有消息字符串。
fn messages_of(updates: &RunUpdates) -> Vec<String> { updates.updates.iter().map(|update| update.message.to_string()).collect() }

/// 把目标实体 hp 置 0（致死），用于触发死亡/分裂路径。
fn kill(runtime: &mut CombatRuntime, target: EntityIdx) { runtime.entities.get_mut(target).unwrap().runtime.hp = 0; }

/// 子体八围来自 `eV` 位改写后的名字底数，再按 nameDens 缩放（体力不缩放）。
fn assert_slime_child_scaled(child: &EntityRecord, generation: u32, max_hp: i32) {
    let (name_base, _) = PreparedBattleInit::plain_minion_name_base("slime", "slime");
    let attrs = crate::runtime::slime::slime_child_attrs(&name_base, generation);
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

/// 史莱姆公开合并的 namerena 输入样本。
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

/// 带护身符（reraise，level 128 必成功）的史莱姆 Boss，用于复活链。
fn slime_runtime_with_reraise() -> CombatRuntime {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let registry = config.registry;
    let boss_kind = registry
        .player_kind_id_by_export_name(DEFAULT_CORE_BOSS_KIND_EXPORT)
        .expect("default profile should register core boss kind");
    let reraise = registry
        .skill_id_by_export_name(DEFAULT_CORE_RERAISE_SKILL_EXPORT)
        .expect("default profile should register core reraise");
    CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "slime", boss_kind, 0, 1_000, 3)
                .with_magic(1_000_000)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(reraise, 128)])),
            PlayerTemplate::new(2, "hero", 1, 100, 3),
        ],
        registry,
    ))
}

/// 收集指定分裂代数的子体实体索引。
fn children_of(runtime: &CombatRuntime, generation: u32) -> Vec<EntityIdx> {
    runtime
        .entities
        .iter()
        .filter(|(_, entity)| entity.runtime.slime_split_generation == Some(generation))
        .map(|(idx, _)| idx)
        .collect()
}

/// 分裂产生的实体状态快照，用于比较两次完全相同输入的运行结果。
fn snapshot(runtime: &CombatRuntime) -> Vec<String> {
    runtime
        .entities
        .iter()
        .map(|(idx, entity)| {
            let runtime = &entity.runtime;
            format!(
                "{idx:?}|name={}|clan={}|disp={}|id={}|max_hp={}|hp={}|alive={}|gen={:?}|corpse={:?}|team={}|owner={}|root={}|atk={}|def={}|mag={}|res={}|agl={}|wis={}|spd={}|mp={}|sp={}|skills={:?}",
                entity.template.name,
                entity.template.clan_name,
                entity.template.display_name,
                entity.template.id_key_name,
                entity.template.max_hp,
                runtime.hp,
                runtime.alive,
                runtime.slime_split_generation,
                runtime.corpse,
                runtime.team,
                runtime.owner.0,
                runtime.root_owner.0,
                runtime.attack,
                runtime.defense,
                runtime.magic,
                runtime.resistance,
                runtime.agility,
                runtime.wisdom,
                runtime.speed,
                runtime.magic_point,
                runtime.move_state.speed_points,
                entity.template.skills.skills(),
            )
        })
        .collect()
}

#[test]
fn thanos_snap_interleaves_slime_split_with_the_next_draw() {
    let registry = default_custom_runtime_import_config().unwrap().registry;
    let boss_kind = registry.player_kind_id_by_export_name(DEFAULT_CORE_BOSS_KIND_EXPORT).unwrap();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "thanos", boss_kind, 0, 1000, 3),
            PlayerTemplate::with_kind(2, "slime", boss_kind, 1, 1000, 3),
            PlayerTemplate::new(3, "enemy-a", 1, 100, 3),
            PlayerTemplate::new(4, "enemy-b", 1, 100, 3),
            PlayerTemplate::new(5, "enemy-c", 1, 100, 3),
        ],
        registry,
    ));
    let (thanos, slime) = (EntityIdx(0), EntityIdx(1));
    let enemy_pool: Vec<_> = runtime.world.flat_alive().iter().copied().filter(|&id| id != thanos).collect();
    let mut reference = runtime.rng.clone();
    let mut immediate_second = None;
    let mut expected_second = None;
    // 找一个“首抽史莱姆，且分裂确实改变次抽”的初始随机位置。
    for _ in 0..4096 {
        reference = runtime.rng.clone();
        let mut pool = enemy_pool.clone();
        let first = pool.remove(reference.next_i32(pool.len() as i32) as usize);
        if first == slime {
            let early = reference.clone().next_i32(pool.len() as i32) as usize;
            reference.r255();
            reference.r255();
            let after_split = reference.next_i32(pool.len() as i32) as usize;
            if early != after_split {
                immediate_second = Some(pool[early]);
                expected_second = Some(pool[after_split]);
                break;
            }
        }
        runtime.rng.next_u8();
    }
    let expected_second = expected_second.expect("必须找到分裂会改变次抽的随机位置");
    let targets = runtime.select_plain_thanos_snap_targets(thanos).expect("敌方四人应触发响指");
    let mut updates = RunUpdates::new();
    runtime.drain_plain_thanos_snap_into(thanos, targets, &mut updates);

    let victims: Vec<_> = updates
        .updates
        .iter()
        .filter(|update| update.message == get_lang(keys::SKL_THANOS_DUST))
        .map(|update| EntityIdx(update.target as u32))
        .collect();
    assert_eq!(victims, [slime, expected_second]);
    assert!(runtime.entities.get(immediate_second.unwrap()).unwrap().runtime.alive);
    let children = children_of(&runtime, 1);
    assert_eq!(children.len(), 2);
    assert!(children.iter().all(|child| runtime.world.flat_alive().contains(child)));
    assert!(!runtime.world.flat_alive().contains(&slime));
    assert_eq!(runtime.world.alive_group_count(), 2);
    assert_rng_state_eq(&runtime.rng, &reference);
}

/// 分裂只到第二代：0 代 Boss → 1 代子体 → 2 代孙体，之后任何死亡都不再产生新实体。
#[test]
fn slime_split_recursion_depth_is_capped_at_two_generations() {
    let mut runtime = slime_runtime();
    kill(&mut runtime, EntityIdx(0));
    let mut updates = RunUpdates::new();
    assert!(runtime.drain_plain_slime_split_into(EntityIdx(0), &mut updates));
    assert_eq!(children_of(&runtime, 1), [EntityIdx(2), EntityIdx(3)]);

    // 1 代子体各自再分裂一次：这里逐个分裂，保证递归是逐层展开而不是一次铺开。
    for (parent, expected_children) in [
        (EntityIdx(2), [EntityIdx(4), EntityIdx(5)]),
        (EntityIdx(3), [EntityIdx(6), EntityIdx(7)]),
    ] {
        kill(&mut runtime, parent);
        let mut updates = RunUpdates::new();
        assert!(
            runtime.drain_plain_slime_split_into(parent, &mut updates),
            "第一代子体必须还能分裂一次"
        );
        let generation_2 = children_of(&runtime, 2);
        assert_eq!(
            &generation_2[generation_2.len() - 2..],
            expected_children,
            "子体命名与索引必须按分裂顺序递增"
        );
        assert_eq!(messages_of(&updates), ["\n", "[0][分裂]", "分成了[0] 和  [1]"]);
    }
    assert_eq!(
        children_of(&runtime, 2),
        [EntityIdx(4), EntityIdx(5), EntityIdx(6), EntityIdx(7)]
    );
    assert_eq!(runtime.entities.len(), 8, "0 代 1 只 + 1 代 2 只 + 2 代 4 只 + 路人 1 只");

    // 第二代全体死亡也只走普通死亡链：不再生成实体、不再消费随机数、不再改尸体标记。
    let rng_before = runtime.rng.clone();
    for child in children_of(&runtime, 2) {
        kill(&mut runtime, child);
        let mut updates = RunUpdates::new();
        assert!(
            !runtime.drain_plain_slime_split_into(child, &mut updates),
            "第二代子体不得再分裂"
        );
        assert!(updates.updates.is_empty());
        let entity = runtime.entities.get(child).unwrap();
        assert_eq!(entity.runtime.corpse, RuntimeCorpseKind::None);
        assert_eq!(entity.runtime.slime_split_generation, Some(2));
    }
    assert_rng_state_eq(&runtime.rng, &rng_before);
    assert_eq!(runtime.entities.len(), 8);
}

/// 子体死亡后再次分裂走真实死亡链：尸体标记随分裂写入，孙体带半血与治疗。
#[test]
fn slime_child_lethal_death_resplits_and_grandchildren_stop() {
    let mut runtime = slime_runtime();
    kill(&mut runtime, EntityIdx(0));
    assert!(runtime.drain_plain_slime_split_into(EntityIdx(0), &mut RunUpdates::new()));

    kill(&mut runtime, EntityIdx(2));
    let mut updates = RunUpdates::new();
    runtime.drain_plain_lethal_damage_into(EntityIdx(1), EntityIdx(2), &mut updates);
    assert_eq!(
        messages_of(&updates),
        ["\n", "[1]被击倒了", "\n", "[0][分裂]", "分成了[0] 和  [1]"]
    );
    let parent = runtime.entities.get(EntityIdx(2)).unwrap();
    assert!(!parent.runtime.alive);
    assert_eq!(parent.runtime.corpse, RuntimeCorpseKind::Slime);
    assert_eq!(parent.runtime.slime_split_generation, Some(1));

    let half = runtime
        .registry
        .skill_id_by_export_name(BuiltinActiveSkill::Half.export_name())
        .expect("default profile should register core half skill");
    let heal = runtime
        .registry
        .skill_id_by_export_name(BuiltinActiveSkill::Heal.export_name())
        .expect("default profile should register core heal skill");
    for child in [EntityIdx(4), EntityIdx(5)] {
        let entity = runtime.entities.get(child).unwrap();
        assert_eq!(entity.runtime.slime_split_generation, Some(2));
        assert_eq!(entity.template.skills.skills(), [half, heal]);
        assert_eq!(entity.template.skills.level_at(0), Some(32));
        assert_eq!(entity.template.skills.level_at(1), Some(32));
        assert_eq!(entity.template.max_hp, 134);
        assert!(entity.runtime.alive && entity.runtime.active());
    }

    // 孙体走完整死亡链后只是普通尸体，不会再分裂。
    kill(&mut runtime, EntityIdx(4));
    let mut updates = RunUpdates::new();
    runtime.drain_plain_lethal_damage_into(EntityIdx(1), EntityIdx(4), &mut updates);
    assert_eq!(messages_of(&updates), ["\n", "[1]被击倒了"]);
    let grandchild = runtime.entities.get(EntityIdx(4)).unwrap();
    assert!(!grandchild.runtime.alive);
    assert_eq!(grandchild.runtime.corpse, RuntimeCorpseKind::None);
    assert_eq!(grandchild.runtime.slime_split_generation, Some(2));
    assert_eq!(runtime.entities.len(), 6);
}

/// 分裂只对真正死亡的史莱姆实体生效：活着的本体与别的实体会被直接跳过。
#[test]
fn slime_split_ignores_live_and_foreign_entities() {
    let mut runtime = slime_runtime();
    let rng_before = runtime.rng.clone();
    let mut updates = RunUpdates::new();

    // 本体还活着（包括被复活回血）时不分裂。
    assert!(!runtime.drain_plain_slime_split_into(EntityIdx(0), &mut updates));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 7;
    assert!(!runtime.drain_plain_slime_split_into(EntityIdx(0), &mut updates));
    // 非史莱姆实体与不存在的索引都不分裂。
    assert!(!runtime.drain_plain_slime_split_into(EntityIdx(1), &mut updates));
    assert!(!runtime.drain_plain_slime_split_into(EntityIdx(99), &mut updates));

    assert_eq!(runtime.entities.len(), 2);
    assert!(updates.updates.is_empty());
    assert_rng_state_eq(&runtime.rng, &rng_before);
    let boss = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(boss.runtime.slime_split_generation, Some(0));
    assert_eq!(boss.runtime.corpse, RuntimeCorpseKind::None);

    // 复活后的血量归零才重新具备分裂资格。
    kill(&mut runtime, EntityIdx(0));
    assert!(runtime.drain_plain_slime_split_into(EntityIdx(0), &mut updates));
    assert_eq!(runtime.entities.len(), 4);
}

#[test]
fn slime_public_duel_uses_second_generation_skill_rng() {
    let config = default_custom_runtime_import_config().unwrap();
    let mut runner = RuntimeRunner::from_custom_mixed_namerena_raw("slime@!\n\nmario@!".to_owned(), config).unwrap();
    let mut seen = 0;
    for _ in 0..256 {
        let Some(frame) = runner.run_round().frame else {
            continue;
        };
        seen += 1;
        let (caster, damage, expected_rng) = match seen {
            // 原版第 16 批是二代子体跳过两项主动技能后的普攻，仍消耗技能概率随机数。
            16 => ("slime?2@!", 61, (174, 137)),
            17 => ("slime?0@!", 59, (204, 222)),
            _ => continue,
        };
        let runtime = runner.runtime();
        let attack = &frame.updates.updates[0];
        assert_eq!(
            runtime.entities.get(EntityIdx(attack.caster as u32)).unwrap().template.id_key_name,
            caster
        );
        assert_eq!(frame.updates.updates[1].score, damage);
        assert_eq!((runtime.rng.i, runtime.rng.j), expected_rng, "第 {seen} 个可见批次");
        if seen == 17 {
            break;
        }
    }
    assert_eq!(seen, 17);
}

/// 奇数伤害由使魔向下取整分担；正好致死时先分裂，再由护身符复活。
#[test]
fn slime_summon_shared_damage_splits_before_reraise() {
    let config = default_custom_runtime_import_config().unwrap();
    let mut runner = RuntimeRunner::from_custom_mixed_namerena_raw("slime@!\n\nhero".to_owned(), config).unwrap();
    let runtime = runner.runtime_mut();
    let (slime, hero) = (EntityIdx(0), EntityIdx(1));
    let reraise = runtime.registry.skill_id_by_export_name(DEFAULT_CORE_RERAISE_SKILL_EXPORT).unwrap();
    runtime.entities.get_mut(slime).unwrap().template.skills = SkillLoadout::from_skill_levels([(reraise, 128)]);
    runtime.drain_plain_summon_skill_into(slime, &mut RunUpdates::new());
    let (summon, _) = runtime
        .entities
        .iter()
        .find(|(_, entity)| entity.runtime.owner == slime && entity.runtime.flags.contains(PlayerKindFlags::SUMMON))
        .expect("史莱姆的使魔应进入战场");
    runtime.entities.get_mut(slime).unwrap().runtime.hp = 5;
    runtime.entities.get_mut(summon).unwrap().runtime.hp = 20;
    let before_order = runtime.world.round_order().to_vec();
    let mut reference = runtime.rng.clone();
    let child_speed = [reference.r255() as i32 * 4, reference.r255() as i32 * 4];
    reference.r127(); // 护身符成功骰在两个子体之后消耗。
    let revive_hp = reference.r16() as i32;

    let mut updates = RunUpdates::new();
    assert!(!runtime.apply_plain_attack_damage_into(hero, summon, 11, &mut updates));

    assert_eq!((updates.updates[0].target, updates.updates[0].score), (summon.0 as usize, 11));
    assert_eq!((updates.updates[1].target, updates.updates[1].score), (slime.0 as usize, 5));
    let messages = messages_of(&updates);
    assert_eq!(
        &messages[2..8],
        [
            "\n",
            "[1]被击倒了",
            "\n",
            get_lang(keys::SKL_SLIME_SPAWN),
            get_lang(keys::SKL_SLIME_SPAWNED),
            "[0]使用[护身符]抵挡了一次死亡",
        ]
    );
    let boss = runtime.entities.get(slime).unwrap();
    assert!(boss.runtime.alive);
    assert_eq!((boss.runtime.hp, revive_hp), (11, 11));
    assert_eq!(boss.runtime.corpse, RuntimeCorpseKind::Slime);
    let summon = &runtime.entities.get(summon).unwrap().runtime;
    assert!(summon.alive);
    assert_eq!(summon.hp, 9);
    let children = children_of(runtime, 1);
    assert_eq!(children.len(), 2);
    assert_eq!(child_speed, [192, 276]);
    for (&child, speed) in children.iter().zip(child_speed) {
        let child = &runtime.entities.get(child).unwrap().runtime;
        assert_eq!((child.hp, child.move_state.speed_points), (218, speed));
    }
    assert_eq!(runtime.world.round_order(), [&before_order[..], &children].concat());
    assert_eq!(runtime.world.alive_group_count(), 2);
    assert_rng_state_eq(&runtime.rng, &reference);
}

/// 护身符复活不会摘掉分裂钩子：第二次死亡会再分裂一对。
#[test]
fn slime_revived_boss_splits_again_after_second_death() {
    let mut runtime = slime_runtime_with_reraise();
    kill(&mut runtime, EntityIdx(0));
    let mut updates = RunUpdates::new();
    runtime.drain_plain_lethal_damage_into(EntityIdx(1), EntityIdx(0), &mut updates);

    // 分裂钩子优先级 0，先于护身符（10）；复活中止死亡链但不回收刚生成的子体。
    let messages = messages_of(&updates);
    assert!(messages.contains(&"[0][分裂]".to_owned()), "分裂必须先于复活发生: {messages:?}");
    assert!(
        messages.contains(&"[0]使用[护身符]抵挡了一次死亡".to_owned()),
        "护身符必须在本体死亡链里生效: {messages:?}"
    );
    assert_eq!(children_of(&runtime, 1).len(), 2);
    let boss = runtime.entities.get(EntityIdx(0)).unwrap();
    assert!(boss.runtime.alive, "护身符复活后本体必须存活");
    assert!(boss.runtime.hp > 0);
    assert_eq!(boss.runtime.slime_split_generation, Some(0), "复活不消耗分裂机会");
    assert_eq!(boss.runtime.corpse, RuntimeCorpseKind::Slime);

    // 第二次死亡：钩子仍在，再分裂一对；代数仍由子体承担。
    kill(&mut runtime, EntityIdx(0));
    let mut updates = RunUpdates::new();
    runtime.drain_plain_lethal_damage_into(EntityIdx(1), EntityIdx(0), &mut updates);
    let messages = messages_of(&updates);
    assert!(messages.contains(&"[0][分裂]".to_owned()), "复活后再死必须再分裂: {messages:?}");
    assert_eq!(runtime.entities.len(), 6);
    assert_eq!(children_of(&runtime, 1).len(), 4);
    assert_eq!(children_of(&runtime, 2).len(), 0);
}

/// 同一串输入跑两遍必须得到逐位一致的实体状态、消息与随机流。
#[test]
fn slime_split_is_deterministic_for_identical_runs() {
    fn run() -> (Vec<String>, Vec<String>, RC4, Vec<String>) {
        let mut runtime = slime_runtime();
        // 3 次分裂共 6 只子体，按生成顺序各吃 1 次 r255；独立推进一份随机流做对照。
        let mut expected_rng = runtime.rng.clone();
        let expected_speeds: Vec<i32> = (0..6).map(|_| expected_rng.r255() as i32 * 4).collect();
        let mut messages = Vec::new();
        kill(&mut runtime, EntityIdx(0));
        let mut updates = RunUpdates::new();
        assert!(runtime.drain_plain_slime_split_into(EntityIdx(0), &mut updates));
        messages.extend(messages_of(&updates));
        kill(&mut runtime, EntityIdx(2));
        let mut updates = RunUpdates::new();
        runtime.drain_plain_lethal_damage_into(EntityIdx(1), EntityIdx(2), &mut updates);
        messages.extend(messages_of(&updates));
        kill(&mut runtime, EntityIdx(3));
        let mut updates = RunUpdates::new();
        runtime.drain_plain_slime_split_into(EntityIdx(3), &mut updates);
        messages.extend(messages_of(&updates));

        // 子体速度点必须按生成顺序消耗 r255，且分裂结束后随机流停在同一位置。
        let speeds: Vec<i32> = (2..=7)
            .map(|idx| runtime.entities.get(EntityIdx(idx)).unwrap().runtime.move_state.speed_points)
            .collect();
        assert_eq!(speeds, expected_speeds, "子体速度点必须按生成顺序取 r255 * 4");
        assert_rng_state_eq(&runtime.rng, &expected_rng);
        (
            messages,
            snapshot(&runtime),
            runtime.rng.clone(),
            children_of(&runtime, 2).iter().map(|idx| format!("{idx:?}")).collect(),
        )
    }

    let (messages_a, snapshot_a, rng_a, ids_a) = run();
    let (messages_b, snapshot_b, rng_b, ids_b) = run();
    assert_eq!(messages_a, messages_b);
    assert_eq!(snapshot_a, snapshot_b);
    assert_eq!(ids_a, ids_b);
    assert_rng_state_eq(&rng_b, &rng_a);
    assert_eq!(snapshot_a.len(), 8, "0 代 + 路人 + 6 只子体");
}

/// 大量子体：1000 次分裂后实体数量线性增长、命名唯一、耗时在预算内。
#[test]
fn slime_mass_split_stays_within_time_budget() {
    const ROUNDS: usize = 1_000;
    let mut runtime = slime_runtime();
    let started = std::time::Instant::now();
    for round in 0..ROUNDS {
        let boss = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        boss.runtime.hp = 0;
        boss.runtime.slime_split_generation = Some(0);
        let mut updates = RunUpdates::new();
        assert!(
            runtime.drain_plain_slime_split_into(EntityIdx(0), &mut updates),
            "第 {round} 轮分裂失败"
        );
    }
    let elapsed = started.elapsed();
    println!("{ROUNDS} 次分裂耗时 {elapsed:?}，实体数 {}", runtime.entities.len());

    assert_eq!(runtime.entities.len(), 2 + ROUNDS * 2);
    let generation_1 = children_of(&runtime, 1);
    assert_eq!(generation_1.len(), ROUNDS * 2);
    assert_eq!(
        runtime.entities.get(generation_1[0]).unwrap().template.id_key_name,
        "slime?0@slime"
    );
    assert_eq!(
        runtime.entities.get(*generation_1.last().unwrap()).unwrap().template.id_key_name,
        format!("slime?{}@slime", ROUNDS * 2 - 1)
    );
    let mut id_names: Vec<&str> = generation_1
        .iter()
        .map(|idx| runtime.entities.get(*idx).unwrap().template.id_key_name.as_str())
        .collect();
    id_names.sort_unstable();
    let unique = {
        let mut cloned = id_names.clone();
        cloned.dedup();
        cloned.len()
    };
    assert_eq!(unique, id_names.len(), "子体 id 名必须互不重复");
    for child in [generation_1[0], *generation_1.last().unwrap()] {
        let entity = runtime.entities.get(child).unwrap();
        assert_eq!(entity.template.max_hp, 218);
        assert_eq!(entity.runtime.hp, 218);
        assert_eq!(get_lang(keys::BOSS_NAME_SLIME), entity.template.display_name);
    }
    assert!(
        elapsed < std::time::Duration::from_secs(10),
        "1000 次分裂耗时 {elapsed:?}，超出预算"
    );
}

/// 子体是普通参战者：不随本体死亡清理，并保留 root owner 指向原始史莱姆。
#[test]
fn slime_children_survive_boss_death_with_owner_links() {
    let mut runtime = slime_runtime();
    kill(&mut runtime, EntityIdx(0));
    let mut updates = RunUpdates::new();
    runtime.drain_plain_lethal_damage_into(EntityIdx(1), EntityIdx(0), &mut updates);

    let boss = runtime.entities.get(EntityIdx(0)).unwrap();
    assert!(!boss.runtime.alive);
    for child in [EntityIdx(2), EntityIdx(3)] {
        let entity = runtime.entities.get(child).unwrap();
        assert!(entity.runtime.alive && entity.runtime.active(), "本体死亡不得清理子体");
        assert!(!entity.runtime.is_combat_minion());
        assert_eq!(entity.runtime.team, boss.runtime.team);
        assert_eq!(entity.runtime.owner, child);
        assert_eq!(entity.runtime.root_owner, EntityIdx(0));
        assert_eq!(entity.runtime.corpse, RuntimeCorpseKind::None);
    }
    // 子体不进入死亡清理链：再走一次本体死亡不会改动它们（钩子常驻，因此会再分裂一对）。
    runtime.drain_die_hooks_into(EntityIdx(0), &mut RunUpdates::new());
    for child in [EntityIdx(2), EntityIdx(3)] {
        assert!(runtime.entities.get(child).unwrap().runtime.alive);
    }
    assert_eq!(runtime.entities.len(), 6);
}

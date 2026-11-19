use super::*;

use crate::runtime::lang::{get_lang, keys};

/// 与 `plain_slime_skill_tests` 相同的史莱姆 Boss 战场。
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

fn messages_of(updates: &RunUpdates) -> Vec<String> { updates.updates.iter().map(|update| update.message.to_string()).collect() }

fn kill(runtime: &mut CombatRuntime, target: EntityIdx) { runtime.entities.get_mut(target).unwrap().runtime.hp = 0; }

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
        let (half_level, heal_level) = crate::runtime::tests::plain_slime_skill_tests::expected_slime_child_skill_levels();
        assert_eq!(entity.template.skills.level_at(0), Some(half_level));
        assert_eq!(entity.template.skills.level_at(1), Some(heal_level));
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

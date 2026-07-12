use super::*;

#[test]
fn custom_runner_fixture_matches_strict_diff_golden() {
    let mut builder = ExtensionRegistryBuilder::default();
    let owner_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "runner-owner",
            "custom.runner_owner",
            PlayerKindFlags::BED2,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToSummons,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("runner owner kind should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "runner-summon",
            "custom.runner_summon",
            PlayerKindFlags::MINION | PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: true,
            },
        )
        .expect("runner summon kind should register");
    let hp_marker = builder
        .reserve_entity_slot("custom", "hp-marker", "custom.hp_marker")
        .expect("hp marker slot should reserve");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 20, 3).with_def_res(77, 88),
            PlayerTemplate::new(2, "healer", 0, 10, 1),
            PlayerTemplate::new(3, "enemy", 1, 10, 1),
        ],
        registry,
    ));

    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(4, "summon", summon_kind, 0, 10, 1).with_def_res(11, 22),
    });
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(2),
        target: EntityIdx(0),
        amount: 4,
    });
    runtime.effects.push(QueuedEffect::Heal {
        caster: EntityIdx(1),
        target: EntityIdx(3),
        amount: 2,
    });
    runtime.effects.push(QueuedEffect::Replay {
        caster: EntityIdx(0),
        target: EntityIdx(0),
        message: "[0]还剩[2]点血".to_owned(),
        score: 87,
    });

    let frame = runtime.flush_effects().expect("custom runner fixture should emit updates");
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .slots
        .set(hp_marker, SlotValue::Bool(true))
        .expect("hp marker slot should write");
    let outcome = RoundOutcome {
        action: None,
        frame: Some(frame),
        winner_team: runtime.world.sync_winner(&runtime.entities),
    };
    let actual = NormalizedOutcome::from_runtime(&runtime, &outcome);
    let expected = NormalizedOutcome {
        winner_team: None,
        round: 0,
        total_score: 97,
        rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::default(),
        entity_ids: vec![1, 2, 3, 4],
        teams: vec![0, 0, 1, 0],
        hp: vec![16, 10, 10, 8],
        magic_point: vec![0, 0, 0, 0],
        defense: vec![77, 0, 0, 77],
        resistance: vec![88, 0, 0, 88],
        alive: vec![true, true, true, true],
        round_order: vec![0, 1, 2, 3],
        flat_alive: vec![0, 1, 3, 2],
        team_alive: vec![vec![0, 1, 3], vec![2]],
        alive_group_count: 2,
        actions: Vec::new(),
        frames: vec![
            NormalizedUpdateFrame {
                message: "出现一个新的[1]".to_owned(),
                caster: 0,
                target: 3,
                targets: Vec::new(),
                param: None,
                score: 0,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
            NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 2,
                target: 0,
                targets: Vec::new(),
                param: None,
                score: 4,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
            NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 2,
                target: 3,
                targets: Vec::new(),
                param: None,
                score: 4,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
            NormalizedUpdateFrame {
                message: "[1]回复体力[2]点".to_owned(),
                caster: 1,
                target: 3,
                targets: Vec::new(),
                param: None,
                score: 2,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
            NormalizedUpdateFrame {
                message: "[0]还剩[2]点血".to_owned(),
                caster: 0,
                target: 0,
                targets: Vec::new(),
                param: None,
                score: 87,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
        ],
    };

    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(hp_marker),
        Some(&SlotValue::Bool(true))
    );
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().template.defense, 77);
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().template.resistance, 88);
    assert_eq!(strict_diff(&expected, &actual), Ok(()));
}

#[test]
fn custom_runner_minion_owner_death_matches_strict_diff_golden() {
    let mut builder = ExtensionRegistryBuilder::default();
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "runner-linked-minion",
            "custom.runner_linked_minion",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION | PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("runner minion kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    ));

    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "owner?0", minion_kind, 0, 4, 1),
    });
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(4, "owner?1", minion_kind, 0, 4, 1),
    });
    runtime.flush_effects().expect("minion spawns should emit updates");
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(0),
        amount: 10,
    });

    let frame = runtime.flush_effects().expect("owner death should cleanup linked minions");
    let outcome = RoundOutcome {
        action: None,
        frame: Some(frame),
        winner_team: runtime.world.sync_winner(&runtime.entities),
    };
    let actual = NormalizedOutcome::from_runtime(&runtime, &outcome);
    let expected = NormalizedOutcome {
        winner_team: Some(1),
        round: 0,
        total_score: 110,
        rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::default(),
        entity_ids: vec![1, 2, 3, 4],
        teams: vec![0, 1, 0, 0],
        hp: vec![0, 10, 0, 0],
        magic_point: vec![0, 0, 0, 0],
        defense: vec![0, 0, 0, 0],
        resistance: vec![0, 0, 0, 0],
        alive: vec![false, true, false, false],
        round_order: vec![1],
        flat_alive: vec![1],
        team_alive: vec![Vec::new(), vec![1]],
        alive_group_count: 1,
        actions: Vec::new(),
        frames: vec![
            NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 1,
                target: 0,
                targets: Vec::new(),
                param: None,
                score: 10,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
            NormalizedUpdateFrame {
                message: "\n".to_owned(),
                caster: 0,
                target: 0,
                targets: Vec::new(),
                param: None,
                score: 0,
                delay0: 0,
                delay1: 0,
                update_type: crate::engine::update::UpdateType::NextLine,
            },
            NormalizedUpdateFrame {
                message: "[1]消失了".to_owned(),
                caster: 0,
                target: 2,
                targets: Vec::new(),
                param: None,
                score: 50,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
            NormalizedUpdateFrame {
                message: "\n".to_owned(),
                caster: 0,
                target: 0,
                targets: Vec::new(),
                param: None,
                score: 0,
                delay0: 0,
                delay1: 0,
                update_type: crate::engine::update::UpdateType::NextLine,
            },
            NormalizedUpdateFrame {
                message: "[1]消失了".to_owned(),
                caster: 0,
                target: 3,
                targets: Vec::new(),
                param: None,
                score: 50,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
        ],
    };

    assert_eq!(strict_diff(&expected, &actual), Ok(()));
}

#[test]
fn custom_runner_merge_matches_strict_diff_golden() {
    let mut builder = ExtensionRegistryBuilder::default();
    let skill_a = builder
        .register_skill("custom", "runner-a", "custom.runner_a", TargetPolicy::Enemy, SkillPriority(0))
        .expect("runner skill should register");
    let skill_b = builder
        .register_skill("custom", "runner-b", "custom.runner_b", TargetPolicy::Enemy, SkillPriority(1))
        .expect("runner skill should register");
    let skill_c = builder
        .register_skill("custom", "runner-c", "custom.runner_c", TargetPolicy::Enemy, SkillPriority(2))
        .expect("runner skill should register");
    let merge_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "runner-merge",
            "custom.runner_merge",
            PlayerKindFlags::NONE,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::FixedLane,
                inherit_owner_def_res: false,
            },
        )
        .expect("runner merge kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "merge-owner", merge_kind, 0, 10, 3)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(skill_a, 1)])),
            PlayerTemplate::new(2, "merge-target", 1, 10, 3)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(skill_b, 2), (skill_c, 3)])),
        ],
        registry,
    ));

    runtime.effects.push(QueuedEffect::Merge {
        caster: EntityIdx(0),
        target: EntityIdx(1),
    });

    let frame = runtime.flush_effects().expect("runner merge should emit updates");
    let outcome = RoundOutcome {
        action: None,
        frame: Some(frame),
        winner_team: runtime.world.sync_winner(&runtime.entities),
    };
    let actual = NormalizedOutcome::from_runtime(&runtime, &outcome);
    let expected = NormalizedOutcome {
        winner_team: None,
        round: 0,
        total_score: 60,
        rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::default(),
        entity_ids: vec![1, 2],
        teams: vec![0, 1],
        hp: vec![10, 10],
        magic_point: vec![0, 0],
        defense: vec![0, 0],
        resistance: vec![0, 0],
        alive: vec![true, true],
        round_order: vec![0, 1],
        flat_alive: vec![0, 1],
        team_alive: vec![vec![0], vec![1]],
        alive_group_count: 2,
        actions: Vec::new(),
        frames: vec![
            NormalizedUpdateFrame {
                message: "\n".to_owned(),
                caster: 0,
                target: 0,
                targets: Vec::new(),
                param: None,
                score: 0,
                delay0: 0,
                delay1: 0,
                update_type: crate::engine::update::UpdateType::NextLine,
            },
            NormalizedUpdateFrame {
                message: "[0][吞噬]了[1]".to_owned(),
                caster: 0,
                target: 1,
                targets: Vec::new(),
                param: None,
                score: 60,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
            NormalizedUpdateFrame {
                message: "[0]属性上升".to_owned(),
                caster: 0,
                target: 1,
                targets: Vec::new(),
                param: None,
                score: 0,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
        ],
    };

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.skills(), &[skill_a]);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.levels(), &[2]);
    assert_eq!(strict_diff(&expected, &actual), Ok(()));
}

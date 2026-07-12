use super::*;

#[test]
fn runtime_v2_runner_runs_mixed_namerena_raw_fixture_shape() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon = builder
        .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
        .expect("summon skill should register");
    let bed2 = builder
        .register_player_kind_with_policies(
            "custom",
            "bed2",
            "custom.bed2",
            PlayerKindFlags::BED2,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::RootOwner,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::FixedLane,
                inherit_owner_def_res: false,
            },
        )
        .expect("bed2 kind should register");
    let registry = builder.build();
    let raw_input = "plain@red\nalpha@red+bed2[9]\n\nseed:custom-seed@!\n\nbeta@blue+bed2[3]\n";

    let mut runner = prepared_init_tests::runtime_v2_runner_from_raw(raw_input, |raw_groups| {
        RuntimeV2Runner::from_mixed_roster(raw_groups, registry, bed2, summon)
    });
    let legacy = crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy runner should construct");
    runner.runtime_mut().set_skill_handler(summon, skill_noop);
    assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
    let initial_rng = crate::runtime_v2::oracle::NormalizedRngCheckpoint::from_runtime(runner.runtime());
    let plain = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.clone();

    let (summary, actual) = runner.run_until_winner_normalized(8);

    assert_eq!(initial_rng.i, legacy.randomer.i);
    assert_eq!(initial_rng.j, legacy.randomer.j);
    assert_eq!(summary.rounds.len(), 2);
    assert_eq!(summary.winner_team, Some(1));
    assert!(!summary.guard_exhausted);
    let expected = NormalizedOutcome {
        winner_team: Some(1),
        round: 2,
        total_score: plain.attack as u64,
        rng: actual.rng.clone(),
        entity_ids: vec![1, 2, 3],
        teams: vec![1, 1, 0],
        hp: vec![plain.max_hp, 9, 0],
        magic_point: vec![plain.magic_point, 0, 0],
        defense: vec![plain.defense, DEFAULT_BED2_DEFENSE, DEFAULT_BED2_DEFENSE],
        resistance: vec![plain.resistance, DEFAULT_BED2_RESISTANCE, DEFAULT_BED2_RESISTANCE],
        alive: vec![true, true, false],
        round_order: vec![0, 1],
        flat_alive: vec![0, 1],
        team_alive: vec![Vec::new(), vec![0, 1]],
        alive_group_count: 1,
        actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
            round: 2,
            actor: 0,
            target: 2,
            amount: plain.attack,
        }],
        frames: vec![NormalizedUpdateFrame {
            message: "[0]攻击[1]".to_owned(),
            caster: 0,
            target: 2,
            targets: Vec::new(),
            param: None,
            score: plain.attack as u32,
            delay0: crate::engine::update::DEFAULT_DELAY0_MS,
            delay1: crate::engine::update::DEFAULT_DELAY1_MS,
            update_type: crate::engine::update::UpdateType::None,
        }],
    };

    assert_eq!(strict_diff(&expected, &actual), Ok(()));
}

#[test]
fn runtime_v2_runner_rejects_plain_rows_in_bed2_roster() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon = builder
        .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
        .expect("summon skill should register");
    let bed2 = builder
        .register_player_kind_with_policies(
            "custom",
            "bed2",
            "custom.bed2",
            PlayerKindFlags::BED2,
            PlayerKindPolicies::default(),
        )
        .expect("bed2 kind should register");
    let registry = builder.build();
    let raw_groups = vec![vec!["plain".to_owned()], vec!["beta@blue@bed2".to_owned()]];

    let err = RuntimeV2Runner::from_bed2_roster(&raw_groups, registry, bed2, summon)
        .expect_err("bed2-only runner constructor should reject non-bed2 rows");

    assert_eq!(
        err,
        CustomBed2RosterImportError {
            team_index: 0,
            player_index: 0,
            raw: "plain".to_owned(),
        }
    );
}

#[test]
fn runtime_v2_runner_runs_mixed_roster_until_winner() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon = builder
        .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
        .expect("summon skill should register");
    let bed2 = builder
        .register_player_kind_with_policies(
            "custom",
            "bed2",
            "custom.bed2",
            PlayerKindFlags::BED2,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::RootOwner,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::FixedLane,
                inherit_owner_def_res: false,
            },
        )
        .expect("bed2 kind should register");
    let registry = builder.build();
    let raw_groups = vec![
        vec!["plain@red".to_owned(), "alpha@red+bed2[9]".to_owned()],
        vec!["seed:custom-seed@!".to_owned(), "beta@blue+bed2[3]".to_owned()],
    ];

    let mut runner = RuntimeV2Runner::from_mixed_roster(&raw_groups, registry, bed2, summon)
        .expect("mixed roster should construct a runtime v2 runner");
    runner.runtime_mut().set_skill_handler(summon, skill_noop);
    let plain = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.clone();

    let (summary, actual) = runner.run_until_winner_normalized(8);

    assert_eq!(summary.rounds.len(), 1);
    assert_eq!(summary.winner_team, Some(0));
    assert!(!summary.guard_exhausted);
    let expected = NormalizedOutcome {
        winner_team: Some(0),
        round: 1,
        total_score: plain.attack as u64,
        rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::after_next_u8(1),
        entity_ids: vec![1, 2, 3],
        teams: vec![0, 0, 1],
        hp: vec![plain.max_hp, 9, 0],
        magic_point: vec![plain.magic_point, 0, 0],
        defense: vec![plain.defense, DEFAULT_BED2_DEFENSE, DEFAULT_BED2_DEFENSE],
        resistance: vec![plain.resistance, DEFAULT_BED2_RESISTANCE, DEFAULT_BED2_RESISTANCE],
        alive: vec![true, true, false],
        round_order: vec![0, 1],
        flat_alive: vec![0, 1],
        team_alive: vec![vec![0, 1], Vec::new()],
        alive_group_count: 1,
        actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
            round: 1,
            actor: 0,
            target: 2,
            amount: plain.attack,
        }],
        frames: vec![NormalizedUpdateFrame {
            message: "[0]攻击[1]".to_owned(),
            caster: 0,
            target: 2,
            targets: Vec::new(),
            param: None,
            score: plain.attack as u32,
            delay0: crate::engine::update::DEFAULT_DELAY0_MS,
            delay1: crate::engine::update::DEFAULT_DELAY1_MS,
            update_type: crate::engine::update::UpdateType::None,
        }],
    };

    assert_eq!(strict_diff(&expected, &actual), Ok(()));
}

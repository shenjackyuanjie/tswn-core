use super::*;

#[test]
fn custom_runner_multi_round_normalized_run_matches_strict_diff_golden() {
    let mut builder = ExtensionRegistryBuilder::default();
    let bed2 = builder
        .register_player_kind_with_policies(
            "custom",
            "bed2-runner",
            "custom.bed2_runner",
            PlayerKindFlags::BED2,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::RootOwner,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::FixedLane,
                inherit_owner_def_res: false,
            },
        )
        .expect("bed2 runner kind should register");
    let registry = builder.build();
    let mut runner = RuntimeV2Runner::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 8, 3),
            PlayerTemplate::with_kind(2, "bed2", bed2, 1, 5, 0).with_def_res(DEFAULT_BED2_DEFENSE, DEFAULT_BED2_RESISTANCE),
        ],
        registry,
    ));

    let run = runner.run_until_winner_normalized_rounds(8);

    assert_eq!(run.winner_team, Some(0));
    assert!(!run.guard_exhausted);
    assert_eq!(run.total_score, 6);
    assert_eq!(run.rounds.len(), 3);
    let expected_rounds = vec![
        NormalizedOutcome {
            winner_team: None,
            round: 1,
            total_score: 3,
            rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::after_next_u8(1),
            entity_ids: vec![1, 2],
            teams: vec![0, 1],
            hp: vec![8, 2],
            magic_point: vec![0, 0],
            defense: vec![0, DEFAULT_BED2_DEFENSE],
            resistance: vec![0, DEFAULT_BED2_RESISTANCE],
            alive: vec![true, true],
            round_order: vec![0, 1],
            flat_alive: vec![0, 1],
            team_alive: vec![vec![0], vec![1]],
            alive_group_count: 2,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 1,
                actor: 0,
                target: 1,
                amount: 3,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 0,
                target: 1,
                targets: Vec::new(),
                param: None,
                score: 3,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        },
        NormalizedOutcome {
            winner_team: None,
            round: 2,
            total_score: 0,
            rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::after_next_u8(2),
            entity_ids: vec![1, 2],
            teams: vec![0, 1],
            hp: vec![8, 2],
            magic_point: vec![0, 0],
            defense: vec![0, DEFAULT_BED2_DEFENSE],
            resistance: vec![0, DEFAULT_BED2_RESISTANCE],
            alive: vec![true, true],
            round_order: vec![0, 1],
            flat_alive: vec![0, 1],
            team_alive: vec![vec![0], vec![1]],
            alive_group_count: 2,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 2,
                actor: 1,
                target: 0,
                amount: 0,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 1,
                target: 0,
                targets: Vec::new(),
                param: None,
                score: 0,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        },
        NormalizedOutcome {
            winner_team: Some(0),
            round: 3,
            total_score: 3,
            rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::after_next_u8(3),
            entity_ids: vec![1, 2],
            teams: vec![0, 1],
            hp: vec![8, 0],
            magic_point: vec![0, 0],
            defense: vec![0, DEFAULT_BED2_DEFENSE],
            resistance: vec![0, DEFAULT_BED2_RESISTANCE],
            alive: vec![true, false],
            round_order: vec![0],
            flat_alive: vec![0],
            team_alive: vec![vec![0], Vec::new()],
            alive_group_count: 1,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 3,
                actor: 0,
                target: 1,
                amount: 3,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 0,
                target: 1,
                targets: Vec::new(),
                param: None,
                score: 3,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        },
    ];

    for (expected, actual) in expected_rounds.iter().zip(&run.rounds) {
        assert_eq!(strict_diff(expected, actual), Ok(()));
    }
}

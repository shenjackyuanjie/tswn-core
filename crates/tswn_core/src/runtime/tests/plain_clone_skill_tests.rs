use super::*;

/// 按给定 fields 组装 owner/target 的 namerena raw 输入并构建 RuntimeRunner。
fn effective_skill_runner(fields: &str) -> RuntimeRunner {
    let raw = format!("owner+ol:{{\"attrs\":[86,86,86,86,86,86,116,300],{fields}}}\ntarget+ol:{{\"skills\":{{}}}}");
    RuntimeRunner::new_from_namerena_raw(raw).unwrap()
}

#[test]
fn effective_skill_draw_uses_one_sample_and_keeps_the_basic_attack_interval() {
    let actor = EntityIdx(0);
    let mut forward = effective_skill_runner(
        r#""skills":{"sklclone":64,"skliron":32,"sklshield":64},"effective_skills":{"sklclone":64.5,"skliron":32.25,"sklshield":128}"#,
    );
    let mut reverse = effective_skill_runner(
        r#""skills":{"sklshield":64,"skliron":32,"sklclone":64},"effective_skills":{"sklshield":128,"skliron":32.25,"sklclone":64.5}"#,
    );
    assert_eq!(
        forward.runtime().entities.get(actor).unwrap().template.skills,
        reverse.runtime().entities.get(actor).unwrap().template.skills,
    );
    let mut expected_rng = forward.runtime().rng.clone();
    let mut counts = [0; 3];
    for _ in 0..256 {
        let roll = f64::from(expected_rng.rFFFFFF()) / 131_072.0;
        let (expected, bucket) = if roll < 32.25 {
            (Some(BuiltinActiveSkill::Iron), 0)
        } else if roll < 96.75 {
            (Some(BuiltinActiveSkill::Clone), 1)
        } else {
            (None, 2)
        };
        counts[bucket] += 1;
        for runner in [&mut forward, &mut reverse] {
            let runtime = runner.runtime_mut();
            let selected = runtime
                .scan_plain_action_skill_probabilities(actor, false)
                .map(|action| action.selected.skill);
            assert_eq!(selected, expected);
            assert_eq!((runtime.rng.i, runtime.rng.j), (expected_rng.i, expected_rng.j));
        }
    }
    assert!(counts.into_iter().all(|count| count > 0));
}

#[test]
fn effective_skill_draw_normalizes_excess_without_redrawing_unavailable_skills() {
    let actor = EntityIdx(0);
    let mut runner = effective_skill_runner(r#""effective_skills":{"sklcharge":100,"sklclone":100}"#);
    let runtime = runner.runtime_mut();
    runtime.entities.get_mut(actor).unwrap().runtime.charge.active = true;
    let mut expected_rng = runtime.rng.clone();
    for _ in 0..32 {
        let expected = (expected_rng.rFFFFFF() >= 8_388_608).then_some(BuiltinActiveSkill::Clone);
        let selected = runtime
            .scan_plain_action_skill_probabilities(actor, false)
            .map(|action| action.selected.skill);
        assert_eq!(selected, expected);
        assert_eq!((runtime.rng.i, runtime.rng.j), (expected_rng.i, expected_rng.j));
    }
}

#[test]
fn effective_skill_redistribution_shrinks_the_range_for_blocked_skills() {
    let actor = EntityIdx(0);
    let mut runner =
        effective_skill_runner(r#""effective_skills":{"sklcharge":100,"sklclone":100},"effective_skill_redistribution":0.5"#);
    let runtime = runner.runtime_mut();
    runtime.entities.get_mut(actor).unwrap().runtime.charge.active = true;
    let mut expected_rng = runtime.rng.clone();
    for _ in 0..64 {
        let roll = f64::from(expected_rng.rFFFFFF()) * (150.0 / 16_777_216.0);
        let expected = (roll < 100.0).then_some(BuiltinActiveSkill::Clone);
        let selected = runtime
            .scan_plain_action_skill_probabilities(actor, false)
            .map(|action| action.selected.skill);
        assert_eq!(selected, expected);
        assert_eq!((runtime.rng.i, runtime.rng.j), (expected_rng.i, expected_rng.j));
    }
}

#[test]
fn effective_skill_weights_follow_owner_and_clone_levels() {
    let mut runner = effective_skill_runner(
        r#""skills":{"sklclone":128,"sklexchange":"2*40"},"effective_skills":{"sklclone":128,"sklexchange":20}"#,
    );
    let runtime = runner.runtime_mut();
    let actor = EntityIdx(0);
    let skills = &mut runtime.entities.get_mut(actor).unwrap().template.skills;
    let lane_for_key = |key| (0..skills.len()).find(|&lane| skills.fixed_lane_key_at(lane) == Some(key)).unwrap();
    let exchange_lane = lane_for_key(BuiltinActiveSkill::Exchange.legacy_key());
    let clone_lane = lane_for_key(BuiltinActiveSkill::Clone.legacy_key());
    skills.set_level_at(exchange_lane, 40);
    runtime.drain_plain_clone_skill_into(actor, clone_lane, &mut RunUpdates::new());

    for (entity, expected_exchange) in [(actor, 10.0), (EntityIdx(2), 20.0)] {
        let skills = &runtime.entities.get(entity).unwrap().template.skills;
        let weights = skills.effective_action_weights().unwrap().collect::<Vec<_>>();
        assert_eq!(
            weights,
            vec![
                (exchange_lane, expected_exchange),
                (clone_lane, f64::from(skills.level_at(clone_lane).unwrap()))
            ],
        );
    }
}

#[test]
fn compact_effective_skill_draw_is_invariant_to_field_order() {
    let actor = EntityIdx(0);
    let mut forward = effective_skill_runner(
        r#""skills":{"sklcharge":64,"skliron":64,"sklclone":64},"effective_skills":{"sklcharge":64,"skliron":64,"sklclone":64},"effective_skill_adjustment":{"iron":0.2,"clone":0.3}"#,
    );
    let mut reverse = effective_skill_runner(
        r#""skills":{"sklclone":64,"skliron":64,"sklcharge":64},"effective_skills":{"sklclone":64,"skliron":64,"sklcharge":64},"effective_skill_adjustment":{"clone":0.3,"iron":0.2}"#,
    );
    for runner in [&mut forward, &mut reverse] {
        runner.runtime_mut().entities.get_mut(actor).unwrap().runtime.charge.active = true;
    }
    let mut expected_rng = forward.runtime().rng.clone();
    for _ in 0..64 {
        let expected = match expected_rng.rFFFFFF() {
            0..8_388_608 => Some(BuiltinActiveSkill::Iron),
            8_388_608..12_582_912 => Some(BuiltinActiveSkill::Clone),
            _ => None,
        };
        for runner in [&mut forward, &mut reverse] {
            let runtime = runner.runtime_mut();
            let selected = runtime
                .scan_plain_action_skill_probabilities(actor, false)
                .map(|action| action.selected.skill);
            assert_eq!(selected, expected);
            assert_eq!((runtime.rng.i, runtime.rng.j), (expected_rng.i, expected_rng.j));
        }
    }
}

#[test]
fn compact_effective_skill_draw_updates_clone_probability_with_level_and_cut() {
    let actor = EntityIdx(0);
    for (cut, initial) in [(0.0, 8_388_608), (1.0, 4_194_304)] {
        let mut runner = effective_skill_runner(&format!(
            r#""skills":{{"sklfire":64,"sklclone":64}},"effective_skills":{{"sklfire":64,"sklclone":64}},"effective_skill_adjustment":{{"clone":{cut}}}"#,
        ));
        let runtime = runner.runtime_mut();
        let skills = &mut runtime.entities.get_mut(actor).unwrap().template.skills;
        let clone_lane = (0..skills.len())
            .find(|&lane| skills.fixed_lane_key_at(lane) == Some(BuiltinActiveSkill::Clone.legacy_key()))
            .unwrap();
        for (level, threshold) in [(64, initial), (32, initial / 2)] {
            runtime.entities.get_mut(actor).unwrap().template.skills.set_level_at(clone_lane, level);
            for _ in 0..32 {
                let expected_clone = runtime.rng.clone().rFFFFFF() < threshold;
                let selected = runtime
                    .scan_plain_action_skill_probabilities(actor, false)
                    .map(|action| action.selected.skill);
                assert_eq!(selected == Some(BuiltinActiveSkill::Clone), expected_clone);
            }
        }
    }
}

#[test]
fn auto_effective_skill_draw_reuses_the_interval_after_target_failure() {
    let actor = EntityIdx(0);
    let fields = r#""skills":{"sklassassinate":64,"sklclone":64},"effective_skills":true"#;
    let mut runner = effective_skill_runner(fields);
    let mut reference = effective_skill_runner(fields);
    for runtime in [runner.runtime_mut(), reference.runtime_mut()] {
        runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 160;
    }
    let mut failed_targets = 0;
    for _ in 0..64 {
        let expected_runtime = reference.runtime_mut();
        let roll = expected_runtime.rng.rFFFFFF();
        let selected_clone = if roll < 8_388_608 {
            assert!(expected_runtime.select_plain_assassinate_targets(actor, true).is_empty());
            failed_targets += 1;
            roll < 4_194_304
        } else {
            roll < 12_582_912
        };
        let runtime = runner.runtime_mut();
        let selected = runtime
            .scan_plain_action_skill_probabilities(actor, true)
            .map(|action| action.selected.skill);
        assert_eq!(selected, selected_clone.then_some(BuiltinActiveSkill::Clone));
        assert_eq!((runtime.rng.i, runtime.rng.j), (expected_runtime.rng.i, expected_runtime.rng.j));
    }
    assert!(failed_targets > 0);
}

#[test]
fn plain_clone_inherits_summon_blueprint_and_can_summon() {
    let mut runtime = super::skills_summon_tests::summon_runtime();
    let owner = EntityIdx(1);
    let clone_lane = {
        let skills = &runtime.entities.get(owner).unwrap().template.skills;
        (0..skills.len())
            .find(|lane| skills.fixed_lane_key_at(*lane) == Some(BuiltinActiveSkill::Clone.legacy_key()))
            .expect("fixture owner should contain clone")
    };

    runtime.drain_plain_clone_skill_into(owner, clone_lane, &mut RunUpdates::new());

    let clone = EntityIdx(2);
    let clone_entity = runtime.entities.get(clone).expect("clone should spawn");
    assert!(clone_entity.runtime.is_minion());
    assert!(!clone_entity.runtime.is_combat_minion());
    assert_eq!(
        clone_entity.runtime.kind,
        runtime
            .registry
            .player_kind_id_by_export_name(DEFAULT_CORE_CLONE_KIND_EXPORT)
            .expect("core clone kind should exist")
    );
    let blueprint_slot = runtime
        .registry
        .entity_slot_id_by_export_name(DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT)
        .expect("core summon blueprint slot should exist");
    assert!(matches!(
        runtime.entities.get(clone).unwrap().slots.get(blueprint_slot),
        Some(SlotValue::PlayerTemplate(_))
    ));

    let mut updates = RunUpdates::new();
    runtime.drain_plain_summon_skill_into(clone, &mut updates);

    let summoned = EntityIdx(4);
    assert_eq!(runtime.entities.get(summoned).unwrap().runtime.owner, clone);
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0]使用[血祭]", "召唤出[1]"]
    );

    let mut clone_death_updates = RunUpdates::new();
    runtime.emit_plain_lethal_replay_into(owner, clone, &mut clone_death_updates);
    assert_eq!(clone_death_updates.updates.last().unwrap().message, "[1]被击倒了");

    let mut summon_death_updates = RunUpdates::new();
    runtime.emit_plain_lethal_replay_into(owner, summoned, &mut summon_death_updates);
    assert_eq!(summon_death_updates.updates.last().unwrap().message, "[1]消失了");
}

#[test]
fn namer_pf_score_clone_rebuilds_reraise_level_like_legacy() {
    let raw = "! #NHe2ywg@Unbound\n33555277@!\n\n33555278@!\n33555279@!";
    let config = default_custom_runtime_import_config().expect("runtime config should build");
    let mut runner = RuntimeRunner::from_custom_mixed_namerena_raw_with_eval_rq(
        raw.to_owned(),
        crate::namerena::eval_name::WIN_RATE_EVAL_RQ,
        config,
    )
    .expect("namer-pf round should build");

    for _ in 0..14 {
        runner.run_round();
    }

    let clone = runner
        .runtime()
        .entities
        .get(EntityIdx(4))
        .expect("round 14 should spawn the score profile clone");
    let reraise_lane = (0..clone.template.skills.len())
        .find(|lane| clone.template.skills.fixed_lane_key_at(*lane) == Some(28))
        .expect("score profile clone should contain reraise");

    assert_eq!(clone.template.skills.level_at(reraise_lane), Some(14));
}

#[test]
fn score_profile_custom_shadow_clone_can_cast_shadow() {
    let raw = r#"涵虚不等式 PFVKEUPBU@TigerStar+ol:{"attrs":[89,85,88,77,48,96,97,327],"skills":{"sklclone":13,"sklcharm":7,"skldefend":10,"sklheal":5,"sklexchange":3,"sklshadow":"2*46","sklshield":9,"sklreflect":"6+6"},"name_factor_enabled":true,"shadow":{"attrs":[89,85,88,77,48,96,97,164],"skills":{"sklclone":13,"sklcharm":7,"skldefend":10,"sklheal":5,"sklexchange":3,"sklshadow":"2*46","sklshield":9,"sklreflect":"6+6"}}}
33554431@!

33554432@!
33554433@!"#;
    let config = default_custom_runtime_import_config().expect("runtime config should build");
    let mut runner = RuntimeRunner::from_custom_mixed_namerena_raw_with_eval_rq(
        raw.to_owned(),
        crate::namerena::eval_name::WIN_RATE_EVAL_RQ,
        config,
    )
    .expect("custom shadow score round should build");

    runner.run_to_completion_prevalidated(100_000);
}

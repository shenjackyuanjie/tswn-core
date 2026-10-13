use super::*;
use crate::runtime::{
    DEFAULT_CUSTOM_BED2_SHADOW_KIND_EXPORT, DEFAULT_CUSTOM_BED2_SHADOW_TEMPLATE_EXPORT,
    DEFAULT_CUSTOM_BED2_SUMMON_EXPLODE_SKILL_EXPORT, DEFAULT_CUSTOM_BED2_SUMMON_FIRE_SKILL_EXPORT,
    DEFAULT_CUSTOM_BED2_SUMMON_KIND_EXPORT, DEFAULT_CUSTOM_BED2_SUMMON_SKILL_EXPORT, DEFAULT_CUSTOM_BED2_SUMMON_TEMPLATE_EXPORT,
    DEFAULT_CUSTOM_BED2_ZOMBIE_KIND_EXPORT, DEFAULT_CUSTOM_BED2_ZOMBIE_TEMPLATE_EXPORT,
    DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT, EntityIdx, SlotValue, TemplateSlotId, default_custom_runtime_import_config,
};

#[test]
fn runtime_update_type_names_are_stable_json_tokens() {
    assert_eq!(runtime_update_type_name(UpdateType::Win), "win");
    assert_eq!(runtime_update_type_name(UpdateType::None), "none");
    assert_eq!(runtime_update_type_name(UpdateType::NextLine), "next_line");
}

#[test]
fn factored_batch_rate_treats_mirrors_as_weighted_fifty_percent() {
    let targets = vec!["mario\nluigi".to_string()];
    let players = vec!["luigi\nmario".to_string()];
    let result = batch_rate_factored(&targets, &[2.0], &players, 1, None, false, 1).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].avg_win_rate, 50.0);
    assert_eq!(result[0].wins, 1);
    assert_eq!(result[0].total, 2);
}

#[test]
fn factored_batch_rate_validates_factor_length() {
    let err = batch_rate_factored(&["mario".to_string()], &[], &["luigi".to_string()], 1, None, false, 1)
        .expect_err("factor length must match target groups");
    assert_eq!(err.to_string(), "target_factors must match target_groups length");
}

#[test]
fn cli_api_custom_runtime_mixed_runner_imports_custom_profile_raw() {
    let config = default_custom_runtime_import_config().expect("default custom runtime profile should build");
    let bed2 = config.bed2_kind;
    let raw = "plain@red\nalpha@red@bed2\n\nseed:custom-seed@!\n\nbeta@blue+bed2[8]\n";

    let runner = custom_runtime_mixed_runner(raw, config).expect("custom runtime mixed runner should build");
    assert_eq!(runner.runtime().entities.get(EntityIdx(1)).unwrap().template.kind, bed2);
    assert_eq!(runner.runtime().entities.get(EntityIdx(2)).unwrap().template.kind, bed2);
    let mut round_order = runner.runtime().world.round_order().iter().map(|idx| idx.0).collect::<Vec<_>>();
    round_order.sort_unstable();
    assert_eq!(round_order, vec![0, 1, 2]);
}

#[test]
fn cli_api_default_custom_runtime_mixed_runner_imports_ol_minion_overlays() {
    let raw = "plain@red\n\
alpha@red@bed2+ol:{\"summon\":{\"attrs\":[46,47,48,49,50,51,52,123],\"skills\":{\"sklfire2\":4,\"sklfire1\":5},\"inherit_owner_def_res\":true}}\n\
beta@red@bed2+ol:{\"shadow\":{\"attrs\":[47,48,49,50,51,52,53,88],\"skills\":{\"phantom:sklpossess\":5}}}\n\
gamma@red@bed2+ol:{\"zombie\":{\"attrs\":[46,47,48,49,50,51,52,77],\"skills\":{}}}\n\n\
seed:custom-seed@!\n\n\
delta@blue+bed2[8]\n";

    let runner = default_custom_runtime_mixed_runner(raw).expect("default custom runtime mixed runner should build");
    let runtime = runner.runtime();
    let summon_skill = runtime
        .registry
        .skill_id_by_export_name(DEFAULT_CUSTOM_BED2_SUMMON_SKILL_EXPORT)
        .expect("default profile should register summon skill");
    let fire_skill = runtime
        .registry
        .skill_id_by_export_name(DEFAULT_CUSTOM_BED2_SUMMON_FIRE_SKILL_EXPORT)
        .expect("default profile should register summon fire skill");
    let explode_skill = runtime
        .registry
        .skill_id_by_export_name(DEFAULT_CUSTOM_BED2_SUMMON_EXPLODE_SKILL_EXPORT)
        .expect("default profile should register summon explode skill");
    let possess_skill = runtime
        .registry
        .skill_id_by_export_name(DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT)
        .expect("default profile should register possess skill");
    assert!(runtime.skill_handlers.get(summon_skill).is_some());
    assert!(runtime.skill_handlers.get(fire_skill).is_some());
    assert!(runtime.skill_handlers.get(explode_skill).is_some());
    assert!(runtime.skill_handlers.get(possess_skill).is_some());
    let summon_kind = runtime
        .registry
        .player_kinds()
        .iter()
        .find(|kind| kind.export_name == DEFAULT_CUSTOM_BED2_SUMMON_KIND_EXPORT)
        .expect("default profile should register summon kind")
        .id;
    let shadow_kind = runtime
        .registry
        .player_kinds()
        .iter()
        .find(|kind| kind.export_name == DEFAULT_CUSTOM_BED2_SHADOW_KIND_EXPORT)
        .expect("default profile should register shadow kind")
        .id;
    let zombie_kind = runtime
        .registry
        .player_kinds()
        .iter()
        .find(|kind| kind.export_name == DEFAULT_CUSTOM_BED2_ZOMBIE_KIND_EXPORT)
        .expect("default profile should register zombie kind")
        .id;
    let summon_template_slot = runtime
        .registry
        .template_slots()
        .iter()
        .find(|slot| slot.export_name == DEFAULT_CUSTOM_BED2_SUMMON_TEMPLATE_EXPORT)
        .expect("default profile should reserve summon template slot")
        .id;
    let shadow_template_slot = runtime
        .registry
        .template_slots()
        .iter()
        .find(|slot| slot.export_name == DEFAULT_CUSTOM_BED2_SHADOW_TEMPLATE_EXPORT)
        .expect("default profile should reserve shadow template slot")
        .id;
    let zombie_template_slot = runtime
        .registry
        .template_slots()
        .iter()
        .find(|slot| slot.export_name == DEFAULT_CUSTOM_BED2_ZOMBIE_TEMPLATE_EXPORT)
        .expect("default profile should reserve zombie template slot")
        .id;
    assert_eq!(summon_template_slot, TemplateSlotId(0));

    assert_eq!(
        runtime.entities.get(EntityIdx(1)).unwrap().template.skills.skills(),
        &[summon_skill]
    );
    let SlotValue::PlayerTemplate(summon_template) = runtime
        .template_slots
        .get(summon_template_slot)
        .expect("default cli api should populate summon template slot")
    else {
        panic!("default cli api summon overlay slot should hold PlayerTemplate");
    };
    assert_eq!(summon_template.kind, summon_kind);
    assert_eq!(summon_template.max_hp, 123);
    assert_eq!(summon_template.skills.skills(), &[fire_skill, fire_skill, explode_skill]);
    assert_eq!(summon_template.skills.active_order(), &[1, 0]);

    let SlotValue::PlayerTemplate(shadow_template) = runtime
        .template_slots
        .get(shadow_template_slot)
        .expect("default cli api should populate shadow template slot")
    else {
        panic!("default cli api shadow overlay slot should hold PlayerTemplate");
    };
    assert_eq!(shadow_template.kind, shadow_kind);
    assert_eq!(shadow_template.max_hp, 88);
    assert_eq!(shadow_template.skills.skills(), &[possess_skill]);

    let SlotValue::PlayerTemplate(zombie_template) = runtime
        .template_slots
        .get(zombie_template_slot)
        .expect("default cli api should populate zombie template slot")
    else {
        panic!("default cli api zombie overlay slot should hold PlayerTemplate");
    };
    assert_eq!(zombie_template.kind, zombie_kind);
    assert_eq!(zombie_template.max_hp, 77);
    assert!(zombie_template.skills.skills().is_empty());
}

#[test]
fn cli_api_default_custom_runtime_mixed_runner_rejects_unimplemented_minion_heal() {
    let raw = "alpha@red@bed2+ol:{\"zombie\":{\"attrs\":[46,47,48,49,50,51,52,77],\"skills\":{\"sklheal\":3}}}\n\n\
beta@blue\n";

    let err = default_custom_runtime_mixed_runner(raw)
        .expect_err("default custom runtime profile must reject imported skills without handlers");

    assert_eq!(
        err.to_string(),
        "runtime missing skill handlers: custom.minion.heal (id 4) used by template slot 2"
    );
}

#[test]
fn cli_api_custom_runtime_normalized_run_executes_plain_raw() {
    let config = default_custom_runtime_import_config().expect("default custom runtime profile should build");
    let raw = "left@red\n\nright@blue\n";

    let run = custom_runtime_normalized_run(raw, 1, config).expect("custom runtime normalized run should execute");

    assert_eq!(run.rounds.len(), 1);
    assert_eq!(run.guard_exhausted, run.winner_team.is_none());
    assert!(!run.rounds[0].frames.is_empty());
}

#[test]
fn cli_api_default_custom_runtime_normalized_run_executes_plain_raw() {
    let raw = "left@red\n\nright@blue\n";

    let run = default_custom_runtime_normalized_run(raw, 1).expect("default custom runtime normalized run should execute");

    assert_eq!(run.rounds.len(), 1);
    assert_eq!(run.guard_exhausted, run.winner_team.is_none());
    assert!(!run.rounds[0].frames.is_empty());
}

#[test]
fn cli_api_default_custom_runtime_normalized_run_executes_bed2_summon_overlay() {
    let raw = "alpha@red+bed2[3000]+ol:{\"summon\":{\"attrs\":[46,47,48,49,50,51,52,123],\"skills\":{\"sklfire1\":5}}}\n\n\
beta@blue\n";

    let run = default_custom_runtime_normalized_run(raw, 1).expect("default custom runtime summon run should execute");

    assert_eq!(run.rounds.len(), 1);
    assert_eq!(
        run.rounds[0]
            .frames
            .iter()
            .take(2)
            .map(|frame| frame.message.as_str())
            .collect::<Vec<_>>(),
        vec!["[0]使用[血祭]", "召唤出[1]"]
    );
    assert_eq!(run.rounds[0].frames[0].score, 60);
    assert_eq!(run.rounds[0].frames[1].target, 2);
    assert_eq!(run.rounds[0].hp, vec![3000, 295, 123]);
    assert_eq!(run.rounds[0].alive, vec![true, true, true]);
    assert_eq!(run.rounds[0].team_alive, vec![vec![0, 2], vec![1]]);
    assert_eq!(run.rounds[0].flat_alive, vec![0, 2, 1]);
}

#[test]
fn cli_api_default_custom_runtime_normalized_run_executes_spawned_summon_fire() {
    let raw = "alpha@red+bed2[3000]+ol:{\"summon\":{\"attrs\":[46,47,48,49,50,51,52,123],\"skills\":{\"sklfire1\":5}}}\n\n\
beta@blue\n";

    let run = default_custom_runtime_normalized_run(raw, 3).expect("default custom runtime summon fire run should execute");

    assert_eq!(run.rounds.len(), 3);
    assert_eq!(run.rounds[2].actions[0].actor, 2);
    let fire = &run.rounds[2].frames[0];
    assert_eq!(fire.message, "[0]使用[火球术]");
    assert_eq!(fire.caster, 2);
    assert_eq!(fire.target, 1);
    assert_eq!(fire.score, 1);

    let resolution = &run.rounds[2].frames[1];
    match resolution.message.as_str() {
        "[0]攻击[1]" => {
            assert_eq!(resolution.caster, 2);
            assert_eq!(resolution.target, 1);
            assert!(resolution.score > 0);
        }
        "[0][回避]了攻击" => {
            assert_eq!(resolution.caster, 1);
            assert_eq!(resolution.target, 2);
            assert_eq!(resolution.score, 20);
        }
        message => panic!("unexpected summon fire resolution: {message}"),
    }
}

#[test]
fn cli_api_default_custom_runtime_normalized_run_executes_spawned_summon_explode() {
    let raw = "alpha@red+bed2[3000]+ol:{\"summon\":{\"attrs\":[46,47,48,49,50,51,52,123],\"skills\":{\"sklexplode\":5}}}\n\n\
beta@blue\n";

    let run = default_custom_runtime_normalized_run(raw, 3).expect("default custom runtime summon explode run should execute");

    assert_eq!(run.rounds.len(), 3);
    assert_eq!(run.rounds[2].actions[0].actor, 2);
    assert_eq!(
        run.rounds[2]
            .frames
            .iter()
            .take(2)
            .map(|frame| frame.message.as_str())
            .collect::<Vec<_>>(),
        vec!["[0]使用[自爆]", "[1]受到[2]点伤害[s_dmg160]"]
    );
    assert_eq!(run.rounds[2].frames[0].caster, 2);
    assert_eq!(run.rounds[2].frames[0].target, 1);
    assert_eq!(run.rounds[2].frames[0].score, 0);
    assert!(!run.rounds[2].alive[2]);
    assert_eq!(run.rounds[2].team_alive[0], vec![0]);
    assert!(run.rounds[2].hp[1] < run.rounds[1].hp[1]);
}

#[test]
fn cli_api_custom_runtime_normalized_run_rejects_zero_max_rounds() {
    let config = default_custom_runtime_import_config().expect("default custom runtime profile should build");
    let err = custom_runtime_normalized_run("left@red\n\nright@blue\n", 0, config)
        .expect_err("custom runtime normalized run should reject zero max rounds");

    assert_eq!(err.to_string(), "runtime max_rounds must be positive");
}

#[test]
fn cli_api_default_custom_runtime_normalized_run_rejects_zero_max_rounds() {
    let err = default_custom_runtime_normalized_run("left@red\n\nright@blue\n", 0)
        .expect_err("default custom runtime normalized run should reject zero max rounds");

    assert_eq!(err.to_string(), "runtime max_rounds must be positive");
}

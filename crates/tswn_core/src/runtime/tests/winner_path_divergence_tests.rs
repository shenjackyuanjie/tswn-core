//! 批量热路径与全量扫描的判胜一致性检查。
//!
//! 批量路径（`run_to_completion_prevalidated`）不扫实体表，改用 `team_alive` 视图判胜；
//! 可交互路径用 `sync_winner` 扫描实体表。这里在批量路径的每一步同时求两种结果，
//! 并检查 `team_alive`、`flat_alive` 与实体表 `runtime.alive` 的一致性。
//!
//! `alive_group_count` 是 legacy 粘性计数（队伍清空后复活不回补），只做统计打印，
//! 不参与判胜；它一旦被用作判据，就会出现“残局被提前判出胜者”的分歧。

use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::runtime::{RuntimeRunner, default_custom_runtime_import_config};

const MAX_ROUNDS: usize = 20_000;

#[derive(Default, Debug)]
struct CaseReport {
    rounds: usize,
    winner: Option<usize>,
    winner_mismatches: usize,
    count_mismatches: usize,
    team_view_mismatches: usize,
    flat_view_mismatches: usize,
    revives: usize,
    first_detail: Option<String>,
}

/// 实体表视角的存活队伍 -> 成员。
fn entity_alive_teams(runner: &RuntimeRunner) -> BTreeMap<usize, BTreeSet<usize>> {
    let mut teams: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for (idx, entity) in runner.runtime.entities.iter() {
        if entity.runtime.alive {
            teams.entry(entity.runtime.team).or_default().insert(idx.0 as usize);
        }
    }
    teams
}

fn world_view_teams(runner: &RuntimeRunner) -> BTreeMap<usize, BTreeSet<usize>> {
    let mut teams: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut team = 0usize;
    while let Some(alive) = runner.runtime.world.team_alive(team) {
        if !alive.is_empty() {
            teams.insert(team, alive.iter().map(|idx| idx.0 as usize).collect());
        }
        team += 1;
    }
    teams
}

fn flat_view(runner: &RuntimeRunner) -> BTreeSet<usize> {
    runner.runtime.world.flat_alive().iter().map(|idx| idx.0 as usize).collect()
}

/// 实体表视角下当前仍存活的队伍编号。
fn live_teams(runtime: &CombatRuntime) -> Vec<usize> {
    let mut teams = BTreeSet::new();
    for (_, entity) in runtime.entities.iter() {
        if entity.runtime.alive {
            teams.insert(entity.runtime.team);
        }
    }
    teams.into_iter().collect()
}

fn run_case(file_name: &str) -> CaseReport {
    let path = format!("{}/../tswn_test/cases/runtime_stress/{file_name}", env!("CARGO_MANIFEST_DIR"));
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("读取 {path} 失败: {err}"));
    scan_raw(raw, crate::namerena::eval_name::DEFAULT_EVAL_RQ).unwrap_or_else(|err| panic!("{file_name} 导入失败: {err}"))
}

fn scan_raw(raw: String, eval_rq: f64) -> Result<CaseReport, String> {
    let config = default_custom_runtime_import_config().map_err(|err| format!("{err:?}"))?;
    let mut runner =
        RuntimeRunner::from_custom_mixed_namerena_raw_with_eval_rq(raw, eval_rq, config).map_err(|err| format!("{err:?}"))?;

    let mut report = CaseReport::default();
    let mut prev_alive: BTreeSet<usize> = entity_alive_teams(&runner).values().flatten().copied().collect();
    let mut round = 0usize;
    loop {
        let views = runner.runtime.world.sync_winner_from_alive_views();
        let ground = runner.runtime.world.sync_winner(&runner.runtime.entities);
        let entity_teams = entity_alive_teams(&runner);
        let world_teams = world_view_teams(&runner);
        let flat = flat_view(&runner);
        let entity_flat: BTreeSet<usize> = entity_teams.values().flatten().copied().collect();

        if views != ground {
            report.winner_mismatches += 1;
            report.first_detail.get_or_insert_with(|| {
                format!(
                    "round {round}: 全量扫描={ground:?} alive视图={views:?} 实体队伍={entity_teams:?} world队伍={world_teams:?} alive_group_count={}",
                    runner.runtime.world.alive_group_count()
                )
            });
        }
        if runner.runtime.world.alive_group_count() != entity_teams.len() {
            report.count_mismatches += 1;
            report.first_detail.get_or_insert_with(|| {
                format!(
                    "round {round}: alive_group_count={} 但实体存活队伍={:?}",
                    runner.runtime.world.alive_group_count(),
                    entity_teams.keys().collect::<Vec<_>>()
                )
            });
        }
        if world_teams != entity_teams {
            report.team_view_mismatches += 1;
            report
                .first_detail
                .get_or_insert_with(|| format!("round {round}: team_alive={world_teams:?} 但实体存活={entity_teams:?}"));
        }
        if flat != entity_flat {
            report.flat_view_mismatches += 1;
            report
                .first_detail
                .get_or_insert_with(|| format!("round {round}: flat_alive={flat:?} 但实体存活={entity_flat:?}"));
        }
        if views.is_some() || round >= MAX_ROUNDS {
            report.rounds = round;
            report.winner = views;
            return Ok(report);
        }
        runner.runtime.run_minimal_round_no_capture();
        round += 1;
        let now_alive: BTreeSet<usize> = entity_alive_teams(&runner).values().flatten().copied().collect();
        report.revives += now_alive.difference(&prev_alive).count();
        prev_alive = now_alive;
    }
}

const CASES: &[&str] = &[
    "1v1-0f92cb76cc37fdc5.txt",
    "2v2-554f4128af707167.txt",
    "2v2-a47d6d806a77e593.txt",
    "3v3v3-0ace5df17b84e26a.txt",
    "3v3v3-0f20f33db97a2e10.txt",
    "3v3v3-35070658d93637e1.txt",
    "3v3v3-3d4a0f9a1de8fa32.txt",
    "3v3v3-42186ce5db28b886.txt",
    "3v3v3-59f9ed82b34e7760.txt",
    "3v3v3-5e1dd340fc7876ac.txt",
    "3v3v3-6c9aabe2aa79d92f.txt",
    "3v3v3-711d7e09df7b7da3.txt",
    "3v3v3-7dfb2a54a1cd12d7.txt",
    "3v3v3-908ecb326213b7aa.txt",
    "3v3v3-947fbfff2b995a89.txt",
    "3v3v3-d6dc36b3d836cf28.txt",
    "3v3v3-db3944e8cf92d8d9.txt",
    "3v3v3-e1541883a7613633.txt",
    "3v3v3-ea03011602664398.txt",
    "cqd-p06-t26-r0135.txt",
    "cqd-p19-t26-r0336.txt",
    "cqd-p21-t31-r5997-guard.txt",
    "cqd-p21-t39-r0107.txt",
    "cqd-p28-t26-r0447.txt",
    "ffa_4-37fc802e0ef650a3.txt",
    "ffa_4-4cdab500cdcdd9bf.txt",
    "ffa_4-b9ba9b639670ceb3.txt",
    "ffa_6-3ae9dd4b7b788849.txt",
    "ffa_6-9af1d802c0fc1ad4.txt",
    "ffa_8-16d11de1ebe1df41.txt",
    "ffa_8-3d155c15a9d6ec4b.txt",
    "ffa_8-76f0580bd07d1405.txt",
    "ffa_8-b9a9c7882f4f1f99.txt",
    "ffa_8-cce83cebc69761de.txt",
    "ffa_8-fee2c41a2508ad16.txt",
];

#[test]
fn stress_cases_have_no_winner_path_divergence() {
    let mut total_rounds = 0usize;
    let mut total_revives = 0usize;
    for file_name in CASES {
        let report = run_case(file_name);
        total_rounds += report.rounds;
        total_revives += report.revives;
        println!(
            "{file_name}: rounds={} winner={:?} revives={} winner_mismatch={} count_mismatch={} team_view_mismatch={} flat_view_mismatch={}",
            report.rounds,
            report.winner,
            report.revives,
            report.winner_mismatches,
            report.count_mismatches,
            report.team_view_mismatches,
            report.flat_view_mismatches,
        );
        if let Some(detail) = report.first_detail {
            println!("  first: {detail}");
        }
        // `alive_group_count` 的残留是 legacy 语义，这里只作为统计打印；
        // 判胜结果与存活视图必须与实体表一致。
        println!("  count_mismatch={}", report.count_mismatches);
        assert_eq!(report.winner_mismatches, 0, "{file_name} 判胜分歧");
        assert_eq!(report.team_view_mismatches, 0, "{file_name} team_alive 与实体表不一致");
        assert_eq!(report.flat_view_mismatches, 0, "{file_name} flat_alive 与实体表不一致");
    }
    println!("总计 rounds={total_rounds} revives={total_revives}");
}

/// 在真实名字池上统计“视图与实体表失配”和“判胜分歧”的出现频率。
///
/// 依赖未纳入版本库的输入池 `tests/sqp5900.txt`，属于本地诊断：
/// `TSWN_DIVERGENCE_BATTLES=10000 cargo test -p tswn_core --lib pool_matchups -- --ignored --nocapture`。
#[test]
#[ignore = "依赖未纳入版本库的输入池，本地手动运行"]
fn pool_matchups_winner_path_divergence_rate() {
    let path = format!("{}/../../tests/sqp5900.txt", env!("CARGO_MANIFEST_DIR"));
    let Ok(raw) = std::fs::read_to_string(&path) else {
        println!("输入池 {path} 不存在，跳过");
        return;
    };
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let pool: Vec<&str> = normalized
        .lines()
        .filter(|line| !line.trim().is_empty() && !crate::namerena::is_seed_line(line))
        .collect();

    let battles_target: usize = std::env::var("TSWN_DIVERGENCE_BATTLES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(2000);
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };

    let started = std::time::Instant::now();
    let mut battles = 0usize;
    let mut skipped = 0usize;
    let mut total_rounds = 0usize;
    let mut total_revives = 0usize;
    let mut view_mismatch_battles = 0usize;
    let mut diverged_battles = 0usize;
    let mut first_detail: Option<String> = None;
    for _ in 0..battles_target {
        let mut picked = BTreeSet::new();
        while picked.len() < 6 {
            picked.insert((next() % pool.len() as u64) as usize);
        }
        let names: Vec<&str> = picked.into_iter().map(|index| pool[index]).collect();
        let raw = names.chunks(2).map(|team| team.join("\n")).collect::<Vec<_>>().join("\n\n");
        let report = match scan_raw(raw, crate::namerena::eval_name::WIN_RATE_EVAL_RQ) {
            Ok(report) => report,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        battles += 1;
        total_rounds += report.rounds;
        total_revives += report.revives;
        if report.count_mismatches + report.team_view_mismatches + report.flat_view_mismatches > 0 {
            view_mismatch_battles += 1;
        }
        if report.winner_mismatches > 0 {
            diverged_battles += 1;
            first_detail.get_or_insert_with(|| report.first_detail.clone().unwrap_or_default());
        }
    }
    println!(
        "输入池对局={battles} 跳过={skipped} 总回合={total_rounds} 复活次数={total_revives} 视图失配对局={view_mismatch_battles} 判胜分歧对局={diverged_battles} 用时={:?}",
        started.elapsed()
    );
    if let Some(detail) = first_detail {
        println!("首个分歧: {detail}");
    }
}

fn kill(runtime: &mut CombatRuntime, actor: EntityIdx, team: usize) {
    let entity = runtime.entities.get_mut(actor).expect("被击杀实体存在");
    entity.runtime.hp = 0;
    entity.runtime.alive = false;
    assert!(runtime.world.mark_dead(actor, team));
}

/// 构造“某队被清空后又复活”的局面，确认批量路径不会把尚未结束的对局判出胜者。
#[test]
fn wiped_team_revived_by_charmed_caster_keeps_batch_winner_consistent() {
    let mut builder = ExtensionRegistryBuilder::default();
    let charm = builder
        .register_state(
            "core",
            "charm",
            DEFAULT_CORE_CHARM_STATE_EXPORT,
            ProcMask::POST_ACTION,
            SkillPriority(210),
        )
        .expect("charm 状态注册");
    let revive = builder
        .register_skill(
            "core",
            "revive",
            BuiltinActiveSkill::Revive.export_name(),
            TargetPolicy::Ally,
            SkillPriority(16),
        )
        .expect("revive 技能注册");
    let registry = builder.build();
    let loadout = SkillLoadout::from_skill_levels([(revive, 19)]);
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "charmed-reviver", 0, 100, 3).with_skill_loadout(loadout),
            PlayerTemplate::new(2, "wiped-then-revived", 1, 100, 3),
            PlayerTemplate::new(3, "bystander", 2, 100, 3),
        ],
        registry,
    ));

    // 队伍 1 被清空：alive_group_count 3 -> 2。
    kill(&mut runtime, EntityIdx(1), 1);
    assert_eq!(runtime.world.alive_group_count(), 2);

    // 队伍 0 的成员被魅惑，视队伍 1 为自己人，因此能复活已被清空的队伍 1。
    assert!(
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::charm(
            76,
            charm,
            2,
            Some(1),
            Some(0),
            Some(0),
            2,
            SkillPriority(210),
        ))
    );
    assert_eq!(runtime.plain_effective_team(EntityIdx(0)), 1);
    let selected = runtime.select_plain_revive_targets(EntityIdx(0), true);
    assert_eq!(selected.as_slice(), &[EntityIdx(1)]);
    let mut updates = RunUpdates::new();
    runtime.drain_plain_revive_skill_into(EntityIdx(0), 0, EntityIdx(1), &mut updates);
    assert!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
    // 复活不会补回 alive_group_count：仍然是 2。
    assert_eq!(runtime.world.alive_group_count(), 2);

    // 队伍 0 被清空：2 -> 1，此时队伍 1 与队伍 2 都还活着。
    kill(&mut runtime, EntityIdx(0), 0);
    assert_eq!(runtime.world.alive_group_count(), 1);
    assert_eq!(live_teams(&runtime), vec![1, 2]);

    // 全量扫描（可交互/数据集路径）看到两队存活，正确地判为无胜者。
    assert_eq!(runtime.world.sync_winner(&runtime.entities), None);
    // alive 视图（批量路径）必须给出同样的结论：复活后的队伍仍算存活，
    // 即使 `alive_group_count` 残留为 1。
    assert_eq!(runtime.world.alive_group_count(), 1);
    assert_eq!(runtime.world.sync_winner_from_alive_views(), None);
    // 批量路径继续打，而不是直接宣布队伍 1 获胜。
    let outcome = runtime.run_minimal_round_no_capture();
    assert_ne!(outcome.winner_team, Some(1));
}

//! 有限的 !test! 评分会话；复用原评分的 profile 名单、空 seed 与初始化路径。
use std::collections::BTreeMap;

use serde::Serialize;

use super::*;
use crate::cli_api::{CliApiError, CliApiResult};
use crate::runtime::search::SearchConfig;

pub const SCORE_TEST_TOTAL: usize = 10_000;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TestSkillFrequency {
    pub message: String,
    pub count: u64,
    /// 与参考站一致：真实消息次数 / 完成场数 × 100，可超过 100%。
    pub frequency: f64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ScoreTestProgress {
    pub completed: usize,
    pub total: usize,
    pub percent: usize,
    pub wins: usize,
    pub score: f64,
    pub truncated: usize,
    pub done: bool,
    pub skills: Vec<TestSkillFrequency>,
}

pub struct ScoreTestSession {
    prepared: PreparedRuntimeRunner,
    runner: RuntimeRunner,
    groups: ScoreMatchGroups,
    subjects: Vec<usize>,
    search: Option<SearchConfig>,
    legacy_opponents: bool,
    completed: usize,
    wins: usize,
    truncated: usize,
    skills: BTreeMap<String, u64>,
    failure: Option<String>,
}

impl ScoreTestSession {
    pub fn new(raw: &str, search: Option<SearchConfig>) -> CliApiResult<Self> { Self::new_with_mode(raw, search, false) }

    /// 普通与强化只改变陪测 profile 修饰符；重复名字仍由原评分规则识别为单挑。
    pub fn new_with_mode(raw: &str, search: Option<SearchConfig>, enhanced: bool) -> CliApiResult<Self> {
        // 保留 !test! 与名字队伍间的空行，让标准解析器把连续名字视为同一队。
        let (groups, _) = RuntimeRunner::split_namerena_into_groups(format!("!test!\n\n{raw}"));
        if groups.len() != 2 || groups[1].is_empty() {
            return Err(CliApiError::InvalidInput(
                "!test! 后请输入一个名字或同一队伍的名字，不要再分队".into(),
            ));
        }
        let target = &groups[1];
        let groups = ScoreMatchGroups::new(target, if enhanced { "!" } else { "\u{0002}" });
        let config = default_custom_runtime_import_config().map_err(|e| CliApiError::RunnerInit(format!("{e:?}")))?;
        let prepared = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(
            &groups.groups,
            crate::namerena::eval_name::WIN_RATE_EVAL_RQ,
            config,
        )
        .map_err(|e| CliApiError::RunnerInit(format!("{e:?}")))?;
        // input_groups 的行动顺序会重排，不能把其中第一个当成被测名字。
        // 评分构造器将固定目标置于实体前缀，随后才是动态 profile。
        let subjects = (0..js_score_targets_per_round(target)).collect();
        let runner = prepared.new_reusable_runner();
        Ok(Self {
            prepared,
            runner,
            groups,
            subjects,
            search,
            legacy_opponents: false,
            completed: 0,
            wins: 0,
            truncated: 0,
            skills: BTreeMap::new(),
            failure: None,
        })
    }

    /// 默认双方采用所选策略；仅显式勾选验证模式时把搜索限制在被测侧及队友。
    pub fn set_legacy_opponents(&mut self, enabled: bool) { self.legacy_opponents = enabled; }

    /// 热循环只读累计值，避免为每个场次重建技能频率和百分比统计。
    pub const fn wins(&self) -> usize { self.wins }

    pub const fn truncated(&self) -> usize { self.truncated }

    pub fn progress(&self) -> ScoreTestProgress {
        ScoreTestProgress {
            completed: self.completed,
            total: SCORE_TEST_TOTAL,
            percent: self.completed * 100 / SCORE_TEST_TOTAL,
            wins: self.wins,
            score: self.wins as f64 * 10_000.0 / self.completed.max(1) as f64,
            truncated: self.truncated,
            done: self.completed == SCORE_TEST_TOTAL,
            skills: self
                .skills
                .iter()
                .map(|(message, count)| TestSkillFrequency {
                    message: message.clone(),
                    count: *count,
                    frequency: *count as f64 * 100.0 / self.completed.max(1) as f64,
                })
                .collect(),
        }
    }

    /// 每次只跑一场；前端 Worker 可在场次间取消，达到 10000 后调用幂等。
    pub fn step(&mut self) -> CliApiResult<()> {
        if let Some(error) = &self.failure {
            return Err(CliApiError::Runtime(error.clone()));
        }
        if self.completed == SCORE_TEST_TOTAL {
            return Ok(());
        }
        self.advance_round_setup()?;
        for _ in 0..BATCH_MAX_ROUNDS {
            if self.runner.have_winner() {
                break;
            }
            // 仅处理正式返回的帧，搜索分支内部的探测、候选试算与续局均不计数。
            let outcome = if let Some(mut config) = self.search {
                // 勾选验证时仅接管固定目标前缀所在队伍；默认接管双方。
                config.team = self
                    .legacy_opponents
                    .then(|| self.runner.runtime().entities.get(crate::runtime::EntityIdx(0)).unwrap().runtime.team);
                self.runner.runtime_mut().run_search_round(config).outcome
            } else {
                self.runner.runtime_mut().run_minimal_round()
            };
            if let Some(frame) = outcome.frame {
                for update in frame.updates.updates {
                    if self.subjects.contains(&update.caster)
                        && update.message.contains("[0]")
                        && update.score > 0
                        && !update.message.contains("[s_dmg0]")
                    {
                        *self.skills.entry(update.message.into_owned()).or_default() += 1;
                    }
                }
            }
        }
        self.truncated += usize::from(!self.runner.have_winner());
        self.wins += usize::from(self.runner.input_group_won(0));
        self.completed += 1;
        Ok(())
    }

    /// 失败检查之后的场次推进：推进轮次游标并重置 profile 阵容；`step` 与 `step_observed` 共用。
    fn advance_round_setup(&mut self) -> CliApiResult<()> {
        self.groups.set_round(self.completed);
        if let Err(error) = self.prepared.reset_score_groups_with_seed_and_eval_rq(
            &mut self.runner,
            &self.groups.groups,
            &self.groups.profile_player_ids,
            &self.groups.modifier,
            &self.groups.profile_team_rng,
            &mut self.groups.skill_buffers,
            &mut self.groups.identity_buffers,
            &mut self.groups.round_scratch,
            &mut self.groups.roster_buffers,
            &[],
            crate::namerena::eval_name::WIN_RATE_EVAL_RQ,
        ) {
            let error = format!("{error:?}");
            self.failure = Some(error.clone());
            return Err(CliApiError::Runtime(error));
        }
        Ok(())
    }

    /// 带逐轮观测回调的单场：每轮调度前回调当前战场（回调时必无胜者），
    /// 返回 (是否超轮数截断, 胜者队伍)。用于自对弈状态采集等离线用途。
    pub fn step_observed<F: FnMut(&crate::runtime::CombatRuntime)>(
        &mut self,
        mut observe: F,
    ) -> CliApiResult<(bool, Option<usize>)> {
        if let Some(error) = &self.failure {
            return Err(CliApiError::Runtime(error.clone()));
        }
        if self.completed == SCORE_TEST_TOTAL {
            return Ok((false, None));
        }
        self.advance_round_setup()?;
        for _ in 0..BATCH_MAX_ROUNDS {
            if self.runner.have_winner() {
                break;
            }
            observe(self.runner.runtime());
            if let Some(mut config) = self.search {
                // 勾选验证时仅接管固定目标前缀所在队伍；默认接管双方。
                config.team = self
                    .legacy_opponents
                    .then(|| self.runner.runtime().entities.get(crate::runtime::EntityIdx(0)).unwrap().runtime.team);
                let _ = self.runner.runtime_mut().run_search_round(config);
            } else {
                let _ = self.runner.runtime_mut().run_minimal_round();
            }
        }
        let winner = self.runner.runtime().world.winner_team();
        let truncated = winner.is_none();
        self.truncated += usize::from(truncated);
        self.wins += usize::from(self.runner.input_group_won(0));
        self.completed += 1;
        Ok((truncated, winner))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_checkpoints_match_existing_score_and_stop_at_ten_thousand() {
        let mut session = ScoreTestSession::new("alpha", None).unwrap();
        for n in 1..=SCORE_TEST_TOTAL {
            session.step().unwrap();
            if n == 1000 || n == SCORE_TEST_TOTAL {
                let baseline = runtime_score(
                    &["alpha".into()],
                    "\u{0002}",
                    n,
                    crate::namerena::eval_name::WIN_RATE_EVAL_RQ,
                    1,
                )
                .unwrap();
                let progress = session.progress();
                assert_eq!(progress.completed, n);
                assert_eq!(progress.score, baseline.score_10000());
                assert!(!progress.skills.is_empty());
                for skill in &progress.skills {
                    assert_eq!(skill.frequency, skill.count as f64 * 100.0 / n as f64);
                }
            }
        }
        let final_result = session.progress();
        assert!(final_result.done);
        session.step().unwrap();
        assert_eq!(session.progress(), final_result);
    }

    #[test]
    fn search_counts_only_real_frames_and_matches_direct_cpu_search() {
        // 两种接管范围、四种模式均比较实际战斗，统计只归属被测本体。
        for legacy_opponents in [false, true] {
            for enhanced in [false, true] {
                for raw in ["alpha", "alpha\nalpha"] {
                    let config = SearchConfig {
                        samples: 2,
                        horizon: 8,
                        ..SearchConfig::default()
                    };
                    let mut session = ScoreTestSession::new_with_mode(raw, Some(config), enhanced).unwrap();
                    if legacy_opponents {
                        session.set_legacy_opponents(true);
                    }
                    let mut cpu = session.prepared.new_reusable_runner();
                    let config = SearchConfig {
                        team: legacy_opponents
                            .then(|| cpu.runtime().entities.get(crate::runtime::EntityIdx(0)).unwrap().runtime.team),
                        ..config
                    };
                    let mut counts = BTreeMap::<String, u64>::new();
                    while !cpu.have_winner() {
                        let result = cpu.runtime_mut().run_search_round(config);
                        if let Some(frame) = result.outcome.frame {
                            for update in frame.updates.updates {
                                if update.caster == 0
                                    && update.score > 0
                                    && update.message.contains("[0]")
                                    && !update.message.contains("[s_dmg0]")
                                {
                                    *counts.entry(update.message.into_owned()).or_default() += 1;
                                }
                            }
                        }
                    }
                    session.step().unwrap();
                    assert_eq!(
                        session
                            .runner
                            .runtime()
                            .entities
                            .get(super::super::super::EntityIdx(0))
                            .unwrap()
                            .template
                            .name,
                        "alpha"
                    );
                    assert_eq!(session.skills, counts);
                    assert_eq!(session.wins, usize::from(cpu.input_group_won(0)));
                    assert_eq!(session.runner.player_snapshots(), cpu.player_snapshots());
                }
            }
        }
    }

    #[test]
    fn four_modes_match_original_score_rosters_and_results() {
        // 对照旧评分的普通/强化修饰符和单名/重名分组，验证实际战斗结果。
        for enhanced in [false, true] {
            for duel in [false, true] {
                let target = vec!["alpha".to_owned(); if duel { 2 } else { 1 }];
                let modifier = if enhanced { "!" } else { "\u{0002}" };
                let mut session = ScoreTestSession::new_with_mode(&target.join("\n"), None, enhanced).unwrap();
                assert_eq!(session.subjects, vec![0]);
                assert_eq!(
                    session.groups.groups.iter().map(Vec::len).collect::<Vec<_>>(),
                    if duel { vec![1, 1] } else { vec![2, 2] }
                );
                for _ in 0..64 {
                    session.step().unwrap();
                }
                let baseline = runtime_score(&target, modifier, 64, crate::namerena::eval_name::WIN_RATE_EVAL_RQ, 1).unwrap();
                assert_eq!(session.progress().score, baseline.score_10000());
                assert!(
                    session
                        .groups
                        .profile_slots
                        .iter()
                        .all(|&(group, player)| session.groups.groups[group][player].ends_with(&format!("@{modifier}")))
                );
            }
        }
    }

    #[test]
    fn rejects_empty_or_multiple_teams() {
        assert!(ScoreTestSession::new("", None).is_err());
        assert!(ScoreTestSession::new("alpha\n\nbeta", None).is_err());
    }

    #[test]
    fn consecutive_names_are_one_team_and_duplicate_mode_keeps_one_subject() {
        let mut session = ScoreTestSession::new("alpha\nalpha", None).unwrap();
        session.step().unwrap();
        let baseline = runtime_score(
            &["alpha".into(), "alpha".into()],
            "\u{0002}",
            1,
            crate::namerena::eval_name::WIN_RATE_EVAL_RQ,
            1,
        )
        .unwrap();
        assert_eq!(session.progress().score, baseline.score_10000());
        assert_eq!(
            session.runner.runtime().entities.get(crate::runtime::EntityIdx(0)).unwrap().template.name,
            "alpha"
        );
    }
}

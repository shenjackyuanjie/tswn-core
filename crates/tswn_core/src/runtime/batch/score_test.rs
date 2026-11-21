use std::borrow::Cow;
use std::collections::BTreeMap;

use serde::Serialize;

use super::{BATCH_MAX_ROUNDS, ScoreMatchGroups, js_score_targets_per_round};
use crate::cli_api::{CliApiError, CliApiResult};
use crate::namerena::eval_name::WIN_RATE_EVAL_RQ;
use crate::namerena::trim_js_line_end;
use crate::runtime::{PreparedRuntimeRunner, RunUpdate, RuntimeRunner, default_custom_runtime_import_config};

pub const SCORE_TEST_TOTAL: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScoreSkillFrequency {
    pub message: String,
    pub count: u64,
    pub frequency: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScoreTestProgress {
    pub completed: usize,
    pub total: usize,
    pub wins: usize,
    pub score: f64,
    pub truncated: usize,
    pub done: bool,
    pub skills: Vec<ScoreSkillFrequency>,
}

/// 固定样本的交互评分；逐帧累计被测本体的消息，不保留回放或改动普通批量评分路径。
pub struct ScoreTestSession {
    prepared: PreparedRuntimeRunner,
    runner: RuntimeRunner,
    groups: ScoreMatchGroups,
    subject_count: usize,
    completed: usize,
    wins: usize,
    truncated: usize,
    skills: BTreeMap<Cow<'static, str>, u64>,
    failure: Option<CliApiError>,
}

impl ScoreTestSession {
    /// 接收剥掉网页指令后的单队文本。强化仅切换陪测 profile 的修饰符。
    pub fn new(raw: &str, enhanced: bool) -> CliApiResult<Self> {
        let raw = raw.replace("\r\n", "\n").replace('\r', "\n");
        let targets = raw.trim().lines().map(trim_js_line_end).map(str::to_owned).collect::<Vec<_>>();
        if targets.is_empty()
            || targets.iter().any(|name| {
                name.trim().is_empty()
                    || matches!(name.trim(), "!" | "!test!")
                    || name.trim_start().get(..5).is_some_and(|prefix| prefix.eq_ignore_ascii_case("seed:"))
            })
        {
            return Err(CliApiError::InvalidInput("评分需要一组名字；不能分队或指定 seed。".into()));
        }

        let groups = ScoreMatchGroups::new(&targets, if enhanced { "!" } else { "\u{0002}" });
        let config = default_custom_runtime_import_config().map_err(|error| CliApiError::RunnerInit(format!("{error:?}")))?;
        let prepared = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(&groups.groups, WIN_RATE_EVAL_RQ, config)
            .map_err(|error| CliApiError::RunnerInit(format!("{error:?}")))?;
        let runner = prepared.new_reusable_runner();
        Ok(Self {
            prepared,
            runner,
            groups,
            subject_count: js_score_targets_per_round(&targets),
            completed: 0,
            wins: 0,
            truncated: 0,
            skills: BTreeMap::new(),
            failure: None,
        })
    }

    /// 推进至多 batch_size 场；完成后不再推进，复位失败后不再继续本批评分。
    pub fn step(&mut self, batch_size: usize) -> CliApiResult<()> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let end = self.completed.saturating_add(batch_size).min(SCORE_TEST_TOTAL);
        while self.completed < end {
            if let Err(error) = self.step_match() {
                self.failure = Some(error.clone());
                return Err(error);
            }
        }
        Ok(())
    }

    fn step_match(&mut self) -> CliApiResult<()> {
        self.groups.set_round(self.completed);
        self.prepared
            .reset_score_groups_with_seed_and_eval_rq(
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
                WIN_RATE_EVAL_RQ,
            )
            .map_err(|error| CliApiError::Runtime(format!("{error:?}")))?;

        for _ in 0..BATCH_MAX_ROUNDS {
            let outcome = self.runner.runtime_mut().run_minimal_round();
            if let Some(frame) = outcome.frame {
                for update in frame.updates.updates {
                    self.count_update(update);
                }
            }
            if outcome.winner_team.is_some() {
                break;
            }
        }
        self.truncated += usize::from(!self.runner.have_winner());
        self.wins += usize::from(self.runner.input_group_won(0));
        self.completed += 1;
        Ok(())
    }

    fn count_update(&mut self, update: RunUpdate) {
        if update.has_caster
            && update.caster < self.subject_count
            && update.score > 0
            && update.message.contains("[0]")
            && !update.message.contains("[s_dmg0]")
        {
            *self.skills.entry(update.message).or_default() += 1;
        }
    }

    pub fn progress(&self) -> ScoreTestProgress {
        let denominator = self.completed.max(1) as f64;
        ScoreTestProgress {
            completed: self.completed,
            total: SCORE_TEST_TOTAL,
            wins: self.wins,
            score: self.wins as f64 * 10_000.0 / denominator,
            truncated: self.truncated,
            done: self.completed == SCORE_TEST_TOTAL,
            skills: self
                .skills
                .iter()
                .map(|(message, &count)| ScoreSkillFrequency {
                    message: message.to_string(),
                    count,
                    frequency: count as f64 * 100.0 / denominator,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::runtime_score;

    #[test]
    fn score_test_checkpoints_match_bulk_scoring_and_stop_at_limit() {
        for raw in ["alpha", "alpha\nalpha"] {
            for enhanced in [false, true] {
                let mut session = ScoreTestSession::new(raw, enhanced).unwrap();
                let targets = raw.lines().map(str::to_owned).collect::<Vec<_>>();
                let modifier = if enhanced { "!" } else { "\u{0002}" };
                for (batch, expected_completed) in [(1_000, 1_000), (usize::MAX, SCORE_TEST_TOTAL)] {
                    session.step(batch).unwrap();
                    let progress = session.progress();
                    let baseline = runtime_score(&targets, modifier, expected_completed, WIN_RATE_EVAL_RQ, 1).unwrap();
                    assert_eq!(progress.completed, expected_completed);
                    assert_eq!(progress.done, expected_completed == SCORE_TEST_TOTAL);
                    assert_eq!(progress.wins, baseline.wins, "{raw:?} enhanced={enhanced}");
                    assert_eq!(progress.score, baseline.score_10000());
                    assert_eq!(progress.truncated, baseline.guard_exhausted);
                    assert!(!progress.skills.is_empty());
                    for skill in progress.skills {
                        assert_eq!(skill.frequency, skill.count as f64 * 100.0 / expected_completed as f64);
                    }
                }
                let final_progress = session.progress();
                session.step(1).unwrap();
                assert_eq!(session.progress(), final_progress);
            }
        }
    }

    #[test]
    fn score_test_treats_consecutive_mixed_names_as_one_team() {
        let mut session = ScoreTestSession::new("alpha\r\nbeta", false).unwrap();
        let baseline = runtime_score(&["alpha".into(), "beta".into()], "\u{0002}", 128, WIN_RATE_EVAL_RQ, 1).unwrap();
        let initial = session.progress();
        session.step(0).unwrap();
        assert_eq!(session.progress(), initial);
        session.step(37).unwrap();
        session.step(91).unwrap();
        let progress = session.progress();
        assert_eq!(progress.completed, 128);
        assert_eq!(progress.wins, baseline.wins);
        assert_eq!(progress.truncated, baseline.guard_exhausted);
    }

    #[test]
    fn score_test_counts_only_qualifying_updates_from_subjects() {
        let mut session = ScoreTestSession::new("alpha\nbeta", false).unwrap();
        let mut missing_caster = RunUpdate::new("[0]无来源", 0, 0, 10);
        missing_caster.has_caster = false;
        for update in [
            RunUpdate::new("[0]发起攻击", 0, 2, 10),
            RunUpdate::new("[0]发起攻击", 1, 2, 10),
            RunUpdate::new("[0]陪测动作", 2, 0, 10),
            RunUpdate::new("[0]召唤物动作", 4, 0, 10),
            RunUpdate::new("[0]受到[2]点伤害[s_dmg0]", 0, 0, 10),
            RunUpdate::new("[0]无分值动作", 0, 0, 0),
            RunUpdate::new("没有施法者占位符", 0, 0, 10),
            missing_caster,
        ] {
            session.count_update(update);
        }
        assert_eq!(
            session.progress().skills,
            vec![ScoreSkillFrequency {
                message: "[0]发起攻击".into(),
                count: 2,
                frequency: 200.0,
            }]
        );
    }

    #[test]
    fn score_test_rejects_separate_teams_and_directives() {
        for raw in [
            "",
            "alpha\n \nbeta",
            "alpha\n\nseed:123",
            "alpha\nSEED:123",
            "!test!\nalpha",
            "!\nalpha",
        ] {
            assert!(
                matches!(ScoreTestSession::new(raw, false), Err(CliApiError::InvalidInput(_))),
                "{raw:?}"
            );
        }
    }
}

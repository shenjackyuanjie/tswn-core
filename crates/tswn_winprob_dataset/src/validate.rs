use crate::{
    BattleRow, DatasetConfig, SampleRow, input, random,
    storage::{self, ShardReceipt},
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
use tswn_core::cli_api::battle::BattleStopReason;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub battles: usize,
    pub samples: usize,
    pub resolved: usize,
    pub truncated: usize,
    pub failed: usize,
    pub split_battles: BTreeMap<String, usize>,
    pub stop_reasons: BTreeMap<String, usize>,
    pub team_count_battles: BTreeMap<usize, usize>,
}

impl Summary {
    fn add(&mut self, other: Self) {
        self.battles += other.battles;
        self.samples += other.samples;
        self.resolved += other.resolved;
        self.truncated += other.truncated;
        self.failed += other.failed;
        for (key, count) in other.split_battles {
            *self.split_battles.entry(key).or_default() += count;
        }
        for (key, count) in other.stop_reasons {
            *self.stop_reasons.entry(key).or_default() += count;
        }
        for (key, count) in other.team_count_battles {
            *self.team_count_battles.entry(key).or_default() += count;
        }
    }
}

pub fn validate_dataset(out: &Path) -> Result<Summary> {
    let config: DatasetConfig = storage::read_json(&out.join("manifest.json"))?;
    ensure!(
        config.format_version == 1 && config.state_schema_version == tswn_core::runtime::model_state::MODEL_STATE_SCHEMA_VERSION,
        "不支持的数据版本"
    );
    ensure!(
        config.games_per_matchup > 0
            && config.battles_per_shard > 0
            && config.samples_per_game > 0
            && config.max_rounds > 0
            && config.eval_rq.is_finite(),
        "manifest 配置无效"
    );
    ensure!(!config.cases.is_empty(), "manifest 没有阵容");
    for case in &config.cases {
        ensure!(
            case.groups.len() >= 2 && case.groups.iter().all(|group| !group.is_empty()),
            "阵容队伍无效"
        );
        ensure!(case.matchup_id == input::matchup_id(&case.groups), "阵容哈希不匹配");
    }
    let total = config.cases.len().checked_mul(config.games_per_matchup).context("对局数溢出")?;
    let shard_count = total.div_ceil(config.battles_per_shard);
    let expected_dirs: BTreeSet<_> = (0..shard_count).map(|index| format!("shard-{index:06}")).collect();
    for entry in std::fs::read_dir(out)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("shard-") {
            ensure!(expected_dirs.contains(&name), "发现 manifest 之外的分片 {name}");
        }
    }
    let mut summary = Summary::default();
    for index in 0..shard_count {
        let (first, end) = crate::generate::range(&config, index, total);
        let dir = out.join(format!("shard-{index:06}"));
        let receipt = storage::check_receipt(&dir, first, end).with_context(|| format!("检查分片 {index}"))?;
        summary.add(validate_shard(&dir, &config, &receipt)?);
    }
    ensure!(summary.battles == total, "总对局数不匹配");
    Ok(summary)
}

pub(crate) fn validate_shard(dir: &Path, config: &DatasetConfig, receipt: &ShardReceipt) -> Result<Summary> {
    let mut summary = Summary::default();
    let mut battles = Vec::new();
    storage::read_rows::<BattleRow>(&dir.join("battles.parquet"), |row| {
        let expected_id = receipt.first_battle + battles.len();
        ensure!(
            row.battle_id == expected_id as u64 && expected_id < receipt.end_battle,
            "对局顺序或范围不正确"
        );
        ensure!(row.case_index == expected_id / config.games_per_matchup, "对局阵容编号不正确");
        let case = config.cases.get(row.case_index).context("无效阵容编号")?;
        ensure!(
            row.matchup_id == case.matchup_id && row.split == random::split(&case.matchup_id),
            "阵容切分不正确"
        );
        ensure!(
            row.seed == random::battle_seed(&config.seed, &case.matchup_id, row.battle_id),
            "战斗 seed 不正确"
        );
        ensure!(
            row.outcome.rounds_advanced <= config.max_rounds && row.outcome.frames_emitted <= row.outcome.rounds_advanced,
            "终止计数不正确"
        );
        match (row.outcome.stop_reason, row.outcome.winner_team_indices.as_slice()) {
            (BattleStopReason::Winner, [winner]) => {
                ensure!(*winner < case.groups.len(), "胜者编号越界");
                summary.resolved += 1;
            }
            (BattleStopReason::MaxRounds | BattleStopReason::NoProgress, []) => {
                summary.truncated += 1;
            }
            _ => anyhow::bail!("已决胜者必须唯一，截断不能携带胜者"),
        }
        if row.outcome.stop_reason == BattleStopReason::MaxRounds {
            ensure!(row.outcome.rounds_advanced == config.max_rounds, "max_rounds 终止计数不正确");
        }
        let eligible = row
            .outcome
            .frames_emitted
            .saturating_sub(usize::from(row.outcome.stop_reason == BattleStopReason::Winner));
        let expected_samples = if row.outcome.rounds_advanced == 0 {
            0
        } else {
            config.samples_per_game.min(eligible + 1)
        };
        ensure!(row.samples == expected_samples, "抽样数量不正确");
        summary.battles += 1;
        *summary.split_battles.entry(row.split.clone()).or_default() += 1;
        *summary.stop_reasons.entry(format!("{:?}", row.outcome.stop_reason)).or_default() += 1;
        *summary.team_count_battles.entry(case.groups.len()).or_default() += 1;
        battles.push(row);
        Ok(())
    })?;
    ensure!(battles.len() == receipt.battles, "对局行数与回执不匹配");
    let mut counts = vec![0; battles.len()];
    let mut previous: Option<(u64, usize)> = None;
    storage::read_rows::<SampleRow>(&dir.join("samples.parquet"), |row| {
        let index = (row.battle_id as usize).checked_sub(receipt.first_battle).context("样本对局越界")?;
        let battle = battles.get(index).context("样本对局越界")?;
        ensure!(
            row.matchup_id == battle.matchup_id && row.split == battle.split,
            "样本与对局切分不一致"
        );
        ensure!(
            row.winner_team_index == battle.outcome.winner_team_indices.first().copied(),
            "样本胜者标签不正确"
        );
        ensure!(
            row.state.input_teams.len() == config.cases[battle.case_index].groups.len(),
            "状态输入队伍数不一致"
        );
        ensure!(
            row.rounds_advanced <= battle.outcome.rounds_advanced && row.state.round == row.rounds_advanced as u64,
            "样本轮数不正确"
        );
        ensure!(row.state.world.winner_team.is_none(), "样本包含已决胜者");
        if battle.outcome.stop_reason == BattleStopReason::Winner {
            ensure!(row.rounds_advanced < battle.outcome.rounds_advanced, "不得采样已决终局");
        }
        let expected_progress = row.rounds_advanced as f64 / battle.outcome.rounds_advanced.max(1) as f64;
        ensure!(
            row.progress.is_finite() && (row.progress - expected_progress).abs() < 1e-12,
            "审计进度不正确"
        );
        match row.frame {
            None => ensure!(row.rounds_advanced == 0 && counts[index] == 0, "初始状态必须为本局第一行"),
            Some(frame) => {
                ensure!(counts[index] > 0, "本局缺少初始状态");
                ensure!(
                    frame.frame_index < battle.outcome.frames_emitted
                        && frame.rounds_advanced == row.rounds_advanced
                        && frame.round_index + 1 == row.rounds_advanced,
                    "帧边界不正确"
                );
            }
        }
        let key = (row.battle_id, row.rounds_advanced);
        ensure!(previous.is_none_or(|last| last < key), "样本重复或行序不正确");
        previous = Some(key);
        row.state.validate()?;
        counts[index] += 1;
        summary.samples += 1;
        Ok(())
    })?;
    ensure!(summary.samples == receipt.samples, "样本行数与回执不匹配");
    ensure!(
        counts.iter().zip(&battles).all(|(count, battle)| *count == battle.samples),
        "每局样本数量不匹配"
    );
    Ok(summary)
}

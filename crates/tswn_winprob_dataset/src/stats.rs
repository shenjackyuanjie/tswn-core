//! 数据集分布统计；只读取已提交分片，不改变数据。
//!
//! 统计口径必须与 `SampleRow`/`BattleRow` 的 schema 保持一致，因此放在生成器里而不是
//! 训练侧脚本：`validate` 已经依赖同一套类型和 `storage::read_rows`，这里复用同一条读取路径。

use crate::{
    BattleRow, DatasetConfig, SampleRow,
    storage::{self},
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// 按进度划分的评估桶；`progress` 只用于分析，不能进入模型特征。
const PROGRESS_BUCKETS: [(f64, &str); 5] = [
    (0.2, "0-20%"),
    (0.4, "20-40%"),
    (0.6, "40-60%"),
    (0.8, "60-80%"),
    (1.01, "80-100%"),
];

/// 整数序列的汇总；`histogram` 保留逐值计数，便于直接看长尾。
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueStats {
    pub count: usize,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub median: f64,
    pub p90: f64,
    pub p99: f64,
    pub histogram: BTreeMap<u64, usize>,
}

#[derive(Debug, Default, Clone)]
struct Series {
    values: Vec<f64>,
    histogram: BTreeMap<u64, usize>,
}

impl Series {
    fn push(&mut self, value: u64) {
        self.values.push(value as f64);
        *self.histogram.entry(value).or_default() += 1;
    }
    /// 连续量只保留分位数，不记录逐值直方图。
    fn push_continuous(&mut self, value: f64) { self.values.push(value); }
    fn finish(mut self) -> ValueStats {
        if self.values.is_empty() {
            return ValueStats::default();
        }
        self.values.sort_by(f64::total_cmp);
        let pick = |quantile: f64| -> f64 {
            let index = ((self.values.len() - 1) as f64 * quantile).round() as usize;
            self.values[index]
        };
        ValueStats {
            count: self.values.len(),
            min: self.values[0],
            max: self.values[self.values.len() - 1],
            mean: self.values.iter().sum::<f64>() / self.values.len() as f64,
            median: pick(0.5),
            p90: pick(0.9),
            p99: pick(0.99),
            histogram: self.histogram,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct BattleStats {
    pub total: usize,
    pub resolved: usize,
    pub truncated: usize,
    pub failed: usize,
    pub split: BTreeMap<String, usize>,
    pub stop_reasons: BTreeMap<String, usize>,
    pub team_count: BTreeMap<usize, usize>,
    /// 胜者按输入队伍索引统计；截断局不参与。
    pub winner_team_index: BTreeMap<String, usize>,
    pub rounds_advanced: ValueStats,
    pub frames_emitted: ValueStats,
    pub samples_per_battle: ValueStats,
    /// 截断局按队伍数量分布，用于判断截断是否集中在某些对局类型。
    pub truncated_by_team_count: BTreeMap<usize, usize>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleStats {
    pub total: usize,
    pub split: BTreeMap<String, usize>,
    /// 每局第一行的初始状态数量。
    pub initial: usize,
    pub progress: ValueStats,
    pub progress_buckets: BTreeMap<String, usize>,
    pub rounds_advanced: ValueStats,
    pub entity_count: ValueStats,
    pub alive_entity_count: ValueStats,
    pub entity_slot_count: ValueStats,
    pub state_entry_count: ValueStats,
    pub skill_count: ValueStats,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentStats {
    /// 稳定 1-based 技能 ID 频次；不是 fixed_lane 或 legacy key。
    pub skill_id: BTreeMap<u32, usize>,
    /// 状态载荷 kind 频次。
    pub payload_kind: BTreeMap<String, usize>,
    /// 实体模板 kind 频次，包含召唤物。
    pub player_kind: BTreeMap<u32, usize>,
    /// 内置 Boss 种类频次；键为 `BOSS_NAMES` 下标。
    pub boss_kind: BTreeMap<String, usize>,
    /// 阵营相等关系分组数量；只表达相等关系，不含阵营文本。
    pub clan_group: BTreeMap<u32, usize>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasetStats {
    pub battles: BattleStats,
    pub samples: SampleStats,
    pub content: ContentStats,
}

/// 汇总一个已完成数据集；要求每个分片都已提交。
pub fn collect(out: &Path) -> Result<DatasetStats> {
    let config: DatasetConfig = storage::read_json(&out.join("manifest.json")).context("读取 manifest")?;
    let total = config.cases.len() * config.games_per_matchup;
    let shard_count = total.div_ceil(config.battles_per_shard);
    let mut stats = DatasetStats::default();
    let mut samples_per_battle = Series::default();
    let mut progress = Series::default();
    let mut sample_rounds = Series::default();
    let mut entity_count = Series::default();
    let mut alive_entity_count = Series::default();
    let mut entity_slot_count = Series::default();
    let mut state_entry_count = Series::default();
    let mut skill_count = Series::default();
    let mut battle_rounds = Series::default();
    let mut battle_frames = Series::default();
    for index in 0..shard_count {
        let dir = out.join(format!("shard-{index:06}"));
        let mut rows = Vec::new();
        storage::read_rows::<BattleRow>(&dir.join("battles.parquet"), |row| {
            rows.push(row);
            Ok(())
        })
        .with_context(|| format!("读取分片 {index} 的对局表"))?;
        for row in rows {
            stats.battles.total += 1;
            *stats.battles.split.entry(row.split.clone()).or_default() += 1;
            *stats.battles.stop_reasons.entry(format!("{:?}", row.outcome.stop_reason)).or_default() += 1;
            let team_count = config.cases.get(row.case_index).map_or(0, |case| case.groups.len());
            *stats.battles.team_count.entry(team_count).or_default() += 1;
            match row.outcome.winner_team_indices.as_slice() {
                [winner] => {
                    stats.battles.resolved += 1;
                    *stats.battles.winner_team_index.entry(winner.to_string()).or_default() += 1;
                }
                [] => {
                    stats.battles.truncated += 1;
                    *stats.battles.truncated_by_team_count.entry(team_count).or_default() += 1;
                }
                _ => stats.battles.failed += 1,
            }
            battle_rounds.push(row.outcome.rounds_advanced as u64);
            battle_frames.push(row.outcome.frames_emitted as u64);
            samples_per_battle.push(row.samples as u64);
        }
        storage::read_rows::<SampleRow>(&dir.join("samples.parquet"), |row| {
            stats.samples.total += 1;
            *stats.samples.split.entry(row.split.clone()).or_default() += 1;
            if row.frame.is_none() {
                stats.samples.initial += 1;
            }
            progress.push_continuous(row.progress.clamp(0.0, 1.0));
            let bucket = PROGRESS_BUCKETS
                .iter()
                .find(|(upper, _)| row.progress < *upper)
                .map_or("80-100%", |(_, name)| *name);
            *stats.samples.progress_buckets.entry(bucket.into()).or_default() += 1;
            sample_rounds.push(row.rounds_advanced as u64);
            let state = &row.state;
            entity_count.push(state.entities.len() as u64);
            alive_entity_count.push(state.entities.iter().filter(|entity| entity.runtime.alive).count() as u64);
            entity_slot_count.push(state.entity_slot_count as u64);
            let mut entries = 0usize;
            let mut lanes = 0usize;
            for entity in &state.entities {
                entries += entity.states.len();
                lanes += entity.template.skills.lanes.len();
                for entry in &entity.states {
                    *stats.content.payload_kind.entry(entry.payload.kind.clone()).or_default() += 1;
                }
                *stats.content.player_kind.entry(entity.template.kind.0).or_default() += 1;
                *stats.content.clan_group.entry(entity.template.identity.clan_group as u32).or_default() += 1;
                if let Some(kind) = entity.template.identity.boss_kind {
                    *stats.content.boss_kind.entry(kind.to_string()).or_default() += 1;
                }
                for lane in &entity.template.skills.lanes {
                    *stats.content.skill_id.entry(lane.skill_id).or_default() += 1;
                }
            }
            state_entry_count.push(entries as u64);
            skill_count.push(lanes as u64);
            Ok(())
        })
        .with_context(|| format!("读取分片 {index} 的样本表"))?;
    }
    stats.battles.rounds_advanced = battle_rounds.finish();
    stats.battles.frames_emitted = battle_frames.finish();
    stats.battles.samples_per_battle = samples_per_battle.finish();
    stats.samples.progress = progress.finish();
    stats.samples.rounds_advanced = sample_rounds.finish();
    stats.samples.entity_count = entity_count.finish();
    stats.samples.alive_entity_count = alive_entity_count.finish();
    stats.samples.entity_slot_count = entity_slot_count.finish();
    stats.samples.state_entry_count = state_entry_count.finish();
    stats.samples.skill_count = skill_count.finish();
    Ok(stats)
}

/// 打印人类可读报告；完整数据用 `--json-out` 落盘。
pub fn print_report(stats: &DatasetStats, top: usize) {
    let battles = &stats.battles;
    let samples = &stats.samples;
    println!("# 数据集分布");
    println!();
    println!(
        "- 对局 {}：已决 {}，截断 {}，异常 {}",
        battles.total, battles.resolved, battles.truncated, battles.failed
    );
    println!("- 切分 {:?}", battles.split);
    println!("- 终止原因 {:?}", battles.stop_reasons);
    println!("- 队伍数量 {:?}", battles.team_count);
    println!("- 胜者（输入队伍索引） {:?}", battles.winner_team_index);
    println!("- 截断按队伍数 {:?}", battles.truncated_by_team_count);
    println!();
    println!("| 序列 | 数量 | 最小 | 中位 | 平均 | p90 | p99 | 最大 |");
    println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
    for (name, series) in [
        ("对局轮数", &battles.rounds_advanced),
        ("对局可见帧", &battles.frames_emitted),
        ("每局样本数", &battles.samples_per_battle),
        ("样本轮数", &samples.rounds_advanced),
        ("每样本实体数", &samples.entity_count),
        ("每样本存活实体", &samples.alive_entity_count),
        ("每样本实体槽", &samples.entity_slot_count),
        ("每样本状态条目", &samples.state_entry_count),
        ("每样本技能槽", &samples.skill_count),
    ] {
        println!(
            "| {name} | {} | {} | {} | {:.1} | {} | {} | {} |",
            series.count, series.min, series.median, series.mean, series.p90, series.p99, series.max
        );
    }
    println!();
    println!(
        "- 样本 {}（初始状态 {}），切分 {:?}",
        samples.total, samples.initial, samples.split
    );
    println!("- 进度桶 {:?}", samples.progress_buckets);
    println!();
    println!("## 内容频次");
    println!();
    println!("- 技能 ID（top {top}）：{:?}", top_of(&stats.content.skill_id, top));
    println!("- 载荷 kind：{:?}", stats.content.payload_kind);
    println!("- 模板 kind：{:?}", stats.content.player_kind);
    println!("- Boss kind：{:?}", stats.content.boss_kind);
    println!("- 阵营分组：{:?}", stats.content.clan_group);
}

fn top_of<K: std::fmt::Debug + Clone + Ord>(map: &BTreeMap<K, usize>, top: usize) -> Vec<(K, usize)> {
    let mut items: Vec<_> = map.iter().map(|(key, count)| (key.clone(), *count)).collect();
    items.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    items.truncate(top);
    items
}

/// `stats` 子命令参数。
#[derive(Debug, Clone, clap::Args)]
pub struct StatsArgs {
    #[arg(long)]
    pub out: PathBuf,
    /// 额外把完整统计写入该 JSON 文件。
    #[arg(long)]
    pub json_out: Option<PathBuf>,
    /// 打印频次列表时保留的条目数。
    #[arg(long, default_value_t = 20)]
    pub top: usize,
}

pub fn run(args: &StatsArgs) -> Result<()> {
    let stats = collect(&args.out)?;
    if let Some(path) = &args.json_out {
        storage::write_json(path, &stats)?;
    }
    print_report(&stats, args.top);
    Ok(())
}

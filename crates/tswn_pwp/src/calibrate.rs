//! 逐字段标量校准：只读已提交数据集，采集机制标量分布并拟合归一化常数。
//!
//! 输出是 `encoder-manifest.json` 的数值部分（每字段 `s_f`/`c_f` 与分位数），由 Rust 生成，
//! 训练侧只加载、不重新拟合。口径见 `docs/design/feature-encoder-spec.md` 第 5 节：
//! `s_f = max(1, Q50(|x|))`、`c_f = max(1, Q99(|x|))`，分位数按 `round((n-1)*q)` 取。
//!
//! 本轮只覆盖第 5 节"逐字段标量"里已冻结的第一批字段；分类、引用、bit、精确注册序与
//! 未进白名单的槽值不参与统计。缺字段时 encoder 必须返回 `MissingCalibration`，不允许默认 1。

use crate::{DatasetConfig, SampleRow, storage};
use anyhow::{Context, Result};
use clap::Args;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};
use tswn_core::runtime::{
    entity::CloneBuildData,
    model_state::{BattleModelState, ModelPayload, ModelTemplate},
};

#[derive(Debug, Clone, Args)]
pub struct CalibrateArgs {
    /// 已完成的胜率数据集目录。
    #[arg(long)]
    pub out: PathBuf,
    /// 只统计该切分的行。
    #[arg(long, default_value = "train")]
    pub split: String,
    /// 保留空标签行；默认排除，与训练口径一致。
    #[arg(long)]
    pub keep_unlabeled: bool,
    /// 校准结果 JSON 落盘路径；省略时只打印摘要。
    #[arg(long)]
    pub json_out: Option<PathBuf>,
    /// 打印每个字段一行摘要。
    #[arg(long)]
    pub print_fields: bool,
}

/// 单字段的分布与拟合常数。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationField {
    pub count: usize,
    pub min: f64,
    pub max: f64,
    pub p50: f64,
    pub p99: f64,
    pub abs_p50: f64,
    pub abs_p99: f64,
    pub s_f: f64,
    pub c_f: f64,
}

/// 校准结果；`fields` 的键是第 5 节的完整字段路径。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationReport {
    pub schema: String,
    pub schema_version: u32,
    pub state_schema_version: u32,
    pub format_version: u32,
    pub split: String,
    pub require_label: bool,
    pub samples_seen: usize,
    pub samples_selected: usize,
    /// 原输入摘要（来自数据集 manifest）。
    pub input_sha256: String,
    /// 生成器可执行文件摘要；同时约束引擎、schema、依赖版本与构建选项。
    pub executable_sha256: String,
    /// 选中行序列（分片号 + 片内行号）的摘要，用来证明这批常数是从哪些行拟合的。
    pub selected_rows_digest: String,
    /// 采集器身份。
    pub calibrator: String,
    pub fields: BTreeMap<String, CalibrationField>,
}

#[derive(Debug, Default)]
struct Scalars {
    values: BTreeMap<&'static str, Vec<f64>>,
}

impl Scalars {
    /// 非有限值不计入统计；`NaN`/`Inf` 由 encoder 在编码阶段报 `NonFiniteValue`。
    fn push(&mut self, path: &'static str, value: f64) {
        if value.is_finite() {
            self.values.entry(path).or_default().push(value);
        }
    }
    fn push_int(&mut self, path: &'static str, value: i64) { self.push(path, value as f64) }
    fn push_float(&mut self, path: &'static str, value: f64) { self.push(path, value) }
    fn push_count(&mut self, path: &'static str, value: usize) { self.push(path, value as f64) }

    fn finish(&mut self) -> BTreeMap<String, CalibrationField> {
        let mut fields = BTreeMap::new();
        for (path, values) in std::mem::take(&mut self.values) {
            if values.is_empty() {
                continue;
            }
            let mut absolute: Vec<f64> = values.iter().map(|value| value.abs()).collect();
            let mut sorted = values;
            sorted.sort_by(f64::total_cmp);
            absolute.sort_by(f64::total_cmp);
            let s_f = quantile(&absolute, 0.5).max(1.0);
            let c_f = quantile(&absolute, 0.99).max(1.0);
            fields.insert(
                path.to_owned(),
                CalibrationField {
                    count: sorted.len(),
                    min: sorted[0],
                    max: sorted[sorted.len() - 1],
                    p50: quantile(&sorted, 0.5),
                    p99: quantile(&sorted, 0.99),
                    abs_p50: quantile(&absolute, 0.5),
                    abs_p99: quantile(&absolute, 0.99),
                    s_f,
                    c_f,
                },
            );
        }
        fields
    }
}

/// 与 `stats::Series` 相同的取法：排序后 `round((n-1)*q)`。
fn quantile(sorted: &[f64], q: f64) -> f64 {
    let index = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[index]
}

/// 采集一个 state 里已冻结的第一批逐字段标量。
fn collect_state(state: &BattleModelState, scalars: &mut Scalars) {
    scalars.push_int("global.round", state.round as i64);
    scalars.push_count("global.entity_count", state.entities.len());
    scalars.push_count("global.entity_slot_count", state.entity_slot_count);
    scalars.push_count(
        "global.alive_entity_count",
        state.entities.iter().filter(|entity| entity.runtime.alive).count(),
    );
    scalars.push_count(
        "global.state_entry_count",
        state.entities.iter().map(|entity| entity.states.len()).sum(),
    );
    scalars.push_count(
        "global.skill_lane_count",
        state.entities.iter().map(|entity| entity.template.skills.lanes.len()).sum(),
    );
    scalars.push_count(
        "global.slot_entry_count",
        state.entities.iter().map(|entity| entity.slots.len()).sum(),
    );
    scalars.push_count("global.template_slot_count", state.template_slots.len());
    scalars.push_count("global.battle_slot_count", state.battle_slots.len());
    scalars.push_int("global.world.alive_group_count", state.world.alive_group_count as i64);
    scalars.push_int("global.world.round_pos", state.world.round_pos as i64);

    for entity in &state.entities {
        let runtime = &entity.runtime;
        for (path, value) in ENTITY_RUNTIME_INTS {
            scalars.push_int(path, value(runtime));
        }
        scalars.push_float("entity.runtime.at_boost", f64::from_bits(runtime.at_boost_bits));
        scalars.push_float("entity.runtime.attract", f64::from_bits(runtime.attract_bits));
        scalars.push_int("entity.runtime.move_state.speed_points", runtime.move_state.speed_points as i64);
        scalars.push_int("entity.runtime.charge.step", runtime.charge.step as i64);
        scalars.push_float("entity.runtime.accumulate.acc", runtime.accumulate.acc());
        scalars.push_float(
            "entity.runtime.accumulate.charge_bonus",
            f64::from_bits(runtime.accumulate.charge_bonus_bits),
        );
        if let Some(hide) = runtime.hide {
            scalars.push_int("entity.runtime.hide.level", hide.level as i64);
            scalars.push_int("entity.runtime.hide.agility", hide.agility as i64);
            scalars.push_int("entity.runtime.hide.defense", hide.defense as i64);
            scalars.push_int("entity.runtime.hide.resistance", hide.resistance as i64);
            scalars.push_float("entity.runtime.hide.attract", f64::from_bits(hide.attract_bits));
        }
        for link in &runtime.protect_from {
            scalars.push_int("entity.runtime.protect_from.level", link.level as i64);
        }
        if let Some(count) = runtime.protect_pre_defend_skill_count {
            scalars.push_int("entity.runtime.protect_pre_defend_skill_count", count as i64);
        }

        collect_template(&entity.template, scalars);
        for entry in &entity.states {
            scalars.push_int("state.priority", entry.priority as i64);
            collect_payload(&entry.payload, scalars);
        }
        for slot in &entity.slots {
            // 蓝图缓存槽里的模板与实体模板共用同一套 `template.*` 字段路径（模板轴同理）。
            if let Some(template) = &slot.template {
                collect_template(template, scalars);
            }
            // 第 3.2 节白名单里唯一作为数值消费的槽；其余槽是模板、实体引用或排除项。
            if slot.slot_id == ENTITY_SLOT_MINION_COUNTER {
                if let Some(value) = slot.u64_value {
                    scalars.push("slot.minion_counter", value as f64);
                }
            }
        }
    }
    for slot in state.template_slots.iter().chain(state.battle_slots.iter()) {
        if let Some(template) = &slot.template {
            collect_template(template, scalars);
        }
    }
}

/// 采集一个模板的数值字段；实体模板与槽内蓝图模板共用同一套字段路径。
fn collect_template(template: &ModelTemplate, scalars: &mut Scalars) {
    for (path, value) in TEMPLATE_INTS {
        scalars.push_int(path, value(template));
    }
    scalars.push_float("template.at_boost", f64::from_bits(template.at_boost_bits));
    scalars.push_float("template.attract", f64::from_bits(template.attract_bits));
    scalars.push_int(
        "template.reserved_player_ids_before_spawn",
        template.reserved_player_ids_before_spawn as i64,
    );
    scalars.push_int("template.move_state.speed_points", template.move_state.speed_points as i64);
    scalars.push_int(
        "template.identity.boss_action_prob_count",
        template.identity.boss_action_prob_count as i64,
    );
    scalars.push_int(
        "template.identity.boost_immune_threshold",
        template.identity.boost_immune_threshold as i64,
    );
    for immunity in &template.identity.immunity {
        scalars.push_int("template.identity.immunity.threshold", immunity.threshold as i64);
    }
    for lane in &template.skills.lanes {
        scalars.push_int("lane.level", lane.level as i64);
        scalars.push_int("lane.build_level", lane.build_level as i64);
        if let Some(boost) = &lane.boost {
            scalars.push_int("lane.boost.base", boost.base as i64);
            scalars.push_int("lane.boost.extra", boost.extra as i64);
        }
    }
    if let Some(clone_build) = &template.clone_build {
        collect_clone_build(clone_build, scalars);
    }
}

/// 默认注册表 entity 域 `core.entity.minion_counter` 的 `slot_id`（第 3.2 节白名单）。
const ENTITY_SLOT_MINION_COUNTER: u32 = 5;

const CLONE_ATTR_PATHS: [&str; 8] = [
    "template.clone_build.attrs.0",
    "template.clone_build.attrs.1",
    "template.clone_build.attrs.2",
    "template.clone_build.attrs.3",
    "template.clone_build.attrs.4",
    "template.clone_build.attrs.5",
    "template.clone_build.attrs.6",
    "template.clone_build.attrs.7",
];

const CLONE_WEAPON_PATHS: [&str; 8] = [
    "template.clone_build.weapon_attr_bonus.0",
    "template.clone_build.weapon_attr_bonus.1",
    "template.clone_build.weapon_attr_bonus.2",
    "template.clone_build.weapon_attr_bonus.3",
    "template.clone_build.weapon_attr_bonus.4",
    "template.clone_build.weapon_attr_bonus.5",
    "template.clone_build.weapon_attr_bonus.6",
    "template.clone_build.weapon_attr_bonus.7",
];

const CLONE_ADJUSTMENT_PATHS: [&str; 10] = [
    "template.clone_build.adjustments.max_hp",
    "template.clone_build.adjustments.attack",
    "template.clone_build.adjustments.magic",
    "template.clone_build.adjustments.wisdom",
    "template.clone_build.adjustments.speed",
    "template.clone_build.adjustments.defense",
    "template.clone_build.adjustments.resistance",
    "template.clone_build.adjustments.agility",
    "template.clone_build.adjustments.attr_sum",
    "template.clone_build.adjustments.atk_sum",
];

/// 分身构造参数；Boss 分支不在本轮映射范围，因此这里不处理。
fn collect_clone_build(clone_build: &CloneBuildData, scalars: &mut Scalars) {
    for (index, value) in clone_build.attrs.iter().enumerate() {
        scalars.push(CLONE_ATTR_PATHS[index], f64::from(*value));
    }
    for (index, value) in clone_build.weapon_attr_bonus.iter().enumerate() {
        scalars.push(CLONE_WEAPON_PATHS[index], f64::from(*value));
    }
    scalars.push_float("template.clone_build.name_factor", f64::from_bits(clone_build.name_factor_bits));
    scalars.push_float(
        "template.clone_build.child_name_factor",
        f64::from_bits(clone_build.child_name_factor_bits),
    );
    let adjustments = &clone_build.adjustments;
    let integers = [
        i64::from(adjustments.max_hp),
        i64::from(adjustments.attack),
        i64::from(adjustments.magic),
        i64::from(adjustments.wisdom),
        i64::from(adjustments.speed),
        i64::from(adjustments.defense),
        i64::from(adjustments.resistance),
        i64::from(adjustments.agility),
        adjustments.attr_sum,
        i64::from(adjustments.atk_sum),
    ];
    for (path, value) in CLONE_ADJUSTMENT_PATHS.iter().zip(integers) {
        scalars.push(path, value as f64);
    }
    scalars.push_float(
        "template.clone_build.adjustments.at_boost_delta",
        f64::from_bits(adjustments.at_boost_delta_bits),
    );
    scalars.push_float(
        "template.clone_build.adjustments.attract_delta",
        f64::from_bits(adjustments.attract_delta_bits),
    );
    if let Some(plan) = &clone_build.score_skill_boost_plan {
        // `initially_boosted_mask` 属于 bit 通道（第 14 节 field_class 3），不作为数值采集。
        for (index, boost) in plan.slot_boosts.iter().enumerate() {
            if let Some((first, second)) = boost {
                scalars.push(PLAN_SLOT_BOOST_PATHS[index * 2], f64::from(*first));
                scalars.push(PLAN_SLOT_BOOST_PATHS[index * 2 + 1], f64::from(*second));
            }
        }
    }
}

/// 分身评分计划的槽位强化数值；计划为 Some 且该槽元组存在时才采集（第 14 节 field_class 4–7）。
const PLAN_SLOT_BOOST_PATHS: [&str; 4] = [
    "template.clone_build.score_skill_boost_plan.slot_boosts.0.0",
    "template.clone_build.score_skill_boost_plan.slot_boosts.0.1",
    "template.clone_build.score_skill_boost_plan.slot_boosts.1.0",
    "template.clone_build.score_skill_boost_plan.slot_boosts.1.1",
];

/// 状态载荷数值；按第 8 节的 kind 词表逐分支采集，Boss 分支（12–16）不映射。
fn collect_payload(payload: &ModelPayload, scalars: &mut Scalars) {
    if let Some(value) = payload.fire_mag_half_steps {
        scalars.push_int("state.payload.fire_mag_half_steps", i64::from(value));
    }
    if let Some(ice) = &payload.ice {
        scalars.push_int("state.payload.ice.frozen_step", i64::from(ice.frozen_step));
    }
    if let Some(value) = payload.shield_value {
        scalars.push_int("state.payload.shield_value", i64::from(value));
    }
    if let Some(curse) = &payload.curse {
        scalars.push_int("state.payload.curse.prob", i64::from(curse.prob));
        scalars.push_int("state.payload.curse.multiply", i64::from(curse.multiply));
    }
    if let Some(poison) = &payload.poison {
        scalars.push_float("state.payload.poison.atp", f64::from_bits(poison.atp_bits));
        scalars.push_int("state.payload.poison.count", i64::from(poison.count));
    }
    if let Some(haste) = &payload.haste {
        scalars.push_int("state.payload.haste.faster", i64::from(haste.faster));
        scalars.push_int("state.payload.haste.effective_faster", i64::from(haste.effective_faster));
        scalars.push_int("state.payload.haste.step", i64::from(haste.step));
    }
    if let Some(berserk) = &payload.berserk {
        scalars.push_int("state.payload.berserk.step", i64::from(berserk.step));
    }
    if let Some(charm) = &payload.charm {
        scalars.push_int("state.payload.charm.step", i64::from(charm.step));
    }
    if let Some(slow) = &payload.slow {
        scalars.push_int("state.payload.slow.step", i64::from(slow.step));
    }
    if let Some(iron) = &payload.iron {
        scalars.push_int("state.payload.iron.protect", i64::from(iron.protect));
        scalars.push_int("state.payload.iron.step", i64::from(iron.step));
    }
}

/// 实体 runtime 的整数标量；顺序即输出字段顺序的来源。
const ENTITY_RUNTIME_INTS: &[(&str, fn(&tswn_core::runtime::model_state::ModelPlayerRuntime) -> i64)] = &[
    ("entity.runtime.hp", |runtime| runtime.hp as i64),
    ("entity.runtime.attack", |runtime| runtime.attack as i64),
    ("entity.runtime.magic", |runtime| runtime.magic as i64),
    ("entity.runtime.magic_point", |runtime| runtime.magic_point as i64),
    ("entity.runtime.wisdom", |runtime| runtime.wisdom as i64),
    ("entity.runtime.speed", |runtime| runtime.speed as i64),
    ("entity.runtime.defense", |runtime| runtime.defense as i64),
    ("entity.runtime.resistance", |runtime| runtime.resistance as i64),
    ("entity.runtime.agility", |runtime| runtime.agility as i64),
    ("entity.runtime.attr_sum", |runtime| runtime.attr_sum as i64),
    ("entity.runtime.atk_sum", |runtime| runtime.atk_sum as i64),
    ("entity.runtime.shield", |runtime| runtime.shield as i64),
];

/// 模板的整数标量。
const TEMPLATE_INTS: &[(&str, fn(&tswn_core::runtime::model_state::ModelTemplate) -> i64)] = &[
    ("template.max_hp", |template| template.max_hp as i64),
    ("template.attack", |template| template.attack as i64),
    ("template.magic", |template| template.magic as i64),
    ("template.magic_point", |template| template.magic_point as i64),
    ("template.wisdom", |template| template.wisdom as i64),
    ("template.speed", |template| template.speed as i64),
    ("template.defense", |template| template.defense as i64),
    ("template.resistance", |template| template.resistance as i64),
    ("template.agility", |template| template.agility as i64),
    ("template.attr_sum", |template| template.attr_sum as i64),
    ("template.atk_sum", |template| template.atk_sum as i64),
];

/// 采集数据集并输出校准 JSON；只读已提交分片。
pub fn run(args: &CalibrateArgs) -> Result<()> {
    let config: DatasetConfig = storage::read_json(&args.out.join("manifest.json")).context("读取 manifest")?;
    let total = config.cases.len() * config.games_per_matchup;
    let shard_count = total.div_ceil(config.battles_per_shard);
    let require_label = !args.keep_unlabeled;
    let mut scalars = Scalars::default();
    let mut rows_hasher = Sha256::new();
    let mut samples_seen = 0usize;
    let mut samples_selected = 0usize;
    for index in 0..shard_count {
        let dir = args.out.join(format!("shard-{index:06}"));
        let mut row_index = 0usize;
        storage::read_rows::<SampleRow>(&dir.join("samples.parquet"), |row| {
            let current = row_index;
            row_index += 1;
            samples_seen += 1;
            if row.split != args.split {
                return Ok(());
            }
            if require_label && row.winner_team_index.is_none() {
                return Ok(());
            }
            samples_selected += 1;
            rows_hasher.update((index as u64).to_le_bytes());
            rows_hasher.update((current as u64).to_le_bytes());
            collect_state(&row.state, &mut scalars);
            Ok(())
        })
        .with_context(|| format!("读取分片 {index} 的样本表"))?;
    }

    let fields = scalars.finish();
    let report = CalibrationReport {
        schema: "tswn-pwp/encoder-calibration".to_owned(),
        schema_version: 1,
        state_schema_version: config.state_schema_version,
        format_version: config.format_version,
        split: args.split.clone(),
        require_label,
        samples_seen,
        samples_selected,
        input_sha256: config.input_sha256.clone(),
        executable_sha256: config.executable_sha256.clone(),
        selected_rows_digest: format!("{:x}", rows_hasher.finalize()),
        calibrator: "tswn-pwp calibrate v1".to_owned(),
        fields,
    };

    if let Some(path) = &args.json_out {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("创建输出目录 {}", parent.display()))?;
        }
        std::fs::write(path, serde_json::to_string_pretty(&report)?).with_context(|| format!("写入 {}", path.display()))?;
    }

    println!(
        "校准完成：split={} 选中 {}/{} 行，字段 {} 个",
        report.split,
        report.samples_selected,
        report.samples_seen,
        report.fields.len()
    );
    if args.print_fields {
        for (path, field) in &report.fields {
            println!(
                "  {path}: n={} min={} max={} p50={} p99={} s_f={} c_f={}",
                field.count, field.min, field.max, field.p50, field.p99, field.s_f, field.c_f
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantile_uses_round_of_scaled_index() {
        let values: Vec<f64> = (1..=10).map(f64::from).collect();
        assert_eq!(quantile(&values, 0.0), 1.0);
        assert_eq!(quantile(&values, 0.5), 6.0);
        assert_eq!(quantile(&values, 0.99), 10.0);
    }

    #[test]
    fn finish_fits_from_absolute_quantiles_with_floor_one() {
        let mut scalars = Scalars::default();
        for value in [-8, -4, 0, 2, 6] {
            scalars.push_int("x", value);
        }
        let fields = scalars.finish();
        let field = &fields["x"];
        assert_eq!(field.count, 5);
        assert_eq!(field.min, -8.0);
        assert_eq!(field.max, 6.0);
        // 绝对值排序后为 0,2,4,6,8：Q50=4、Q99=8。
        assert_eq!(field.abs_p50, 4.0);
        assert_eq!(field.abs_p99, 8.0);
        assert_eq!(field.s_f, 4.0);
        assert_eq!(field.c_f, 8.0);
    }

    #[test]
    fn finish_clamps_small_scales_to_one_and_drops_non_finite() {
        let mut scalars = Scalars::default();
        scalars.push_float("tiny", 0.25);
        scalars.push_float("tiny", f64::NAN);
        scalars.push_float("tiny", f64::INFINITY);
        let fields = scalars.finish();
        let field = &fields["tiny"];
        assert_eq!(field.count, 1);
        assert_eq!(field.s_f, 1.0);
        assert_eq!(field.c_f, 1.0);
    }

    #[test]
    fn missing_field_is_absent_rather_than_defaulted() {
        let mut scalars = Scalars::default();
        scalars.push_int("present", 3);
        let fields = scalars.finish();
        assert!(fields.contains_key("present"));
        assert!(!fields.contains_key("absent"));
    }
}

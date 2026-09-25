//! FeatureEncoder 容量 profile 与每样本容量计量。
//!
//! `baseline-64` 是 [FeatureEncoder 规格](../../../docs/design/feature-encoder-spec.md) 第 4 节冻结的档位：
//! `E_max=64` 是**支持域选择**而不是机制上界——幻术与分身可以无限新增实体，审计结论见规格第 4 节
//! “实体数的机制上界”。因此冻结时把“超限即 `CapacityExceeded`、溢出率随统计报告”作为配套要求。
//!
//! 这里的计费按**上界**算：每槽按 3 条 X 记录（`slot_id` raw、至多一条有效值 raw、至多一条实体 ref）。
//! `scripts/measure_encoder_capacity.py` 用同一公式统计真实记录数，用来核对余量；两者口径不同，
//! 前者判“是否可能超限”，后者看“实际峰值”。
//!
//! 本模块是容量定义的**唯一权威**：`tswn_core::encoder` 的容量预检、`tswn_pwp` 的统计与
//! `scripts/measure_encoder_capacity.py` 的公式都以这里为准，不允许在别处复制常量。

use serde::Serialize;
use crate::runtime::model_state::{BattleModelState, ModelSkills, ModelTemplate};

/// 容量维度名；顺序与规格第 4 节的容量表一致。
pub const CAPACITY_DIMS: [&str; 9] = ["e", "t", "r", "h", "l", "s", "q", "v", "x"];

/// 一个冻结的容量档位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EncoderProfile {
    pub name: &'static str,
    pub e_max: usize,
    pub t_max: usize,
    pub r_max: usize,
    pub h_max: usize,
    pub l_max: usize,
    pub s_max: usize,
    pub q_max: usize,
    pub v_max: usize,
    pub x_max: usize,
}

impl EncoderProfile {
    pub fn limit(&self, dim: &str) -> usize {
        match dim {
            "e" => self.e_max,
            "t" => self.t_max,
            "r" => self.r_max,
            "h" => self.h_max,
            "l" => self.l_max,
            "s" => self.s_max,
            "q" => self.q_max,
            "v" => self.v_max,
            "x" => self.x_max,
            other => panic!("unknown encoder capacity dimension: {other}"),
        }
    }
}

/// 冻结档位；推导与依据见规格第 4 节的容量表。
pub const BASELINE_64: EncoderProfile = EncoderProfile {
    name: "baseline-64",
    e_max: 64,
    t_max: 32,
    r_max: 32,
    h_max: 512,
    l_max: 4096,
    s_max: 64,
    q_max: 512,
    v_max: 32768,
    x_max: 65536,
};

/// 单个样本的容量计量。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CapacityMeasure {
    pub e: usize,
    pub t: usize,
    pub r: usize,
    pub h: usize,
    pub l: usize,
    pub s: usize,
    pub q: usize,
    pub v: usize,
    pub x: usize,
}

impl CapacityMeasure {
    pub fn value(&self, dim: &str) -> usize {
        match dim {
            "e" => self.e,
            "t" => self.t,
            "r" => self.r,
            "h" => self.h,
            "l" => self.l,
            "s" => self.s,
            "q" => self.q,
            "v" => self.v,
            "x" => self.x,
            other => panic!("unknown encoder capacity dimension: {other}"),
        }
    }

    /// 按规格第 4 节的计费式统计一个 state；含死亡实体与蓝图模板。
    pub fn measure(state: &BattleModelState) -> Self {
        let mut measure = CapacityMeasure {
            e: state.entities.len(),
            t: state.input_teams.len(),
            r: state.world.team_roster.len().max(state.world.team_alive.len()),
            h: state.entities.len(),
            ..CapacityMeasure::default()
        };

        let mut lanes = 0usize;
        let mut lane_lists = 0usize;
        let mut deferred = 0usize;
        let mut clone_leaves = 0usize;
        let mut protect = 0usize;
        let mut assassinate = 0usize;
        for entity in &state.entities {
            let (entity_lanes, entity_lists, entity_deferred) = skill_counts(&entity.template.skills);
            lanes += entity_lanes;
            lane_lists += entity_lists;
            deferred += entity_deferred;
            clone_leaves += clone_plan_leaves(&entity.template);
            for slot in &entity.slots {
                let Some(template) = &slot.template else { continue };
                measure.h += 1;
                let (slot_lanes, slot_lists, slot_deferred) = skill_counts(&template.skills);
                lanes += slot_lanes;
                lane_lists += slot_lists;
                deferred += slot_deferred;
                clone_leaves += clone_plan_leaves(template);
            }
            measure.s += entity.states.len();
            measure.q += entity.slots.len();
            protect += entity.runtime.protect_from.len();
            if entity.runtime.assassinate.is_some() {
                assassinate += 1;
            }
        }
        measure.q += state.template_slots.len() + state.battle_slots.len();
        for slot in state.template_slots.iter().chain(state.battle_slots.iter()) {
            let Some(template) = &slot.template else { continue };
            measure.h += 1;
            let (slot_lanes, slot_lists, slot_deferred) = skill_counts(&template.skills);
            lanes += slot_lanes;
            lane_lists += slot_lists;
            deferred += slot_deferred;
            clone_leaves += clone_plan_leaves(template);
        }

        let mut world_lists = state.world.round_order.len() + state.world.flat_alive.len();
        world_lists += state.world.team_roster.iter().map(Vec::len).sum::<usize>();
        world_lists += state.world.team_alive.iter().map(Vec::len).sum::<usize>();
        let input_members = state.input_teams.iter().map(Vec::len).sum::<usize>();

        measure.l = lanes;
        // 计费一律用饱和算术：畸形输入不该让计数在 debug 下 panic、在 release 下回绕。
        measure.v = saturate(&[
            lane_lists,
            world_lists,
            3usize.saturating_mul(measure.s),
            protect,
            input_members,
            state.ice_release_events.len(),
            deferred,
            measure.e,
        ]);
        // raw：8e + 10h + 3s + 2q + 2；每槽再按至多一条实体 ref 计费。
        let raw = saturate(&[
            8usize.saturating_mul(measure.e),
            10usize.saturating_mul(measure.h),
            3usize.saturating_mul(measure.s),
            2usize.saturating_mul(measure.q),
            2,
        ]);
        measure.x = saturate(&[clone_leaves, protect, assassinate, measure.q, raw]);
        measure
    }
}

/// 饱和求和；用于容量计费，避免畸形输入造成加法溢出。
fn saturate(parts: &[usize]) -> usize { parts.iter().fold(0usize, |acc, part| acc.saturating_add(*part)) }

/// 单个模板的 lane 总数、五类 lane list 条目数与 deferred 条目数。
fn skill_counts(skills: &ModelSkills) -> (usize, usize, usize) {
    let lanes = skills.lanes.len();
    let lists = lanes
        + skills.merge_lane_order.len()
        + skills.active_order.len()
        + skills.pre_action_order.len()
        + skills.post_damage_order.len();
    (lanes, lists, skills.post_action_after_states.len())
}

/// 分身评分计划的 X 记录数：计划存在记 1，每个 `Some` 的 `slot_boosts[i]` 记 2。
fn clone_plan_leaves(template: &ModelTemplate) -> usize {
    let Some(clone_build) = &template.clone_build else { return 0 };
    let Some(plan) = &clone_build.score_skill_boost_plan else {
        return 0;
    };
    let mut count = 1;
    for boost in &plan.slot_boosts {
        if boost.is_some() {
            count += 2;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_profile_limits_are_self_consistent() {
        // Q_max 必须覆盖 7 个实体槽 × E_max + 3 个全局模板槽；S_max 至少等于 E_max。
        assert!(BASELINE_64.q_max >= 7 * BASELINE_64.e_max + 3);
        assert!(BASELINE_64.s_max >= BASELINE_64.e_max);
        // X 的上界式：V + 15H + 9E + 3Q + 3S + 2，按每槽 3 条计费。
        let bound = BASELINE_64.v_max
            + 15 * BASELINE_64.h_max
            + 9 * BASELINE_64.e_max
            + 3 * BASELINE_64.q_max
            + 3 * BASELINE_64.s_max
            + 2;
        assert!(BASELINE_64.x_max >= bound, "x_max={} bound={}", BASELINE_64.x_max, bound);
        // V 的规划下界：五类 lane list。
        assert!(BASELINE_64.v_max >= 5 * BASELINE_64.l_max);
    }

    #[test]
    fn limit_lookup_covers_every_dimension() {
        for dim in CAPACITY_DIMS {
            assert!(BASELINE_64.limit(dim) > 0, "{dim}");
        }
    }
}

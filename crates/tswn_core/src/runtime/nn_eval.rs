//! 神经网络局面估值：状态特征提取与推理。
//!
//! 特征只来自战斗状态（属性、技能、状态、回合与默认权重启发式摘要），不含任何名字/队伍
//! 标识，对未见名字的泛化只取决于状态空间覆盖。视角约定：第 0 方 = `team` 参数一侧，
//! 实体按有效阵营（魅惑感知）分组，双方各 NN_SLOTS_PER_TEAM 个槽位，超出丢弃。
//!
//! 布局（NN_FEATURE_DIMS = 6*2*73 + 8 = 884）：
//! - 每实体 48 标量 + 25 技能 one-hot（BuiltinActiveSkill::CORE）
//! - 全局 8：回合进度 / 双方存活数 / 双方可复活尸体数 / 双方血量比和 / 默认权重启发式值
//!
//! 启用：环境变量 `TSWN_EVAL_NN=1` 且权重已装载（nn_weights.rs 非空）时，
//! `search_position_value` 走网络路径，否则保持启发式。
use super::*;

mod nn_weights;

use std::sync::OnceLock;

pub const NN_SLOTS_PER_TEAM: usize = 6;
pub const NN_ENTITY_DIMS: usize = 48;
pub const NN_SKILL_DIMS: usize = BuiltinActiveSkill::CORE.len();
pub const NN_ENTITY_BLOCK: usize = NN_ENTITY_DIMS + NN_SKILL_DIMS;
pub const NN_GLOBAL_DIMS: usize = 8;
pub const NN_FEATURE_DIMS: usize = NN_SLOTS_PER_TEAM * 2 * NN_ENTITY_BLOCK + NN_GLOBAL_DIMS;

/// 与训练脚本 DEFAULT 一致：启发式摘要特征用固定默认权重，不读环境覆盖，
/// 保证训练与推理时该输入分布一致。
pub const NN_HEURISTIC_DEFAULTS: [f64; 36] = [
    12.0, 0.04, 0.153116288, 0.0993943736, 0.04, 0.587449632, 0.75, 0.25, 0.124457993, 0.5, 0.334896921, 0.4132613, 0.02, 0.08, 1.2, 0.25, 0.12, 4.0, 0.105811553, 0.06, 0.08,
    0.5, 8.0, 0.04, 12.0, 6.0, 0.0884436616, 0.08, 0.01, 60.3681443, 128.278657, 0.4466431, 1.3862735, 0.532987596, 0.70, 0.05,
];

static NN_ENABLED: OnceLock<bool> = OnceLock::new();

pub fn nn_available() -> bool { !nn_weights::NN_W1.is_empty() }

/// 环境开关与模型装载联合判定；进程内只解析一次。
pub fn nn_enabled() -> bool { *NN_ENABLED.get_or_init(|| std::env::var("TSWN_EVAL_NN").as_deref() == Ok("1") && nn_available()) }

static NN_HYBRID: OnceLock<bool> = OnceLock::new();

/// 混合模式开关：`TSWN_EVAL_NN_HYBRID=1` 时筛选阶段保持启发式估值，仅最终阶段用网络。
pub fn nn_hybrid_enabled() -> bool { *NN_HYBRID.get_or_init(|| std::env::var("TSWN_EVAL_NN_HYBRID").as_deref() == Ok("1")) }

// ---- 估值调用点采集（训练分布对齐用） ----

static NN_DUMP_ACTIVE: OnceLock<bool> = OnceLock::new();
static NN_DUMP_STRIDE: OnceLock<u64> = OnceLock::new();
static NN_DUMP_FINAL_ONLY: OnceLock<bool> = OnceLock::new();

fn nn_dump_active() -> bool { *NN_DUMP_ACTIVE.get_or_init(|| std::env::var("TSWN_DUMP_VALUE").as_deref() == Ok("1")) }

/// 每 N 次估值调用登记一次；1 = 全部登记。
fn nn_dump_stride() -> u64 {
    *NN_DUMP_STRIDE.get_or_init(|| {
        std::env::var("TSWN_DUMP_VALUE_STRIDE")
            .ok()
            .and_then(|value| value.parse().ok())
            .filter(|value| *value > 0)
            .unwrap_or(1)
    })
}

/// 只登记最终阶段（ValueStage::Final）的估值调用，跳过筛选阶段。
fn nn_dump_final_only() -> bool {
    *NN_DUMP_FINAL_ONLY.get_or_init(|| std::env::var("TSWN_DUMP_FINAL_ONLY").as_deref() == Ok("1"))
}

/// 估值的调用阶段，与数据文件里的阶段编号一致。
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ValueStage {
    Screening = 0,
    Final = 1,
    Tactical = 2,
}

fn nn_dump_search_only() -> bool {
    static SEARCH_ONLY: OnceLock<bool> = OnceLock::new();
    *SEARCH_ONLY.get_or_init(|| std::env::var("TSWN_DUMP_SEARCH_ONLY").as_deref() == Ok("1"))
}

thread_local! {
    static VALUE_DUMP: std::cell::RefCell<Vec<(CombatRuntime, usize, ValueStage)>> = std::cell::RefCell::new(Vec::new());
    static VALUE_DUMP_COUNT: std::cell::Cell<u64> = std::cell::Cell::new(0);
}

/// 清空线程本地估值状态缓冲（每场战斗开始前调用）。
pub fn nn_dump_clear() {
    VALUE_DUMP.with(|buffer| buffer.borrow_mut().clear());
    VALUE_DUMP_COUNT.with(|count| count.set(0));
}

/// 估值调用点登记（TSWN_DUMP_VALUE=1 时生效）：存储战场克隆（含 RNG 状态）、视角队伍
/// 与估值阶段。标签由采集方用该克隆继续原版策略推演到终局得到
/// （蒙特卡洛自标签，与真实战斗结局解耦，避免平行宇宙标签错配）。
pub fn nn_dump_push(runtime: &CombatRuntime, team: usize, stage: ValueStage) {
    if !nn_dump_active()
        || (nn_dump_final_only() && stage != ValueStage::Final)
        || (nn_dump_search_only() && stage == ValueStage::Screening)
    {
        return;
    }
    VALUE_DUMP_COUNT.with(|count| {
        let index = count.get();
        count.set(index + 1);
        if index % nn_dump_stride() != 0 {
            return;
        }
        VALUE_DUMP.with(|buffer| buffer.borrow_mut().push((runtime.clone(), team, stage)));
    });
}

/// 取出并清空线程本地缓冲（每场战斗结束后调用）。
pub fn nn_dump_drain() -> Vec<(CombatRuntime, usize, ValueStage)> {
    VALUE_DUMP.with(|buffer| buffer.borrow_mut().drain(..).collect())
}

/// 前向推理：ReLU → ReLU → tanh，输出 [-1,1] 的胜势估计；模型未装载返回 None。
pub fn nn_value(features: &[f32]) -> Option<f64> {
    let (h1n, h2n) = (nn_weights::NN_H1, nn_weights::NN_H2);
    if !nn_available() || features.len() != NN_FEATURE_DIMS || h1n == 0 || h2n == 0 {
        return None;
    }
    let (w1, b1, w2, b2, w3, b3) = (
        nn_weights::NN_W1,
        nn_weights::NN_B1,
        nn_weights::NN_W2,
        nn_weights::NN_B2,
        nn_weights::NN_W3,
        nn_weights::NN_B3,
    );
    if nn_weights::NN_INPUT_DIMS != NN_FEATURE_DIMS
        || w1.len() != NN_FEATURE_DIMS * h1n
        || b1.len() != h1n
        || w2.len() != h1n * h2n
        || b2.len() != h2n
        || w3.len() != h2n
        || b3.len() != 1
    {
        return None;
    }
    let mut h1 = vec![0.0_f32; h1n];
    // 权重为列主序（j 行 = 第 j 隐元的输入权重）：内层沿连续内存，便于向量化。
    for j in 0..h1n {
        let mut s = b1[j];
        let row = &w1[j * NN_FEATURE_DIMS..(j + 1) * NN_FEATURE_DIMS];
        for i in 0..NN_FEATURE_DIMS {
            s += features[i] * row[i];
        }
        h1[j] = s.max(0.0);
    }
    let mut h2 = vec![0.0_f32; h2n];
    for j in 0..h2n {
        let mut s = b2[j];
        let row = &w2[j * h1n..(j + 1) * h1n];
        for i in 0..h1n {
            s += h1[i] * row[i];
        }
        h2[j] = s.max(0.0);
    }
    let mut s = b3[0];
    for i in 0..h2n {
        s += h2[i] * w3[i];
    }
    Some(f64::from(s).tanh())
}

impl CombatRuntime {
    /// 以 `team` 为第 0 方视角提取神经网络输入特征（确定性、不推进随机流）。
    pub fn nn_features(&self, team: usize) -> Vec<f32> {
        let mut feats = vec![0.0_f32; NN_FEATURE_DIMS];
        let mut sides: [Vec<EntityIdx>; 2] = [Vec::new(), Vec::new()];
        for (id, entity) in self.entities.iter() {
            let effective = if entity.runtime.alive {
                self.plain_effective_team(id)
            } else {
                entity.runtime.team
            };
            sides[usize::from(effective != team)].push(id);
        }
        sides[0].sort_by_key(|id| id.0);
        sides[1].sort_by_key(|id| id.0);
        let mut global = [0.0_f32; NN_GLOBAL_DIMS];
        for (side, ids) in sides.iter().enumerate() {
            for (slot, &id) in ids.iter().enumerate() {
                if slot >= NN_SLOTS_PER_TEAM {
                    break;
                }
                let Some(entity) = self.entities.get(id) else { continue };
                let base = (side * NN_SLOTS_PER_TEAM + slot) * NN_ENTITY_BLOCK;
                self.nn_entity_features(entity, id, base, &mut feats, &mut global, side);
            }
        }
        let base_g = NN_SLOTS_PER_TEAM * 2 * NN_ENTITY_BLOCK;
        feats[base_g] = (self.round as f32 / 4096.0).min(1.0);
        feats[base_g + 1] = global[0] / NN_SLOTS_PER_TEAM as f32;
        feats[base_g + 2] = global[1] / NN_SLOTS_PER_TEAM as f32;
        feats[base_g + 3] = global[2] / NN_SLOTS_PER_TEAM as f32;
        feats[base_g + 4] = global[3] / NN_SLOTS_PER_TEAM as f32;
        feats[base_g + 5] = global[4] / NN_SLOTS_PER_TEAM as f32;
        feats[base_g + 6] = global[5] / NN_SLOTS_PER_TEAM as f32;
        feats[base_g + 7] = self.search_position_value_raw(team, Some(&NN_HEURISTIC_DEFAULTS)) as f32;
        feats
    }

    /// 单实体特征段（48 标量 + 25 技能位）与全局聚合。
    fn nn_entity_features(
        &self,
        entity: &EntityRecord,
        id: EntityIdx,
        base: usize,
        feats: &mut [f32],
        global: &mut [f32; NN_GLOBAL_DIMS],
        side: usize,
    ) {
        let r = &entity.runtime;
        let max_hp = i64::from(entity.template.max_hp.max(1));
        let hp = i64::from(r.hp.max(0));
        let alive = r.alive && r.hp > 0;
        let hp_frac = (hp as f32 / max_hp as f32).clamp(0.0, 1.0);
        // 0..31 基础标量
        feats[base] = f32::from(alive);
        feats[base + 1] = f32::from(r.is_minion());
        feats[base + 2] = f32::from(!alive);
        feats[base + 3] = f32::from(!alive && r.corpse == RuntimeCorpseKind::None && !r.is_combat_minion());
        feats[base + 4] = f32::from(alive && self.plain_effective_team(id) != r.team);
        feats[base + 5] = hp_frac;
        feats[base + 6] = (max_hp as f32 / 4096.0).min(16.0);
        feats[base + 7] = (hp as f32 / 4096.0).min(16.0);
        feats[base + 8] = (r.attack as f32 / 4096.0).min(16.0);
        feats[base + 9] = (r.defense as f32 / 4096.0).min(16.0);
        feats[base + 10] = (r.magic as f32 / 4096.0).min(16.0);
        feats[base + 11] = (r.resistance as f32 / 4096.0).min(16.0);
        feats[base + 12] = (r.speed as f32 / 1024.0).min(8.0);
        feats[base + 13] = (r.agility as f32 / 1024.0).min(8.0);
        feats[base + 14] = (r.wisdom as f32 / 1024.0).min(8.0);
        feats[base + 15] = (r.shield as f32 / max_hp as f32).clamp(0.0, 4.0);
        feats[base + 16] = (r.magic_point as f32 / 128.0).clamp(0.0, 1.0);
        feats[base + 17] = (r.move_state.speed_points as f32 / MOVE_POINT_THRESHOLD as f32).min(8.0);
        feats[base + 18] = (entity.effective_speed() as f32 / 1024.0).min(8.0);
        let boost = r.at_boost();
        let boost = if boost.is_finite() { boost.clamp(0.25, 16.0) } else { 1.0 };
        feats[base + 19] = ((boost.log2() + 2.0) / 6.0) as f32;
        feats[base + 20] = f32::from(r.charge.active && !r.charge.permanent);
        feats[base + 21] = f32::from(r.charge.permanent);
        feats[base + 22] = f32::from(r.accumulate.active);
        feats[base + 23] = f32::from(r.assassinate.is_some());
        feats[base + 24] = f32::from(r.assassinate.is_some_and(|a| a.break_on_damage));
        feats[base + 25] = f32::from(
            r.assassinate
                .is_some_and(|a| self.entities.get(a.target).is_some_and(|target| target.runtime.alive)),
        );
        feats[base + 26] = f32::from(r.counter.pending);
        feats[base + 27] = f32::from(r.counter_xuan_yue.pending);
        feats[base + 28] = f32::from(r.hide.is_some());
        feats[base + 29] = f32::from(r.protect_to.is_some());
        feats[base + 30] = (r
            .protect_from
            .iter()
            .filter(|link| {
                self.entities
                    .get(link.owner)
                    .is_some_and(|owner| owner.runtime.alive && owner.runtime.team == r.team)
            })
            .count()
            .min(3) as f32)
            / 3.0;
        // 31..44 状态残值（多条目取更强信号，护盾求和）
        for entry in entity.states.entries() {
            match entry.payload {
                StatePayload::FireMagHalfSteps(half_steps) if half_steps > 0 => {
                    let v = (half_steps.min(8) as f32) / 8.0;
                    feats[base + 31] = feats[base + 31].max(v);
                }
                StatePayload::Ice { frozen_step } => {
                    let v = (frozen_step.max(0) as f32 / MOVE_POINT_THRESHOLD as f32).min(2.0);
                    feats[base + 32] = feats[base + 32].max(v);
                }
                StatePayload::ShieldValue(value) if value > 0 => {
                    feats[base + 43] = (feats[base + 43] + (value as f32 / max_hp as f32).min(4.0)).min(4.0);
                }
                StatePayload::Curse { prob, multiply } => {
                    feats[base + 33] = feats[base + 33].max((prob.min(64) as f32) / 64.0);
                    feats[base + 34] = feats[base + 34].max((multiply.min(8) as f32) / 8.0);
                }
                StatePayload::Poison { atp_bits, count, .. } if count > 0 => {
                    let atp = f64::from_bits(atp_bits);
                    if atp.is_finite() {
                        feats[base + 35] = feats[base + 35].max(((atp.max(0.0) / 4096.0) as f32).min(1.0));
                    }
                    feats[base + 36] = feats[base + 36].max((count.min(3) as f32) / 3.0);
                }
                StatePayload::Haste { step, .. } if step > 0 => {
                    let v = (step.min(4) as f32) / 4.0;
                    feats[base + 37] = feats[base + 37].max(v);
                }
                StatePayload::Berserk { step } if step > 0 => {
                    let v = (step.min(4) as f32) / 4.0;
                    feats[base + 38] = feats[base + 38].max(v);
                }
                StatePayload::Charm { step, .. } => {
                    let v = (step.max(0).min(4) as f32) / 4.0;
                    feats[base + 39] = feats[base + 39].max(v);
                }
                StatePayload::Slow { step } if step > 0 => {
                    let v = (step.min(4) as f32) / 4.0;
                    feats[base + 40] = feats[base + 40].max(v);
                }
                StatePayload::Iron { protect, step } if step > 0 => {
                    feats[base + 41] = feats[base + 41].max((protect.min(256) as f32) / 256.0);
                    feats[base + 42] = feats[base + 42].max((step.min(4) as f32) / 4.0);
                }
                _ => {}
            }
        }
        // 44..48 装备与杂项
        feats[base + 44] = r.weapon_s11.as_ref().map_or(0.0, |weapon| weapon.level as f32 / 128.0);
        feats[base + 45] = f32::from(r.upgrade_active);
        feats[base + 46] = ((r.attract() / 32768.0).clamp(0.0, 2.0)) as f32;
        feats[base + 47] = (r.atk_sum as f32 / 4096.0).min(16.0);
        // 48..73 技能 one-hot
        for (k, skill) in BuiltinActiveSkill::CORE.iter().enumerate() {
            let has = entity.template.skills.active_order().iter().any(|&lane| {
                entity.template.skills.level_at(lane).unwrap_or(0) > 0
                    && self.builtin_active_skill(entity.template.skills.skills()[lane]) == Some(*skill)
            });
            feats[base + NN_ENTITY_DIMS + k] = f32::from(has);
        }
        // 全局聚合：存活数 / 可复活尸体数 / 血量比和
        if alive {
            global[side] += 1.0;
            global[4 + side] += hp_frac;
        } else if feats[base + 3] > 0.0 {
            global[2 + side] += 1.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn features_are_finite_deterministic_perspective_sensitive_and_rng_free() {
        let mut runner =
            RuntimeRunner::new_from_groups_with_seed(&[vec!["alpha".into()], vec!["beta".into()]], &["nn-test".into()]).unwrap();
        let rt = runner.runtime_mut();
        let f0 = rt.nn_features(0);
        let f1 = rt.nn_features(1);
        assert_eq!(f0.len(), NN_FEATURE_DIMS);
        assert!(f0.iter().all(|value| value.is_finite()));
        assert_eq!(f0, rt.nn_features(0));
        assert_ne!(f0, f1, "双方视角应重排实体块");
        let mut expected_rng = rt.rng.clone();
        for _ in 0..8 {
            assert_eq!(expected_rng.next_u8(), rt.rng.next_u8());
        }
        // 权重装载与否两态都验：有模型时推理应有限、确定且在 [-1,1]；无模型时返回 None。
        if nn_available() {
            let value = nn_value(&f0).expect("已装载模型应可推理");
            assert!(value.is_finite() && (-1.0..=1.0).contains(&value));
            assert_eq!(nn_value(&f0), nn_value(&rt.nn_features(0)));
        } else {
            assert_eq!(nn_value(&f0), None);
        }
    }
}

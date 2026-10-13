//! 状态条目、载荷与构造规则。
//!
//! [`StateEntry`] 保存 hook 和排序元数据，[`StatePayload`] 保存实际机制参数。
//! `legacy_order_key` 是兼容状态类别键，[`StateId`] 是扩展注册表 ID。
//! store 另行保存运行期注册游标；压缩状态和专用 runtime 字段与 entries 共同表达机制。

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
/// arena 中稳定的实体索引，允许预留 ID 空槽。
pub struct EntityIdx(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
/// 一个带载荷状态的登记信息；增删由 [`StateStore`] 维护计划缓存与注册游标。
pub struct StateEntry {
    pub legacy_order_key: u32,
    pub extension_state_id: Option<StateId>,
    pub hook_mask: ProcMask,
    pub priority: SkillPriority,
    pub registration_order: RegistrationOrder,
    pub payload: StatePayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CovidInfectionEntry {
    pub boss: EntityIdx,
    pub mutation: i32,
    pub days: i32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
/// 已实现状态的机制数据；步骤、来源、有效阵营与浮点 bits 均有各自语义。
pub enum StatePayload {
    #[default]
    None,
    FireMagHalfSteps(i32),
    Ice {
        frozen_step: i32,
    },
    ShieldValue(i32),
    Curse {
        prob: i32,
        multiply: i32,
    },
    Poison {
        caster: Option<u32>,
        target: Option<u32>,
        atp_bits: u64,
        count: i32,
    },
    Haste {
        faster: i32,
        effective_faster: i32,
        step: i32,
    },
    Berserk {
        step: i32,
    },
    Charm {
        group_id: usize,
        effective_team_idx: Option<usize>,
        source_team_idx: Option<usize>,
        target: Option<u32>,
        step: i32,
    },
    Slow {
        step: i32,
    },
    Iron {
        protect: i32,
        step: i32,
    },
    CovidBoss {
        mutation: i32,
    },
    CovidInfection {
        entries: SmallVec<[CovidInfectionEntry; 2]>,
        mutation_set: SmallVec<[i32; 4]>,
        recovered: bool,
    },
    SaitamaBoss {
        turns: i32,
        damages: i32,
        hitters: SmallVec<[EntityIdx; 8]>,
        minions: SmallVec<[EntityIdx; 8]>,
    },
    LazyBoss {
        at_boost_bits: u64,
    },
    LazyInfection {
        boss: EntityIdx,
    },
}

impl StateEntry {
    pub fn legacy(legacy_order_key: u32) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::default(),
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::None,
        }
    }

    pub fn fire_mag(legacy_order_key: u32, half_steps: i32) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::default(),
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::FireMagHalfSteps(half_steps),
        }
    }

    pub fn ice(legacy_order_key: u32, frozen_step: i32) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::default(),
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Ice { frozen_step },
        }
    }

    pub fn shield(legacy_order_key: u32, state_id: StateId, shield: i32, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_DEFEND,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::ShieldValue(shield),
        }
    }

    pub fn curse(legacy_order_key: u32, state_id: StateId, prob: i32, multiply: i32, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_DEFEND,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Curse { prob, multiply },
        }
    }

    pub fn poison(
        legacy_order_key: u32,
        state_id: StateId,
        caster: Option<u32>,
        target: Option<u32>,
        atp: f64,
        count: i32,
        priority: SkillPriority,
    ) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Poison {
                caster,
                target,
                atp_bits: atp.to_bits(),
                count,
            },
        }
    }

    pub fn haste(legacy_order_key: u32, state_id: StateId, faster: i32, step: i32, priority: SkillPriority) -> Self {
        Self::haste_with_effective_faster(legacy_order_key, state_id, faster, faster, step, priority)
    }

    pub fn haste_with_effective_faster(
        legacy_order_key: u32,
        state_id: StateId,
        faster: i32,
        effective_faster: i32,
        step: i32,
        priority: SkillPriority,
    ) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Haste {
                faster,
                effective_faster,
                step,
            },
        }
    }

    pub fn berserk(legacy_order_key: u32, step: i32) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::default(),
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Berserk { step },
        }
    }

    pub fn charm(
        legacy_order_key: u32,
        state_id: StateId,
        group_id: usize,
        effective_team_idx: Option<usize>,
        source_team_idx: Option<usize>,
        target: Option<u32>,
        step: i32,
        priority: SkillPriority,
    ) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Charm {
                group_id,
                effective_team_idx,
                source_team_idx,
                target,
                step,
            },
        }
    }

    pub fn slow(legacy_order_key: u32, state_id: StateId, step: i32, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Slow { step },
        }
    }

    pub fn iron(legacy_order_key: u32, state_id: StateId, protect: i32, step: i32, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Iron { protect, step },
        }
    }

    pub fn covid_boss(legacy_order_key: u32, mutation: i32) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::NONE,
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::CovidBoss { mutation },
        }
    }

    pub fn covid_infection(
        legacy_order_key: u32,
        state_id: StateId,
        boss: EntityIdx,
        mutation: i32,
        priority: SkillPriority,
    ) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::PRE_ACTION | ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::CovidInfection {
                entries: SmallVec::from_slice(&[CovidInfectionEntry { boss, mutation, days: 0 }]),
                mutation_set: SmallVec::from_slice(&[mutation]),
                recovered: false,
            },
        }
    }

    pub fn lazy_boss(legacy_order_key: u32, at_boost: f64) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::NONE,
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::LazyBoss {
                at_boost_bits: at_boost.to_bits(),
            },
        }
    }

    pub fn saitama_boss(legacy_order_key: u32, state_id: StateId, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_DEFEND,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::SaitamaBoss {
                turns: 0,
                damages: 0,
                hitters: SmallVec::new(),
                minions: SmallVec::new(),
            },
        }
    }

    pub fn lazy_infection(legacy_order_key: u32, state_id: StateId, boss: EntityIdx, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::PRE_ACTION | ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::LazyInfection { boss },
        }
    }

    pub fn fire_mag_value(&self) -> Option<f64> {
        match &self.payload {
            StatePayload::FireMagHalfSteps(half_steps) => Some(f64::from(*half_steps) * 0.5),
            StatePayload::None
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn shield_value(&self) -> Option<i32> {
        match &self.payload {
            StatePayload::ShieldValue(shield) => Some(*shield),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn haste_value(&self) -> Option<(i32, i32)> {
        match &self.payload {
            StatePayload::Haste { faster, step, .. } => Some((*faster, *step)),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn haste_runtime_value(&self) -> Option<(i32, i32, i32)> {
        match &self.payload {
            StatePayload::Haste {
                faster,
                effective_faster,
                step,
            } => Some((*faster, *effective_faster, *step)),
            _ => None,
        }
    }

    pub fn poison_value(&self) -> Option<(Option<u32>, Option<u32>, f64, i32)> {
        match &self.payload {
            StatePayload::Poison {
                caster,
                target,
                atp_bits,
                count,
            } => Some((*caster, *target, f64::from_bits(*atp_bits), *count)),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn charm_value(&self) -> Option<(usize, Option<usize>, Option<usize>, Option<u32>, i32)> {
        match &self.payload {
            StatePayload::Charm {
                group_id,
                effective_team_idx,
                source_team_idx,
                target,
                step,
            } => Some((*group_id, *effective_team_idx, *source_team_idx, *target, *step)),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn slow_value(&self) -> Option<i32> {
        match &self.payload {
            StatePayload::Slow { step } => Some(*step),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn ice_value(&self) -> Option<i32> {
        match &self.payload {
            StatePayload::Ice { frozen_step } => Some(*frozen_step),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn iron_value(&self) -> Option<(i32, i32)> {
        match &self.payload {
            StatePayload::Iron { protect, step } => Some((*protect, *step)),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. } => None,
            StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn priority_for_hook(&self, hook: ProcMask) -> SkillPriority {
        match &self.payload {
            StatePayload::Poison { .. } if hook.intersects(ProcMask::POST_ACTION) => SkillPriority(150),
            StatePayload::Haste { .. } | StatePayload::Charm { .. } | StatePayload::Slow { .. }
                if hook.intersects(ProcMask::POST_ACTION) =>
            {
                SkillPriority(210)
            }
            StatePayload::Iron { .. } if hook.intersects(ProcMask::POST_ACTION) => SkillPriority(210),
            StatePayload::CovidInfection { .. } if hook.intersects(ProcMask::PRE_ACTION | ProcMask::POST_ACTION) => {
                SkillPriority(1000)
            }
            StatePayload::LazyInfection { .. } if hook.intersects(ProcMask::PRE_ACTION | ProcMask::POST_ACTION) => {
                SkillPriority(1000)
            }
            _ => self.priority,
        }
    }

    pub fn positive_clear_message(&self, owner_alive: bool) -> Option<(i32, &'static str)> {
        match &self.payload {
            StatePayload::Haste { .. } if owner_alive => Some((300, "[1]从[疾走]中解除")),
            StatePayload::Iron { .. } => Some((400, "[1]的[铁壁]被打消了")),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn is_positive_state(&self) -> bool {
        match &self.payload {
            StatePayload::ShieldValue(shield) => *shield > 0,
            StatePayload::Haste { .. } => true,
            StatePayload::Iron { step, .. } => *step > 0,
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => false,
        }
    }
}

mod store;

pub use store::{CompressedLegacyState, StatePayloadKind, StateStore};

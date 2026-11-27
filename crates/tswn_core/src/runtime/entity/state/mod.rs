//! 状态条目、载荷与构造规则。
//!
//! [`StateEntry`] 保存 hook 和排序元数据，[`StatePayload`] 保存实际机制参数。
//! `legacy_order_key` 是兼容状态类别键，[`StateId`] 是扩展注册表 ID。
//! store 另行保存运行期注册游标；压缩状态和专用 runtime 字段与 entries 共同表达机制。

use super::*;

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

/// 一条新冠传染记录：来源 Boss、变异值与已发病天数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CovidInfectionEntry {
    pub boss: EntityIdx,
    pub mutation: i32,
    pub days: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registration_order: Option<u64>,
}

/// 新冠传染状态装箱载荷。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CovidInfectionState {
    pub entries: SmallVec<[CovidInfectionEntry; 2]>,
    pub mutation_set: SmallVec<[i32; 4]>,
    pub recovered: bool,
}

/// 一拳超人 Boss 状态装箱载荷。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SaitamaBossState {
    pub turns: i32,
    pub damages: i32,
    pub hitters: SmallVec<[EntityIdx; 8]>,
    pub minions: SmallVec<[EntityIdx; 8]>,
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
    /// 新冠传染状态载荷装箱存储，避免把热路径 StateEntry 撑大到 152 B。
    CovidInfection(Box<CovidInfectionState>),
    /// 一拳超人 Boss 状态载荷装箱存储，避免把热路径 StateEntry 撑大到 152 B。
    SaitamaBoss(Box<SaitamaBossState>),
    LazyBoss {
        at_boost_bits: u64,
    },
    LazyInfection {
        boss: EntityIdx,
    },
    MarioBoss {
        grow: i32,
        fire_level: u32,
    },
    /// 实验体「颅骨重击」附加的易伤；层数只表示持续回合，不提高倍率。
    SkullFracture {
        stacks: i32,
    },
    /// 实验体第三阶段轮换的无实体；受伤压到 1，不可净化。
    TestSubjectNoEntity,
}

impl StateEntry {
    /// 构造只带兼容键的空状态条目。
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

    /// 构造火焰精通状态，半步数乘以 0.5 得到倍率。
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

    /// 构造冰冻状态。
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

    /// 构造护盾状态，挂到 POST_DEFEND。
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

    /// 构造诅咒状态，挂到 POST_DEFEND。
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

    /// 构造中毒状态，挂到 POST_ACTION；攻击威力以 bits 保存。
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

    /// 构造疾走状态，挂到 POST_ACTION；有效加速初值与 faster 相同。
    pub fn haste(legacy_order_key: u32, state_id: StateId, faster: i32, step: i32, priority: SkillPriority) -> Self {
        Self::haste_with_effective_faster(legacy_order_key, state_id, faster, faster, step, priority)
    }

    /// 构造疾走状态并单独指定有效加速值。
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

    /// 构造狂暴状态。
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

    /// 构造魅惑状态，挂到 POST_ACTION。
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

    /// 构造缓慢状态，挂到 POST_ACTION。
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

    /// 构造铁壁状态，同时挂到 POST_DEFEND 与 POST_ACTION。
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

    /// 构造新冠 Boss 的变异标记状态，不挂 hook。
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

    /// 构造新冠传染状态，初始含一条来源记录。
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
            payload: StatePayload::CovidInfection(Box::new(CovidInfectionState {
                entries: SmallVec::from_slice(&[CovidInfectionEntry {
                    boss,
                    mutation,
                    days: 0,
                    registration_order: None,
                }]),
                mutation_set: SmallVec::from_slice(&[mutation]),
                recovered: false,
            })),
        }
    }

    /// 构造懒羊羊 Boss 的攻击加成状态，不挂 hook。
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

    /// 构造一拳超人 Boss 的饥饿计数状态，挂到 POST_DEFEND。
    pub fn saitama_boss(legacy_order_key: u32, state_id: StateId, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_DEFEND,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::SaitamaBoss(Box::new(SaitamaBossState {
                turns: 0,
                damages: 0,
                hitters: SmallVec::new(),
                minions: SmallVec::new(),
            })),
        }
    }

    /// 构造懒羊羊传染状态，挂到 PRE_ACTION 与 POST_ACTION。
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

    /// 马里奥成长状态对应 `SklMarioGet` 自身作为正向 meta 登记；没有 hook，
    /// 只保存蘑菇层数和火球术等级。复活次数不在此载荷内，离散清除后仍要保留。
    pub fn mario_boss(legacy_order_key: u32, grow: i32, fire_level: u32) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::NONE,
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::MarioBoss { grow, fire_level },
        }
    }

    /// 构造实验体易伤状态，挂到 POST_DEFEND 与 POST_ACTION。
    ///
    /// 层数只表示持续回合：目标每次行动后递减一层，归零时解除；倍率恒为 1.5。
    pub fn skull_fracture(legacy_order_key: u32, state_id: StateId, stacks: i32, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::SkullFracture { stacks },
        }
    }

    /// 构造实验体无实体状态，挂到 POST_DEFEND。
    ///
    /// 与铁壁同为减伤类，但没有步数：进入与解除完全由实验体自己的奇偶行动控制，
    /// 因此既不是正面状态也不能被净化打消。
    pub fn testsubject_no_entity(legacy_order_key: u32, state_id: StateId, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_DEFEND,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::TestSubjectNoEntity,
        }
    }

    /// 读取马里奥成长的蘑菇层数与火球术等级。
    pub fn mario_boss_value(&self) -> Option<(i32, u32)> {
        match &self.payload {
            StatePayload::MarioBoss { grow, fire_level } => Some((*grow, *fire_level)),
            _ => None,
        }
    }

    /// 读取火焰精通倍率；非该状态返回 None。
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
            | StatePayload::CovidInfection(_)
            | StatePayload::SaitamaBoss(_)
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. }
            | StatePayload::MarioBoss { .. }
            | StatePayload::SkullFracture { .. }
            | StatePayload::TestSubjectNoEntity => None,
        }
    }

    /// 读取护盾值；非护盾状态返回 None。
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
            | StatePayload::CovidInfection(_)
            | StatePayload::SaitamaBoss(_)
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. }
            | StatePayload::MarioBoss { .. }
            | StatePayload::SkullFracture { .. }
            | StatePayload::TestSubjectNoEntity => None,
        }
    }

    /// 读取疾走的 (加速, 步数)；非该状态返回 None。
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
            | StatePayload::CovidInfection(_)
            | StatePayload::SaitamaBoss(_)
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. }
            | StatePayload::MarioBoss { .. }
            | StatePayload::SkullFracture { .. }
            | StatePayload::TestSubjectNoEntity => None,
        }
    }

    /// 读取疾走的 (加速, 有效加速, 步数)；非该状态返回 None。
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

    /// 读取中毒的 (施法者, 目标, 攻击威力, 剩余计数)；非该状态返回 None。
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
            | StatePayload::CovidInfection(_)
            | StatePayload::SaitamaBoss(_)
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. }
            | StatePayload::MarioBoss { .. }
            | StatePayload::SkullFracture { .. }
            | StatePayload::TestSubjectNoEntity => None,
        }
    }

    /// 读取魅惑的 (组 ID, 有效队伍, 来源队伍, 目标, 步数)；非该状态返回 None。
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
            | StatePayload::CovidInfection(_)
            | StatePayload::SaitamaBoss(_)
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. }
            | StatePayload::MarioBoss { .. }
            | StatePayload::SkullFracture { .. }
            | StatePayload::TestSubjectNoEntity => None,
        }
    }

    /// 读取缓慢剩余步数；非该状态返回 None。
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
            | StatePayload::CovidInfection(_)
            | StatePayload::SaitamaBoss(_)
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. }
            | StatePayload::MarioBoss { .. }
            | StatePayload::SkullFracture { .. }
            | StatePayload::TestSubjectNoEntity => None,
        }
    }

    /// 读取冰冻步数；非该状态返回 None。
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
            | StatePayload::CovidInfection(_)
            | StatePayload::SaitamaBoss(_)
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. }
            | StatePayload::MarioBoss { .. }
            | StatePayload::SkullFracture { .. }
            | StatePayload::TestSubjectNoEntity => None,
        }
    }

    /// 读取铁壁的 (保护值, 步数)；非该状态返回 None。
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
            | StatePayload::CovidInfection(_)
            | StatePayload::SaitamaBoss(_)
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. }
            | StatePayload::MarioBoss { .. }
            | StatePayload::SkullFracture { .. }
            | StatePayload::TestSubjectNoEntity => None,
        }
    }

    /// 按目标 hook 计算该状态的生效优先级，未覆盖时返回登记优先级。
    pub fn priority_for_hook(&self, hook: ProcMask) -> SkillPriority {
        match &self.payload {
            StatePayload::Poison { .. } if hook.intersects(ProcMask::POST_ACTION) => SkillPriority(150),
            StatePayload::Haste { .. } | StatePayload::Charm { .. } | StatePayload::Slow { .. }
                if hook.intersects(ProcMask::POST_ACTION) =>
            {
                SkillPriority(210)
            }
            StatePayload::Iron { .. } if hook.intersects(ProcMask::POST_ACTION) => SkillPriority(210),
            // 易伤的 POST_DEFEND 优先级取登记值 10000，POST_ACTION 单独压到 1000，
            // 与旧实现的 post_defend_priority / post_action_priority 一致。
            StatePayload::SkullFracture { .. } if hook.intersects(ProcMask::POST_ACTION) => SkillPriority(1000),
            StatePayload::CovidInfection(_) if hook.intersects(ProcMask::PRE_ACTION | ProcMask::POST_ACTION) => {
                SkillPriority(1000)
            }
            StatePayload::LazyInfection { .. } if hook.intersects(ProcMask::PRE_ACTION | ProcMask::POST_ACTION) => {
                SkillPriority(1000)
            }
            _ => self.priority,
        }
    }

    pub fn is_boss_infection(&self, entities: &EntityArena) -> bool {
        use crate::runtime::profile::{PLAIN_COVID_INFECTION_STATE_KEY, PLAIN_LAZY_INFECTION_STATE_KEY};

        let from_boss = |source, name| {
            entities
                .get(source)
                .is_some_and(|source| source.runtime.flags.contains(PlayerKindFlags::BOSS) && source.template.name == name)
        };
        match (&self.payload, self.legacy_order_key) {
            (StatePayload::LazyInfection { boss }, PLAIN_LAZY_INFECTION_STATE_KEY) => from_boss(*boss, "lazy"),
            (StatePayload::CovidInfection(state), PLAIN_COVID_INFECTION_STATE_KEY) => {
                !state.entries.is_empty()
                    && state.entries.iter().all(|strain| {
                        entities
                            .get(strain.boss)
                            .is_some_and(|source| source.runtime.flags.contains(PlayerKindFlags::BOSS))
                    })
            }
            _ => false,
        }
    }

    pub fn plan_priority_for_hook(&self, hook: ProcMask, entities: &EntityArena) -> SkillPriority {
        use crate::runtime::profile::{PLAIN_COVID_INFECTION_STATE_KEY, PLAIN_LAZY_INFECTION_STATE_KEY};

        if hook.intersects(ProcMask::POST_ACTION) {
            let same_timed_queue = match self.legacy_order_key {
                PLAIN_LAZY_INFECTION_STATE_KEY | PLAIN_COVID_INFECTION_STATE_KEY => self.is_boss_infection(entities),
                _ => false,
            };
            if same_timed_queue {
                // 同一载体的两种 Boss 感染属于同一个行动后队列，不能用优先级打乱登记顺序。
                return SkillPriority(210);
            }
        }
        self.priority_for_hook(hook)
    }

    /// 正面状态解除时的 (排序键, 文案键)；无需播报时返回 None。
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
            | StatePayload::CovidInfection(_)
            | StatePayload::SaitamaBoss(_)
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. }
            | StatePayload::MarioBoss { .. }
            | StatePayload::SkullFracture { .. }
            | StatePayload::TestSubjectNoEntity => None,
        }
    }

    /// 0.5.5 中 `gT()` 返回负数的减益状态；马里奥复活时会静默清除它们。
    pub fn is_negative_state(&self) -> bool {
        matches!(
            &self.payload,
            StatePayload::FireMagHalfSteps(_)
                | StatePayload::Ice { .. }
                | StatePayload::Curse { .. }
                | StatePayload::Poison { .. }
                | StatePayload::Berserk { .. }
                | StatePayload::Charm { .. }
                | StatePayload::Slow { .. }
        )
    }

    /// 是否为正面状态（护盾、疾走、铁壁、马里奥成长）。
    pub fn is_positive_state(&self) -> bool {
        match &self.payload {
            StatePayload::ShieldValue(shield) => *shield > 0,
            StatePayload::Haste { .. } => true,
            StatePayload::Iron { step, .. } => *step > 0,
            StatePayload::MarioBoss { .. } => true,
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection(_)
            | StatePayload::SaitamaBoss(_)
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. }
            | StatePayload::SkullFracture { .. }
            | StatePayload::TestSubjectNoEntity => false,
        }
    }
}

mod store;

pub use store::{CompressedLegacyState, StatePayloadKind, StateStore};

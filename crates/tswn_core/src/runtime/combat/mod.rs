//! Runtime 的机制执行器；子模块按行动、受击、效果与生命周期划分。
//!
//! [`CombatRuntime`] 组合实体、世界、注册表、handler、槽、队列和 RNG。
//! 调度器选行动，hook 读取当前数据并修改受击值或入队，效果提交后同步世界与消息。
//! 各攻击与状态效果使用各自分支，沿具体技能入口可查看完整的伤害与状态链。
//! 普通应用通过 [`super::RuntimeRunner`] 或规范 BattleSession 使用本执行器。

use super::*;
use std::sync::Arc;

#[cfg(feature = "cards")]
mod abilities;
mod attack;
pub(crate) mod boss_quilt;
pub(crate) mod boss_smdd;
mod boss_squall;
mod boss_thanos;
mod boss_wheel;
mod boss_xjy;
mod boss_yi;
mod defense;
mod effects;
mod infection;
mod lifecycle;
mod passive;
mod round;
mod skills_boss;
mod skills_control;
mod skills_debuff;
mod skills_lifecycle;
mod skills_offense;
mod skills_team;
mod skills_weapon;

/// 截断后仅对正向余量进一，保留 `ceil() as i32` 的饱和及 NaN 规则。
#[inline]
fn ceil_damage_to_i32(value: f64) -> i32 {
    let truncated = value as i32;
    truncated.saturating_add(i32::from(value > f64::from(truncated)))
}

#[derive(Debug, Clone)]
/// 单回合结果：分别记录行动、可选可见帧与当前胜者。
pub struct RoundOutcome {
    pub action: Option<ActionPlan>,
    pub frame: Option<RuntimeFrame>,
    pub winner_team: Option<usize>,
}

/// 调度阶段选中的内置主动技能，附普攻/技能共用的固定 lane 编号。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedBuiltinSkill {
    pub skill: BuiltinActiveSkill,
    pub fixed_lane: usize,
}

/// 内置主动技能的已选目标；常规上限为地震的 6 个目标，直接保存在栈内。
pub type PreparedTargetList = smallvec::SmallVec<[EntityIdx; 8]>;

/// 一个内置主动技能的完整行动计划：选中的技能加上由技能 `aa` 规则选出的目标列表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedBuiltinSkillAction {
    pub selected: SelectedBuiltinSkill,
    pub targets: PreparedTargetList,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Boss `k4` 扫描选中的专属或通用主动技能；`targets` 由技能自身的 `aa` 选出。
pub struct PreparedBossSkillAction {
    pub selected: crate::namerena::BossActionSkill,
    pub targets: PreparedTargetList,
}

/// 普通攻击命中时附加的伤害效果；新冠（covid）与懒羊羊（lazy）变体记录传染源与变异值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlainAttackOnDamage {
    None,
    Absorb,
    Berserk,
    Curse,
    Fire(u32),
    Ice,
    Poison,
    CovidDefaultSpread { mutation: i32 },
    CovidSpread { boss: EntityIdx, mutation: i32 },
    LazyDefaultSpread,
}

/// 普通角色在本回合准备执行的行动，由 [`super::scheduler`] 选出后交给结算逻辑。
///
/// `ThanosSnap` 只标记是否发动，实际目标在结算时按原版顺序与死亡链交错抽取。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedPlainAction {
    BasicAttack {
        target: EntityIdx,
        use_magic: bool,
        amount: i32,
    },
    ForcedAttack {
        target: EntityIdx,
        amount: i32,
    },
    Saitama {
        target: Option<EntityIdx>,
    },
    Conan {
        targets: PreparedTargetList,
    },
    /// 灭霸响指：`targets` 恒为空标记（返回 `None` 表示本次不响指），
    /// 实际目标在结算阶段按原版顺序与死亡链交错抽取。
    ThanosSnap {
        targets: PreparedTargetList,
    },
    WheelExchange {
        target: EntityIdx,
    },
    BuiltinSkill(PreparedBuiltinSkillAction),
    BossSkill(PreparedBossSkillAction),
}

#[derive(Debug, Clone)]
/// 一场战斗的完整可变机制容器；跨语言显示数据由会话层投影。
pub struct CombatRuntime {
    pub entities: EntityArena,
    pub world: WorldArena,
    pub scheduler: PhaseScheduler,
    pub effects: EffectQueue,
    /// 注册表与 handler 在战斗试算中只读；克隆战场时共享，修改时写时复制。
    pub effect_handlers: Arc<EffectHandlers>,
    pub skill_handlers: Arc<SkillHandlers>,
    pub state_handlers: Arc<StateHandlers>,
    pub replay_renderers: Arc<ReplayRenderers>,
    pub show_renderers: Arc<ShowRenderers>,
    /// 固定记录科研 Boss 的实体槽，普通对局不扫描所有实体。
    pub(crate) squall_bosses: Vec<EntityIdx>,
    pub scratch: BattleScratch,
    pub template_slots: TemplateSlotStorage,
    pub slots: BattleSlotStorage,
    pub registry: Arc<ExtensionRegistry>,
    pub rng: RC4,
    /// 外部行动脚本与暂停标记；缺省时执行链与自动模式一致。
    pub(crate) control: BattleControl,
    #[cfg(not(feature = "no_debug"))]
    pub trace: Option<RuntimeTrace>,
    pub round: u64,
}

#[cfg(test)]
mod rounding_tests {
    use super::ceil_damage_to_i32;

    #[test]
    fn rounding_damage_matches_ceil_with_saturation() {
        for value in [
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NAN,
            -0.0,
            0.0,
            f64::EPSILON,
            -f64::EPSILON,
            -1.5,
            -1.0,
            1.0,
            1.5,
            f64::from(i32::MIN) - 0.5,
            f64::from(i32::MIN) + 0.5,
            f64::from(i32::MAX) - 0.5,
            f64::from(i32::MAX) + 0.5,
        ] {
            assert_eq!(ceil_damage_to_i32(value), value.ceil() as i32, "value={value}");
        }
    }
}

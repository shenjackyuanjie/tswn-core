//! Runtime 的机制执行器；子模块按行动、受击、效果与生命周期划分。
//!
//! [`CombatRuntime`] 组合实体、世界、注册表、handler、槽、队列和 RNG。
//! 调度器选行动，hook 读取当前数据并修改受击值或入队，效果提交后同步世界与消息。
//! 各攻击与状态效果使用各自分支，沿具体技能入口可查看完整的伤害与状态链。
//! 普通应用通过 [`super::RuntimeRunner`] 或规范 BattleSession 使用本执行器。
//!
//! # 命名约定
//!
//! **目录表达机制类别，文件名表达具体对象**，函数名表达**角色与动作**，三者各管一个维度：
//!
//! - 顶层无前缀文件（`round` / `attack` / `defense` / `effects` / `lifecycle` /
//!   `passive` / `infection`，以及随 `cards` feature 编译的 `abilities`）是执行阶段或
//!   横切关注点，不属于下面两族。
//! - `boss/`：单只 Boss 的专属机制，文件名就是具体 Boss（`conan` / `mario` /
//!   `yuri` / `thanos` / `wheel` / `slime` / `tiger` / `fish` / `quilt` / `smdd` /
//!   `squall` / `xjy` / `yi`），配套回归放在同目录的 `<boss>_tests.rs`。
//! - `skills/`：技能族，文件名是技能类别（`offense` / `control` / `team` /
//!   `lifecycle` / `debuff` / `weapon` / `summon` / `zombie` / `assassinate`）。
//!
//! `skills/boss.rs` 与 `boss/` 目录是**不同层次**，别混淆：`skills/boss.rs` 承载
//! Boss 技能的三个 match 派发（select / prepare / probability）与
//! `drain_plain_boss_skill_into`，是「技能 → Boss 技能」的派发层；`boss/` 是每只
//! Boss 的专属实现。
//!
//! 函数名里的 `plain_` / `boss_` 是**角色**前缀（普通角色对 Boss），与文件/目录的机制
//! 类别无关，所以 `skills/control.rs` 里会出现 `drain_plain_ice_skill_into`。
//!
//! 两个高频动词的边界：
//!
//! - `drain_*_into`：排空累积队列，例如 `drain_plain_update_end_into` 先
//!   `std::mem::take` 再逐条处理。
//! - `run_*_into`：执行一个具体 handler，并把结果追加进 updates。
//!
//! 两者都接收 `&mut RunUpdates`。`_into` 统一表示「输出写入 RunUpdates」；
//! `apply_*_on_damage` 一族走的是「触发条件」命名轴（on_damage）而不是输出目标轴，
//! 因此不带 `_into`。
//!
//! 边界：少数编码 / 转换入口的 `_into` 表示「结果写入调用方给出的目标缓冲区」，
//! 目标不是 `RunUpdates`，例如 `crate::encoder::encode::encode_into` 写
//! `EncodedBatch`、`crate::namerena::input::raw_namerena_to_id_name_into` 写 `String`。
//! 它们同属「输出目标」轴，只是落到调用方缓冲，因此保留 `_into`，不按 RunUpdates 变体处理。

use super::*;

#[cfg(feature = "cards")]
mod abilities;
mod attack;
pub mod boss;
mod defense;
mod effects;
mod infection;
mod lifecycle;
mod passive;
mod round;
mod skills;

pub use skills::assassinate::PlainSkillPreActionOutcome;

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
    /// 红魔乡专属命中回调。
    Disperse,
    RosmonticBurn,
    RosmonticAbsorbFifth,
    Berserk,
    Curse,
    Fire(u32),
    Ice,
    Poison,
    CovidDefaultSpread {
        mutation: i32,
    },
    CovidSpread {
        boss: EntityIdx,
        mutation: i32,
    },
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

/// 一场战斗内的临时行动标记；复位时清空，不跨对局保留。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BattleScratch {
    pub selected_actor_round: u64,
    /// 搜索逐回合推演时的临时状态；普通对局为 None。
    pub(crate) search_turn: Option<crate::runtime::search::SearchTurn>,
}

impl BattleScratch {
    /// 复位临时游标，避免把上一局的选中回合带入下一局。
    pub fn clear(&mut self) {
        self.selected_actor_round = 0;
        self.search_turn = None;
    }
}
#[derive(Debug, Clone)]
/// 一场战斗的完整可变机制容器；跨语言显示数据由会话层投影。
pub struct CombatRuntime {
    pub entities: EntityArena,
    pub world: WorldArena,
    pub scheduler: PhaseScheduler,
    pub effects: EffectQueue,
    pub effect_handlers: EffectHandlers,
    pub skill_handlers: SkillHandlers,
    pub state_handlers: StateHandlers,
    pub replay_renderers: ReplayRenderers,
    pub show_renderers: ShowRenderers,
    /// 固定记录科研 Boss 的实体槽，普通对局不扫描所有实体。
    pub squall_bosses: Vec<EntityIdx>,
    pub scratch: BattleScratch,
    pub template_slots: TemplateSlotStorage,
    pub slots: BattleSlotStorage,
    pub registry: ExtensionRegistry,
    pub rng: RC4,
    /// 外部行动脚本与暂停标记；缺省时执行链与自动模式一致。
    pub control: BattleControl,
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

//! # 技能系统 (skill)
//!
//! 本模块定义技能系统的基础 trait、技能分发和技能槽管理。
//!
//! ## 子模块
//!
//! - **`act`** — 主动技能（吸血、火球、冰冻等共计 ~27 种）
//! - **`skl`** — 被动/防御技能（反击、防御、隐匿等共计 ~13 种）
//! - **`store`** — [`store::SkillStorage`]，管理玩家当前装备的技能列表及各阶段触发器
//!
//! ## 技能注册流程类型
//!
//! 技能可以在以下流程阶段注册并触发：
//!
//! | 流程阶段          | 说明                              |
//! |-------------------|-----------------------------------|
//! | `UpdateState`     | 每回合刷新属性快照时            |
//! | `PreStep`         | 行动前（移动点数计算）            |
//! | `PreAction`        | 行动前（目标选择前）             |
//! | `PostAction`       | 行动后                          |
//! | `PreDefend`        | 被攻击前（可修改 atp 或截断伤害） |
//! | `PostDefend`       | 被攻击后（可修改实际伤害值）       |
//! | `PostDamage`       | 造成伤害后                        |
//! | `PostDeath`        | 死亡时                          |
//! | `PostKill`         | 击杀时                          |
//!
//! ## 技能目标域
//!
//! | 目标域        | 说明                          |
//! |---------------|-------------------------------|
//! | `EnemyAlive`   | 敌方存活玩家                  |
//! | `AllyAlive`    | 同队存活玩家（含自身）         |
//! | `AllyAny`      | 同队全部玩家（含已死亡）         |
//! | `AllyDead`     | 同队已死亡玩家                |
//! | `SelfOnly`     | 仅自身                        |
//! | `AllAlive`     | 全场存活玩家（可能跨队伍）      |

use std::any::type_name_of_val;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{
    Arc, OnceLock, RwLock,
    atomic::{AtomicBool, Ordering as AtomicOrdering},
};

use crate::engine::storage::Storage;
use crate::engine::update::RunUpdates;
use crate::player::{OnDamageFunc, PlrId};
use crate::rc4::RC4;
use foldhash::HashMap as FastHashMap;

pub mod act;
pub mod skl;
pub mod store;

pub use act::{
    absorb, accumulate, assassinate, berserk, charge, charm, clone, critical, curse, disperse, exchange, fire, half, haste, heal,
    ice, iron, poison, quake, rapid, revive, shadow, slow, summon, thunder,
};
pub use skl::{corpse, counter, defend, hide, merge, none, protect, reflect, reraise, shield, upgrade, zombie};

pub type SkillFactory = fn() -> Box<dyn SkillTrait>;

/// DIY 技能加成类型。
///
/// 用于精确描述一个技能的最终等级是如何构成的，
/// 在分身后克隆体重建时能够正确计算衰减下限。
///
/// # 变体
///
/// | 变体 | 内联格式示例 | 说明 |
/// |------|-------------|------|
/// | `Normal(lv)` | `"sklfire":5` | 普通技能，无特殊加成 |
/// | `SlotBoost { base, boost }` | `"sklfire":"40+30"` | 末尾座位加成，最终 = base + boost |
/// | `LastBoost(base)` | `"sklfire":"2*40"` | 末尾主动技翻倍，最终 = base × 2 |
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillBoost {
    /// 普通技能，无特殊加成。最终等级 = 指定值。
    Normal(u32),
    /// 末尾座位加成：最终等级 = base + boost。
    SlotBoost {
        /// 基础熟练度
        base: u32,
        /// 座位加成值
        boost: u32,
    },
    /// 末尾主动技翻倍：最终等级 = base × 2。
    LastBoost(u32),
}

impl SkillBoost {
    /// 计算最终等级（加成后的展示等级）。
    pub fn final_level(&self) -> u32 {
        match self {
            Self::Normal(lv) => *lv,
            Self::SlotBoost { base, boost } => base + boost,
            Self::LastBoost(base) => base * 2,
        }
    }

    /// 计算基础等级（加成前的原始熟练度）。
    pub fn base_level(&self) -> u32 {
        match self {
            Self::Normal(lv) => *lv,
            Self::SlotBoost { base, .. } => *base,
            Self::LastBoost(base) => *base,
        }
    }

    /// 根据当前衰减后的最终等级，反推衰减后的基础等级。
    ///
    /// - `Normal`: 基础 = 最终（无分离）
    /// - `SlotBoost`: 基础 = max(最终 - boost, 1)
    /// - `LastBoost`: 基础 = 最终 / 2
    ///
    /// 用于 clone 重建时计算衰减下限。
    pub fn decayed_base_from_level(&self, current_level: u32) -> u32 {
        match self {
            Self::Normal(_) => current_level,
            Self::SlotBoost { boost, .. } => current_level.saturating_sub(*boost).max(1),
            Self::LastBoost(_) => current_level / 2,
        }
    }

    /// 根据衰减后的基础等级，重新计算加成后的最终等级。
    ///
    /// 用于 clone 重建时恢复 boost。
    pub fn final_level_from_decayed_base(&self, decayed_base: u32) -> u32 {
        match self {
            Self::Normal(_) => decayed_base,
            Self::SlotBoost { boost, .. } => decayed_base.saturating_add(*boost),
            Self::LastBoost(_) => decayed_base.saturating_mul(2),
        }
    }

    /// 从字符串解析 `SkillBoost`。
    ///
    /// 支持格式：
    /// - `"5"` → `Normal(5)`
    /// - `"40+30"` → `SlotBoost { base: 40, boost: 30 }`
    /// - `"2*40"` → `LastBoost(40)`
    ///
    /// 解析失败时返回 `None`。
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        // 尝试解析为纯数字
        if let Ok(val) = raw.parse::<u32>() {
            return Some(Self::Normal(val));
        }
        // 尝试 "base+boost" 格式
        if let Some((base_str, boost_str)) = raw.split_once('+') {
            let base = base_str.trim().parse::<u32>().ok()?;
            let boost = boost_str.trim().parse::<u32>().ok()?;
            return Some(Self::SlotBoost { base, boost });
        }
        // 尝试 "2*base" 格式
        if let Some((mul_str, base_str)) = raw.split_once('*') {
            let multiplier = mul_str.trim().parse::<u32>().ok()?;
            let base = base_str.trim().parse::<u32>().ok()?;
            if multiplier == 2 {
                return Some(Self::LastBoost(base));
            }
            // 其他倍数暂不支持
            return None;
        }
        None
    }
}

const BUILTIN_SKILL_FACTORIES: [SkillFactory; 35] = [
    fire::FireSkill::box_new,
    ice::IceSkill::box_new,
    thunder::ThunderSkill::box_new,
    quake::QuakeSkill::box_new,
    absorb::AbsorbSkill::box_new,
    poison::PoisonSkill::box_new,
    rapid::RapidSkill::box_new,
    critical::CriticalSkill::box_new,
    half::HalfSkill::box_new,
    exchange::ExchangeSkill::box_new,
    berserk::BerserkSkill::box_new,
    charm::CharmSkill::box_new,
    haste::HasteSkill::box_new,
    slow::SlowSkill::box_new,
    curse::CurseSkill::box_new,
    heal::HealSkill::box_new,
    revive::ReviveSkill::box_new,
    disperse::DisperseSkill::box_new,
    iron::IronSkill::box_new,
    charge::ChargeSkill::box_new,
    accumulate::AccumulateSkill::box_new,
    assassinate::AssassinateSkill::box_new,
    summon::SummonSkill::box_new,
    clone::CloneSkill::box_new,
    shadow::ShadowSkill::box_new,
    defend::DefendSkill::box_new,
    protect::ProtectSkill::box_new,
    reflect::ReflectSkill::box_new,
    reraise::ReraiseSkill::box_new,
    shield::ShieldSkill::box_new,
    counter::CounterSkill::box_new,
    merge::MergeSkill::box_new,
    zombie::ZombieSkill::box_new,
    upgrade::UpgradeSkill::box_new,
    hide::HideSkill::box_new,
];
/// DIY / overlay 模式下的主动技能 ID 列表（按槽位顺序）。
///
/// 共 25 个主动技能，从 Fire(0) 到 Shadow(24)。
/// 这些 ID 对应 `BUILTIN_SKILL_FACTORIES` 中前 25 个工厂函数。
const DIY_ACTIVE_SKILL_IDS: [usize; 25] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
];

/// DIY / overlay 模式下的被动技能 ID 列表（按槽位顺序）。
///
/// 共 10 个被动技能，从 Defend(25) 到 Hide(34)。
const DIY_PASSIVE_SKILL_IDS: [usize; 10] = [25, 26, 27, 28, 29, 30, 31, 32, 33, 34];

/// 将 overlay 中的技能名映射到 Rust 技能 ID。
///
/// 兼容多种输入格式：`sklfire` / `FireSkill` / `fire` / `sklFire` 均可识别。
/// 匹配逻辑：
/// 1. 转小写后去掉前导的 `skl` 或 `skill` 前缀；
/// 2. 剩余部分与内置技能英文名做精确匹配（大小写不敏感）。
///
/// 返回对应的技能 ID（0~35），未识别时返回 `None`。
pub fn skill_name_to_id(name: &str) -> Option<usize> {
    let lower = name.trim().to_ascii_lowercase();
    let normalized = lower
        .strip_prefix("skl")
        .or_else(|| lower.strip_prefix("skill"))
        .unwrap_or(lower.as_str());
    match normalized {
        "fire" => Some(0),
        "ice" => Some(1),
        "thunder" => Some(2),
        "quake" => Some(3),
        "absorb" => Some(4),
        "poison" => Some(5),
        "rapid" => Some(6),
        "critical" => Some(7),
        "half" => Some(8),
        "exchange" => Some(9),
        "berserk" => Some(10),
        "charm" => Some(11),
        "haste" => Some(12),
        "slow" => Some(13),
        "curse" => Some(14),
        "heal" => Some(15),
        "revive" => Some(16),
        "disperse" => Some(17),
        "iron" => Some(18),
        "charge" => Some(19),
        "accumulate" => Some(20),
        "assassinate" => Some(21),
        "summon" => Some(22),
        "clone" => Some(23),
        "shadow" => Some(24),
        "defend" => Some(25),
        "protect" => Some(26),
        "reflect" => Some(27),
        "reraise" => Some(28),
        "shield" => Some(29),
        "counter" => Some(30),
        "merge" => Some(31),
        "zombie" => Some(32),
        "upgrade" => Some(33),
        "hide" => Some(34),
        "none" => Some(35),
        _ => None,
    }
}

/// DIY / overlay 模式下的技能槽顺序。
///
/// 固定顺序：25 个主动技能 → 10 个被动技能 → 5 个未使用槽位（35~39）。
/// 这对应 JS 侧 `k1` 的固定布局，与普通玩家的随机排序不同。
/// 使用固定顺序是为了让 overlay 技能在 merge（吞噬）时槽位类型一致，
/// 确保 Shield / Defend / PassiveSkill 不会被错位。
pub fn diy_skill_order() -> Vec<usize> { DIY_ACTIVE_SKILL_IDS.into_iter().chain(DIY_PASSIVE_SKILL_IDS).chain(35..40).collect() }

/// 将技能 ID 转换为 overlay 兼容的技能名（如 `sklFire`、`sklHeal`）。
///
/// 用于 `to_diy_compact()` / `to_ol_json()` 导出时生成 key。
/// 返回格式与 [`skill_name_to_id`] 接受的格式兼容。
pub fn skill_name_for_export(skill_id: usize) -> String {
    let name = match skill_id {
        0 => "fire",
        1 => "ice",
        2 => "thunder",
        3 => "quake",
        4 => "absorb",
        5 => "poison",
        6 => "rapid",
        7 => "critical",
        8 => "half",
        9 => "exchange",
        10 => "berserk",
        11 => "charm",
        12 => "haste",
        13 => "slow",
        14 => "curse",
        15 => "heal",
        16 => "revive",
        17 => "disperse",
        18 => "iron",
        19 => "charge",
        20 => "accumulate",
        21 => "assassinate",
        22 => "summon",
        23 => "clone",
        24 => "shadow",
        25 => "defend",
        26 => "protect",
        27 => "reflect",
        28 => "reraise",
        29 => "shield",
        30 => "counter",
        31 => "merge",
        32 => "zombie",
        33 => "upgrade",
        34 => "hide",
        35 => "none",
        _ => return format!("skill{}", skill_id),
    };
    format!("skl{}", name)
}

/// 将 overlay 中指定的技能等级写入 SkillStorage。
///
/// 流程：
/// 1. 遍历 `skill_levels` 映射，将每个技能名解析为技能 ID；
/// 2. 根据 [`SkillBoost`] 类型设置技能等级和加成信息：
///    - `Normal(lv)`: 直接 `set_level(lv)`
///    - `SlotBoost { base, boost }`: `set_level(base + boost)`, 标记 `boosted = true`, 存储 boost 信息
///    - `LastBoost(base)`: `set_level(base * 2)`, 标记 `boosted = true`, 存储 boost 信息
/// 3. 重新设定技能槽顺序为 [`diy_skill_order()`] 固定顺序；
/// 4. 调用 `update_proc()` 刷新流程缓存。
///
/// 注意：此函数替代了正常的 boost 流程，overlay 模式不调用 `boost_last()` / `boost_level()`。
pub fn apply_diy_skill_levels(storage: &mut store::SkillStorage, skill_levels: &HashMap<String, SkillBoost>) {
    for (skill_name, skill_boost) in skill_levels {
        let Some(skill_id) = skill_name_to_id(skill_name) else {
            continue;
        };
        if let Some(skill) = storage.store.get_mut(&skill_id) {
            let final_lv = skill_boost.final_level();
            skill.set_level(final_lv);
            match skill_boost {
                SkillBoost::Normal(_) => {
                    // 普通技能：无 boost 标记，diy_boost 置为 None
                    skill.boosted = false;
                    skill.diy_boost = None;
                }
                SkillBoost::SlotBoost { .. } | SkillBoost::LastBoost(_) => {
                    // 有加成的技能：标记 boosted = true 防止后续误 boost，
                    // 并存储 boost 信息供 clone 重建时使用。
                    skill.boosted = true;
                    skill.diy_boost = Some(skill_boost.clone());
                }
            }
        }
    }
    let order = diy_skill_order();
    storage.slot_skill = order.clone();
    storage.skill = order;
    storage.is_diy = true;
    storage.update_proc();
}

#[derive(Default)]
struct SkillRegistry {
    factories: FastHashMap<u8, SkillFactory>,
}

impl SkillRegistry {
    fn with_builtins() -> Self {
        let mut registry = Self::default();
        for (id, factory) in BUILTIN_SKILL_FACTORIES.iter().copied().enumerate() {
            registry.register_builtin(id as u8, factory);
        }
        registry
    }

    #[inline]
    fn register_builtin(&mut self, id: u8, factory: SkillFactory) { self.factories.insert(id, factory); }

    #[inline]
    fn register(&mut self, id: u8, factory: SkillFactory) -> Option<SkillFactory> { self.factories.insert(id, factory) }

    #[inline]
    fn create(&self, id: u8) -> Option<Box<dyn SkillTrait>> { self.factories.get(&id).map(|factory| factory()) }
}

fn global_skill_registry() -> &'static RwLock<SkillRegistry> {
    static REGISTRY: OnceLock<RwLock<SkillRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| RwLock::new(SkillRegistry::with_builtins()))
}

static SKILL_REGISTRY_DIRTY: AtomicBool = AtomicBool::new(false);

#[inline]
fn create_builtin_skill(id: u8) -> Option<Box<dyn SkillTrait>> {
    BUILTIN_SKILL_FACTORIES.get(id as usize).copied().map(|factory| factory())
}

/// 注册（或覆盖）一个技能工厂。
/// 这为后续 DLL/hook 式扩展提供稳定入口，内置技能仍走零配置默认注册。
pub fn register_skill_factory(id: u8, factory: SkillFactory) -> Option<SkillFactory> {
    SKILL_REGISTRY_DIRTY.store(true, AtomicOrdering::Release);
    let mut registry = global_skill_registry().write().expect("skill registry poisoned");
    registry.register(id, factory)
}

#[inline]
fn create_skill_from_registry(id: u8) -> Box<dyn SkillTrait> {
    if !SKILL_REGISTRY_DIRTY.load(AtomicOrdering::Acquire) {
        return create_builtin_skill(id).unwrap_or_else(none::NoneSkill::box_new);
    }
    let registry = global_skill_registry().read().expect("skill registry poisoned");
    registry
        .create(id)
        .or_else(|| create_builtin_skill(id))
        .unwrap_or_else(none::NoneSkill::box_new)
}

/// SkillArgs:
/// PlrId: player handle（稳定 ID，不是内存指针）
/// &'d mut RC4: random number generator
/// &'d mut RunUpdates: updates to be applied
/// &'d `Arc<Storage>`: game storage
pub type SkillArgs<'d> = (PlrId, &'d mut RC4, &'d mut RunUpdates, &'d Arc<Storage>);

/// 技能注册的流程类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProcKind {
    UpdateState,
    PreStep,
    PreAction,
    PostAction,
    PreDefend,
    PostDefend,
    PostDamage,
    PostDeath,
    PostKill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostActionPhase {
    Early,
    Late,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillTargetDomain {
    EnemyAlive,
    AllyAlive,
    AllyAny,
    AllyDead,
    SelfOnly,
    AllAlive,
}

#[allow(unused_variables, unused_mut)]
pub trait SkillTrait: Debug + Send + Sync {
    // ===== 必须实现的 =====
    /// 销毁这个玩意 (技能用过了)
    fn destroy(&self, plr: PlrId, args: SkillArgs);
    /// 用于实现 Clone
    fn clone_box(&self) -> Box<dyn SkillTrait>;
    /// 运行时类型键。
    ///
    /// 这里不要对 `Box<dyn SkillTrait>` / `&dyn SkillTrait` 本身做 `type_name_of_val()`，
    /// 那样拿到的是 trait object 名字，无法区分具体技能实现。
    /// 必须让它经由 vtable 分发到具体实现，再在 concrete `Self` 上取类型名。
    fn runtime_kind(&self) -> &'static str { type_name_of_val(self) }

    // ===== 可选实现的 =====
    /// 更新状态
    fn update_state(&mut self, args: SkillArgs) {}
    fn update_state_with_level(&mut self, _level: u32, args: SkillArgs) { self.update_state(args) }
    /// 内联版更新状态 — 直接修改 PlayerStatus，不经过 Storage。
    /// 在 Player::update_states() 中调用，对齐 JS 的 F() 遍历 rx 回调。
    fn update_state_inline(&mut self, _level: u32, _status: &mut super::PlayerStatus) {}
    /// 行动!
    fn act(&mut self, targets: Vec<PlrId>, smart: bool, args: SkillArgs) {}
    fn act_with_level(&mut self, _level: u32, targets: Vec<PlrId>, smart: bool, args: SkillArgs) {
        self.act(targets, smart, args)
    }
    /// 技能出手后的等级回写钩子。
    ///
    /// 默认返回原等级，表示这次使用不会改变当前熟练度。
    /// 少数主动技能会覆写这里，让“当前熟练度”在出手后衰减；
    /// 当前仓库中确认会降低熟练度的只有：
    /// - `生命之轮`
    /// - `治愈魔法`
    /// - `苏生术`
    /// - `分身`
    /// - `幻术`
    ///
    /// 注意这里作用的是“本场战斗中的当前技能等级”，不是重新 `build()` 时
    /// 根据名字推导出来的原始熟练度。
    fn post_act_level(&self, level: u32) -> u32 { level }

    fn pre_step(&mut self, mut step: i32, args: SkillArgs) -> i32 { step }
    fn pre_step_with_level(&mut self, _level: u32, step: i32, args: SkillArgs) -> i32 { self.pre_step(step, args) }
    /// 行动之前
    fn pre_action(&mut self, args: SkillArgs) {}
    fn pre_action_with_level(&mut self, _level: u32, args: SkillArgs) { self.pre_action(args) }
    /// preAction 是否强制选择当前技能
    fn pre_action_select(&mut self, _smart: bool, _args: SkillArgs) -> bool { false }
    fn pre_action_select_with_level(&mut self, _level: u32, smart: bool, args: SkillArgs) -> bool {
        self.pre_action_select(smart, args)
    }
    /// preAction 是否清空当前强制动作（对齐 JS preAction 链可返回 null）。
    fn pre_action_clear_forced(&mut self, _smart: bool, _args: SkillArgs) -> bool { false }
    fn pre_action_clear_forced_with_level(&mut self, _level: u32, smart: bool, args: SkillArgs) -> bool {
        self.pre_action_clear_forced(smart, args)
    }
    /// JS preAction 链按“上一项返回值”累积 forced skill。
    /// 默认语义：先执行 pre_action，再按 clear/select 决定是清空、改成当前技能还是保留上一值。
    fn pre_action_accumulate(
        &mut self,
        current_forced: Option<usize>,
        self_key: usize,
        smart: bool,
        args: SkillArgs,
    ) -> Option<usize> {
        let (owner, randomer, updates, storage) = args;
        self.pre_action((owner, &mut *randomer, &mut *updates, storage));
        // JS 里多数普通 pre_action entry 的 `aN(prev, ...)` 只是：
        // 1. 执行自己的 pre_action 副作用
        // 2. 如果还没有前人选中的 forced skill，再决定要不要把自己设成 forced
        // 3. 否则原样保留 `prev`
        //
        // 只有少数 hook（如 Hide / Assassinate）会显式“清空 prev”或“无视 prev 强行替换”。
        // 这些需要各自覆写 `pre_action_accumulate_with_level()`，不能走这里的默认语义。
        if current_forced.is_none() && self.pre_action_select(smart, (owner, &mut *randomer, &mut *updates, storage)) {
            Some(self_key)
        } else {
            current_forced
        }
    }
    fn pre_action_accumulate_with_level(
        &mut self,
        level: u32,
        current_forced: Option<usize>,
        self_key: usize,
        smart: bool,
        args: SkillArgs,
    ) -> Option<usize> {
        let (owner, randomer, updates, storage) = args;
        self.pre_action_with_level(level, (owner, &mut *randomer, &mut *updates, storage));
        if current_forced.is_none()
            && self.pre_action_select_with_level(level, smart, (owner, &mut *randomer, &mut *updates, storage))
        {
            Some(self_key)
        } else {
            current_forced
        }
    }
    fn dynamic_pre_action_enabled(&self) -> bool { false }
    fn manages_dynamic_pre_action(&self) -> bool { false }
    /// 行动之后
    fn post_action(&mut self, args: SkillArgs) {}
    fn post_action_with_level(&mut self, _level: u32, args: SkillArgs) { self.post_action(args) }
    fn post_action_phase(&self) -> PostActionPhase { PostActionPhase::Early }
    /// 每次 action 结束后的回调（对齐 RunUpdates.onUpdateEnd）
    fn on_update_end(&mut self, _args: SkillArgs) -> bool { false }
    fn on_update_end_with_level(&mut self, _level: u32, args: SkillArgs) -> bool { self.on_update_end(args) }
    /// 防御之前
    fn pre_defend(&mut self, mut atp: f64, caster: PlrId, is_mag: bool, on_damage: &OnDamageFunc, args: SkillArgs) -> f64 { atp }
    fn pre_defend_with_level(
        &mut self,
        _level: u32,
        atp: f64,
        caster: PlrId,
        is_mag: bool,
        on_damage: &OnDamageFunc,
        args: SkillArgs,
    ) -> f64 {
        self.pre_defend(atp, caster, is_mag, on_damage, args)
    }
    /// 防御之后
    fn post_defend(&mut self, mut dmg: i32, caster: PlrId, on_damage: &OnDamageFunc, args: SkillArgs) -> i32 { dmg }
    fn post_defend_with_level(&mut self, _level: u32, dmg: i32, caster: PlrId, on_damage: &OnDamageFunc, args: SkillArgs) -> i32 {
        self.post_defend(dmg, caster, on_damage, args)
    }
    /// post_defend 优先级（越小越先执行）。JS 中 skill 和 state 共享同一个 y2 链表，按 ga4() 排序。
    fn post_defend_priority(&self) -> i32 { 1000 }
    /// 伤害之后
    fn post_damage(&mut self, dmg: i32, caster: PlrId, args: SkillArgs) {}
    fn post_damage_with_level(&mut self, _level: u32, dmg: i32, caster: PlrId, args: SkillArgs) {
        self.post_damage(dmg, caster, args)
    }
    /// post_damage 优先级（越大越后执行）。JS 中使用 sortId (ga4())，Infinity 表示最后执行。
    fn post_damage_priority(&self) -> i32 { 10000 }
    /// 死亡时（返回 true 表示短路，不再执行后续 die）
    fn die(&mut self, oldhp: i32, caster: PlrId, args: SkillArgs) -> bool { false }
    fn die_with_level(&mut self, _level: &mut u32, oldhp: i32, caster: PlrId, args: SkillArgs) -> bool {
        self.die(oldhp, caster, args)
    }
    /// 击杀目标后（返回 true 表示短路，不再执行后续 kill）
    fn kill(&mut self, target: PlrId, args: SkillArgs) -> bool { false }
    fn kill_with_level(&mut self, _level: u32, target: PlrId, args: SkillArgs) -> bool { self.kill(target, args) }
    /// 被净化等效果清理正向运行时状态时触发，可返回对应文案模板。
    fn clear_positive_runtime(&mut self, _args: SkillArgs) -> Option<&'static str> { None }
    fn clear_positive_runtime_with_level(&mut self, _level: u32, args: SkillArgs) -> Option<&'static str> {
        self.clear_positive_runtime(args)
    }
    fn clear_positive_runtime_priority(&self) -> i32 { 1000 }

    /// 仅供少数需要复用 JS meta("charge") 语义的技能查询。
    /// 默认关闭，只有 ChargeSkill 会在其运行时生效期间返回 true。
    fn charge_runtime_active(&self) -> bool { false }

    /// 蓄力当前的 step 数值（默认 0，仅 ChargeSkill 实现）
    fn charge_step(&self) -> i32 { 0 }

    /// 潜行锁定的目标 ID（默认 None，仅 AssassinateSkill 实现）
    fn assassinate_target(&self) -> Option<PlrId> { None }

    /// 仅供读取短时 update_state 运行时态使用。
    /// 默认关闭，只有少数带“一段时间内持续生效”的技能会在激活时返回 true。
    fn dynamic_update_state_enabled(&self) -> bool { false }

    /// 声明该技能注册到哪些流程
    fn proc_kinds(&self) -> &[ProcKind] { &[] }

    /// 清除 protect 目标（默认无操作，仅 ProtectSkill 实现）
    fn clear_protect_to(&mut self) {}

    /// 获取 protect 目标 ID（默认返回 None，仅 ProtectSkill 实现）
    fn protect_to_id(&self) -> Option<PlrId> { None }

    /// 技能触发概率（默认对齐 Dart: r127 < level）
    fn prob(&self, level: u32, _smart: bool, args: SkillArgs) -> bool { args.1.r127() < level }

    /// 技能目标来源域。
    fn target_domain(&self) -> SkillTargetDomain { SkillTargetDomain::EnemyAlive }
    fn target_domain_with_level(&self, _level: u32) -> SkillTargetDomain { self.target_domain() }
    fn allows_empty_targets(&self) -> bool { false }
    fn allows_empty_targets_with_level(&self, _level: u32) -> bool { self.allows_empty_targets() }

    /// 技能选目标数量（默认对齐 Dart）
    fn select_target_count(&self, smart: bool) -> usize { if smart { 3 } else { 2 } }
    fn select_target_count_with_level(&self, _level: u32, smart: bool) -> usize { self.select_target_count(smart) }

    /// 技能目标合法性判定
    fn valid_target(&self, _target: PlrId, _smart: bool, _args: SkillArgs) -> bool { true }
    fn valid_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> bool {
        self.valid_target(target, smart, args)
    }

    /// 技能目标打分（默认对齐 Dart 基础行为）
    fn score_target(&self, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        let Some(target_plr) = args.3.get_player(&target) else {
            return f64::MIN;
        };
        if smart {
            let rate_hi_hp = |hp: i32| -> f64 {
                if hp < 20 {
                    30.0
                } else if hp > 300 {
                    300.0
                } else {
                    hp as f64
                }
            };
            let rate_low_hp = |hp: i32| -> f64 { 1.0 / rate_hi_hp(hp) };
            let alive_group_count = args.3.alive_group_count();
            let target_alive_group_len = args.3.alive_group_at_team_of(target).map(|group| group.len()).unwrap_or(0);
            let status = target_plr.get_status();
            if alive_group_count > 2 {
                rate_hi_hp(status.hp) * target_alive_group_len as f64 * status.attract
            } else {
                rate_low_hp(status.hp) * status.atk_sum as f64 * status.attract
            }
        } else {
            args.1.rFFFF() as f64 + target_plr.get_status().attract
        }
    }
    fn score_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        self.score_target(target, smart, args)
    }

    /// 技能选目标流程（默认：按 valid 过滤，随机采样后按 score 排序）
    fn select_targets(&self, candidates: &[PlrId], smart: bool, args: SkillArgs) -> Vec<PlrId> {
        let select_count = self.select_target_count(smart);
        if select_count == 0 {
            return Vec::new();
        }
        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let Some(idx) = args.1.pick(candidates) else {
                return Vec::new();
            };
            let target = candidates[idx];
            if !self.valid_target(target, smart, (args.0, args.1, args.2, args.3)) {
                invalid += 1;
                continue;
            }
            if selected.contains(&target) {
                dup += 1;
                continue;
            }
            selected.push(target);
            if selected.len() >= select_count {
                break;
            }
        }
        if selected.is_empty() {
            return Vec::new();
        }
        if selected.len() == 1 {
            let target = selected[0];
            let _ = self.score_target(target, smart, (args.0, args.1, args.2, args.3));
            return vec![target];
        }

        let mut scored = selected
            .into_iter()
            .map(|target| (target, self.score_target(target, smart, (args.0, args.1, args.2, args.3))))
            .collect::<Vec<(PlrId, f64)>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(Ordering::Equal));
        scored.into_iter().map(|x| x.0).collect()
    }

    fn select_targets_with_level(&self, level: u32, candidates: &[PlrId], smart: bool, args: SkillArgs) -> Vec<PlrId> {
        let select_count = self.select_target_count_with_level(level, smart);
        if select_count == 0 {
            return Vec::new();
        }
        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let Some(idx) = args.1.pick(candidates) else {
                return Vec::new();
            };
            let target = candidates[idx];
            if !self.valid_target_with_level(level, target, smart, (args.0, args.1, args.2, args.3)) {
                invalid += 1;
                continue;
            }
            if selected.contains(&target) {
                dup += 1;
                continue;
            }
            selected.push(target);
            if selected.len() >= select_count {
                break;
            }
        }
        if selected.is_empty() {
            return Vec::new();
        }
        if selected.len() == 1 {
            let target = selected[0];
            let _ = self.score_target_with_level(level, target, smart, (args.0, args.1, args.2, args.3));
            return vec![target];
        }

        let mut scored = selected
            .into_iter()
            .map(|target| {
                (
                    target,
                    self.score_target_with_level(level, target, smart, (args.0, args.1, args.2, args.3)),
                )
            })
            .collect::<Vec<(PlrId, f64)>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(Ordering::Equal));
        scored.into_iter().map(|x| x.0).collect()
    }

    /// JS 的 `Skill.aa(...)` 允许具体技能完全覆盖默认选目标流程。
    /// Rust 目前的主动作路径为了稳定旧回放，默认仍保留手工按 domain 抽样；
    /// 只有显式声明的技能才走这里的自定义选目标实现，避免一次性放大回归面。
    fn uses_custom_target_selection(&self) -> bool { false }
    /// 少数技能在 JS 里虽然有自己的 `aa()`，但“正常出手”时其实仍复用 ActionSkill 的
    /// `attack_aa` 抽样路径（`all_alive + pickSkipRange`），只是在 valid/score 上覆写。
    ///
    /// Rust 侧如果直接走 `select_targets_with_level(candidates, ...)`，会把 `EnemyAlive`
    /// 压缩成紧凑列表，导致 RC4 消费变轻。这里提供一个窄 opt-in，让入口继续掌握
    /// `ActionTargets`，只把“候选抽样方式”切到 JS 的 `attack_aa`。
    fn uses_attack_aa_sampling(&self) -> bool { false }

    /// 标记该技能的主动施放逻辑是否已接入当前运行链路。
    fn has_action_impl(&self) -> bool { false }

    fn is_normal_skill(&self) -> bool { true }

    fn is_boss_skill(&self) -> bool { false }

    fn is_weapon_skill(&self) -> bool { false }
}

impl Clone for Box<dyn SkillTrait> {
    fn clone(&self) -> Box<dyn SkillTrait> { self.clone_box() }
}

pub trait SkillExt: SkillTrait {
    fn box_new() -> Box<dyn SkillTrait>;
}

#[derive(Debug, Clone)]
pub struct Skill {
    /// 是否被增强过
    pub boosted: bool,
    /// 等级
    level: u32,
    /// 类型
    skill_type: Box<dyn SkillTrait>,
    /// 目标
    pub target: Option<PlrId>,
    /// DIY 技能加成信息（`None` 表示非 DIY 技能）。
    ///
    /// 存储原始的 [`SkillBoost`] 配置，用于在 clone 重建时
    /// 正确计算衰减下限（decay floor）。
    pub diy_boost: Option<SkillBoost>,
}

impl Skill {
    pub fn new(level: u32, skill_type: Box<dyn SkillTrait>) -> Self {
        Self {
            boosted: false,
            level,
            skill_type,
            target: None,
            diy_boost: None,
        }
    }

    pub fn new_with_id(level: u32, id: u8) -> Self {
        let skill_type = create_skill_from_registry(id);
        Self {
            boosted: false,
            level,
            skill_type,
            target: None,
            diy_boost: None,
        }
    }

    pub fn set_target(&mut self, target: PlrId) { self.target = Some(target); }

    pub fn get_target(&self) -> Option<PlrId> { self.target }

    /// 如果没 boost, 那就 boost 一下
    /// true: boost 成功
    /// false: 已经 boost 过了
    pub fn boost_if_not(&mut self) -> bool {
        if self.boosted {
            false
        } else {
            self.boosted = true;
            self.level *= 2;
            true
        }
    }

    pub fn boost_level(&mut self, level: u32) -> bool {
        if self.boosted {
            self.level += level;
            false
        } else {
            self.level += level;
            self.boosted = true;
            true
        }
    }

    /// 获取技能等级
    pub fn level(&self) -> u32 { self.level }

    pub fn set_level(&mut self, level: u32) { self.level = level; }

    /// 临时取出技能实现，留下 NoneSkill 占位。
    /// 配合 `put_skill_type` 使用，用于在释放 &mut Player 后安全地调用回调。
    pub fn take_skill_type(&mut self) -> Box<dyn SkillTrait> {
        std::mem::replace(&mut self.skill_type, Box::new(skl::none::NoneSkill))
    }

    /// 将之前取出的技能实现放回。
    pub fn put_skill_type(&mut self, skill_type: Box<dyn SkillTrait>) { self.skill_type = skill_type; }

    // ==========
    // 以下是技能 call pre/post 之类的东西
    // ==========

    pub fn update_state(&mut self, args: SkillArgs) { self.skill_type.update_state_with_level(self.level, args) }

    pub fn update_state_inline(&mut self, status: &mut super::PlayerStatus) {
        self.skill_type.update_state_inline(self.level, status)
    }

    /// 执行主动技能，并在动作结束后按 `post_act_level()` 回写当前熟练度。
    ///
    /// 这一步是 clone 必须做 clamp 的根源：
    /// 某些技能在战斗中用过后，会把“当前熟练度”降到低于名字 `build()` 出来的初始值；
    /// 如果 clone 重新 `build()` 后不再以上限裁到 owner 当前等级，这些技能就会被错误地
    /// “刷新回满”，从而改变概率判定、行动顺序和整场回放。
    pub fn act(&mut self, targets: Vec<PlrId>, smart: bool, args: SkillArgs) {
        let current_level = self.level;
        self.skill_type.act_with_level(current_level, targets, smart, args);
        let post_level = self.skill_type.post_act_level(current_level);
        if self.level == current_level || post_level > self.level {
            self.level = post_level;
        }
    }

    pub fn pre_step(&mut self, step: i32, args: SkillArgs) -> i32 { self.skill_type.pre_step_with_level(self.level, step, args) }

    pub fn pre_action(&mut self, args: SkillArgs) { self.skill_type.pre_action_with_level(self.level, args) }

    pub fn pre_action_select(&mut self, smart: bool, args: SkillArgs) -> bool {
        self.skill_type.pre_action_select_with_level(self.level, smart, args)
    }

    pub fn pre_action_clear_forced(&mut self, smart: bool, args: SkillArgs) -> bool {
        self.skill_type.pre_action_clear_forced_with_level(self.level, smart, args)
    }

    pub fn pre_action_accumulate(
        &mut self,
        current_forced: Option<usize>,
        self_key: usize,
        smart: bool,
        args: SkillArgs,
    ) -> Option<usize> {
        self.skill_type
            .pre_action_accumulate_with_level(self.level, current_forced, self_key, smart, args)
    }

    pub fn dynamic_pre_action_enabled(&self) -> bool { self.level > 0 && self.skill_type.dynamic_pre_action_enabled() }

    pub fn manages_dynamic_pre_action(&self) -> bool { self.skill_type.manages_dynamic_pre_action() }

    pub fn post_action(&mut self, args: SkillArgs) { self.skill_type.post_action_with_level(self.level, args) }

    pub fn post_action_phase(&self) -> PostActionPhase { self.skill_type.post_action_phase() }

    pub fn on_update_end(&mut self, args: SkillArgs) -> bool { self.skill_type.on_update_end_with_level(self.level, args) }

    pub fn pre_defend(&mut self, atp: f64, is_mag: bool, caster: PlrId, on_damage: &OnDamageFunc, args: SkillArgs) -> f64 {
        self.skill_type.pre_defend_with_level(self.level, atp, caster, is_mag, on_damage, args)
    }

    pub fn post_defend(&mut self, dmg: i32, caster: PlrId, on_damage: &OnDamageFunc, args: SkillArgs) -> i32 {
        self.skill_type.post_defend_with_level(self.level, dmg, caster, on_damage, args)
    }

    pub fn post_defend_priority(&self) -> i32 { self.skill_type.post_defend_priority() }

    pub fn post_damage(&mut self, dmg: i32, caster: PlrId, args: SkillArgs) {
        self.skill_type.post_damage_with_level(self.level, dmg, caster, args)
    }

    pub fn post_damage_priority(&self) -> i32 { self.skill_type.post_damage_priority() }

    pub fn die(&mut self, oldhp: i32, caster: PlrId, args: SkillArgs) -> bool {
        self.skill_type.die_with_level(&mut self.level, oldhp, caster, args)
    }

    pub fn kill(&mut self, target: PlrId, args: SkillArgs) -> bool { self.skill_type.kill_with_level(self.level, target, args) }

    pub fn clear_positive_runtime(&mut self, args: SkillArgs) -> Option<&'static str> {
        self.skill_type.clear_positive_runtime_with_level(self.level, args)
    }

    pub fn proc_kinds(&self) -> &[ProcKind] { self.skill_type.proc_kinds() }

    pub fn clear_protect_to(&mut self) { self.skill_type.clear_protect_to() }

    pub fn protect_to_id(&self) -> Option<PlrId> { self.skill_type.protect_to_id() }

    pub fn prob(&self, smart: bool, args: SkillArgs) -> bool { self.skill_type.prob(self.level, smart, args) }

    pub fn target_domain(&self) -> SkillTargetDomain { self.skill_type.target_domain_with_level(self.level) }

    pub fn allows_empty_targets(&self) -> bool { self.skill_type.allows_empty_targets_with_level(self.level) }

    pub fn select_target_count(&self, smart: bool) -> usize { self.skill_type.select_target_count_with_level(self.level, smart) }

    pub fn valid_target(&self, target: PlrId, smart: bool, args: SkillArgs) -> bool {
        self.skill_type.valid_target_with_level(self.level, target, smart, args)
    }

    pub fn score_target(&self, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        self.skill_type.score_target_with_level(self.level, target, smart, args)
    }

    pub fn select_targets(&self, candidates: &[PlrId], smart: bool, args: SkillArgs) -> Vec<PlrId> {
        self.skill_type.select_targets_with_level(self.level, candidates, smart, args)
    }

    pub fn uses_custom_target_selection(&self) -> bool { self.skill_type.uses_custom_target_selection() }

    pub fn uses_attack_aa_sampling(&self) -> bool { self.skill_type.uses_attack_aa_sampling() }

    pub fn has_action_impl(&self) -> bool { self.skill_type.has_action_impl() }

    pub fn clear_positive_runtime_priority(&self) -> i32 { self.skill_type.clear_positive_runtime_priority() }

    pub fn charge_runtime_active(&self) -> bool { self.skill_type.charge_runtime_active() }

    pub fn charge_step(&self) -> i32 { self.skill_type.charge_step() }

    pub fn assassinate_target(&self) -> Option<PlrId> { self.skill_type.assassinate_target() }

    pub fn dynamic_update_state_enabled(&self) -> bool { self.level > 0 && self.skill_type.dynamic_update_state_enabled() }

    /// 调试/对齐 JS 时用的运行时技能类型名。
    ///
    /// `md5.js` 里的 merge 不是按“技能 key / 技能 id 是否相等”比较，而是按
    /// `k1[slot]` 里的“技能对象类型”逐槽位比较；Rust 这边没有直接暴露 Dart/JS 的
    /// `runtimeType`，因此需要一个稳定的“当前 SkillTrait 实现类型”视图来对照。
    pub fn debug_skill_type_name(&self) -> &'static str { self.skill_type.runtime_kind() }

    /// 是否与另一技能拥有相同的运行时实现类型。
    ///
    /// 这对应 `md5.js` 里 merge 的：
    /// `q = J.uR(m); if (q.gcw(m) !== q.gcw(l)) break`
    ///
    /// 也就是说，merge 比较的是“同槽位 skill 的类型是否一致”，而不是 key / id
    /// 是否相等。这个区别对 summon 尤其关键：JS summon 的固定槽位是
    /// `[fire, fire, explode]`，两个 fire 在 Rust 里可能是不同 key，但 merge 仍应视为
    /// “同类型、可继承”。
    pub fn same_runtime_kind(&self, other: &Self) -> bool { self.debug_skill_type_name() == other.debug_skill_type_name() }
}

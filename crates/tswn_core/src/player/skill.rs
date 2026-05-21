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
use std::fmt::Debug;
use std::sync::{
    Arc, OnceLock, RwLock,
    atomic::{AtomicBool, Ordering as AtomicOrdering},
};

use smallvec::SmallVec;

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

/// DIY / overlay 技能加成信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillBoost {
    Normal(u32),
    SlotBoost { base: u32, boost: u32 },
    LastBoost(u32),
}

impl SkillBoost {
    pub fn final_level(&self) -> u32 {
        match self {
            Self::Normal(lv) => *lv,
            Self::SlotBoost { base, boost } => base + boost,
            Self::LastBoost(base) => base * 2,
        }
    }

    pub fn base_level(&self) -> u32 {
        match self {
            Self::Normal(lv) => *lv,
            Self::SlotBoost { base, .. } => *base,
            Self::LastBoost(base) => *base,
        }
    }

    pub fn decayed_base_from_level(&self, current_level: u32) -> u32 {
        match self {
            Self::Normal(_) => current_level,
            Self::SlotBoost { boost, .. } => current_level.saturating_sub(*boost).max(1),
            Self::LastBoost(_) => current_level / 2,
        }
    }

    pub fn final_level_from_decayed_base(&self, decayed_base: u32) -> u32 {
        match self {
            Self::Normal(_) => decayed_base,
            Self::SlotBoost { boost, .. } => decayed_base.saturating_add(*boost),
            Self::LastBoost(_) => decayed_base.saturating_mul(2),
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if let Ok(val) = raw.parse::<u32>() {
            return Some(Self::Normal(val));
        }
        if let Some((base_str, boost_str)) = raw.split_once('+') {
            let base = base_str.trim().parse::<u32>().ok()?;
            let boost = boost_str.trim().parse::<u32>().ok()?;
            return Some(Self::SlotBoost { base, boost });
        }
        if let Some((mul_str, base_str)) = raw.split_once('*') {
            let multiplier = mul_str.trim().parse::<u32>().ok()?;
            let base = base_str.trim().parse::<u32>().ok()?;
            return (multiplier == 2).then_some(Self::LastBoost(base));
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

const DIY_ACTIVE_SKILL_IDS: [usize; 25] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
];

const DIY_PASSIVE_SKILL_IDS: [usize; 10] = [25, 26, 27, 28, 29, 30, 31, 32, 33, 34];

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
        _ => normalized.parse::<usize>().ok().filter(|id| *id < 40),
    }
}

pub fn diy_skill_order() -> Vec<usize> { DIY_ACTIVE_SKILL_IDS.into_iter().chain(DIY_PASSIVE_SKILL_IDS).chain(35..40).collect() }

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

pub fn apply_diy_skill_levels(storage: &mut store::SkillStorage, skill_levels: &[(String, SkillBoost)]) {
    let mut ordered_ids: Vec<usize> = Vec::with_capacity(40);
    let mut seen = [false; 40];
    for (skill_name, skill_boost) in skill_levels {
        let Some(skill_id) = skill_name_to_id(skill_name) else {
            continue;
        };
        if seen[skill_id] {
            continue;
        }
        seen[skill_id] = true;
        ordered_ids.push(skill_id);
        if let Some(skill) = storage.store.get_mut(&skill_id) {
            let base_lv = skill_boost.base_level();
            skill.set_level(base_lv);
            skill.boosted = false;
            match skill_boost {
                SkillBoost::Normal(_) => {
                    skill.diy_boost = None;
                }
                SkillBoost::SlotBoost { .. } | SkillBoost::LastBoost(_) => {
                    skill.diy_boost = Some(skill_boost.clone());
                }
            }
        }
    }
    for id in diy_skill_order() {
        if !seen[id] {
            seen[id] = true;
            ordered_ids.push(id);
        }
    }
    storage.slot_skill = (0..40usize).collect();
    storage.skill = ordered_ids;
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

/// 跨实体副作用（方案 J）。
///
/// 在 owner-centric 回调中缓冲，回调返回后由调度器统一 flush。
pub enum Effect {
    /// 攻击目标
    Attack {
        target: PlrId,
        atp: f64,
        is_mag: bool,
        on_damage: OnDamageFunc,
    },
    /// 回血目标
    Heal { target: PlrId, amount: i32 },
    /// 直接伤害（不走攻防计算）
    DamageRaw {
        target: PlrId,
        dmg: i32,
        on_damage: OnDamageFunc,
    },
    /// 位移点增减
    AddMovePoint { target: PlrId, delta: i32 },
    /// owner 自身死亡结算（如附体自灭），需要在 skill 借用释放后执行
    OwnerDie { old_hp: i32 },
}

#[derive(Debug, Clone, Copy)]
pub struct PostDamageCtx {
    pub dmg: i32,
    pub caster: PlrId,
}

/// owner-centric 技能回调的上下文（方案 J）。
///
/// 持有 `&mut Player` 完整引用，技能通过 `ctx.owner` 直接访问 owner 的全部能力，
/// 无需经过 `Storage::just_get_player_mut`。
///
/// **安全约定**：构造时 `ctx.owner` 与 dispatcher 持有的 `&mut Skill` 存在内部别名，
/// 因此回调中 **不要调用 `ctx.owner.update_states()`**（会重入 skills 迭代）。
/// 需要更新状态时请用 `ctx.mark_update_states()`，调度器会在回调返回后统一调用。
pub struct InlineCtx<'a> {
    pub ptr: PlrId,
    pub owner: &'a mut super::Player,
    pub randomer: &'a mut RC4,
    pub updates: &'a mut RunUpdates,
    pub storage: &'a Arc<Storage>,
    pub post_damage: Option<PostDamageCtx>,
    pub effects: SmallVec<[Effect; 4]>,
    pub(super) needs_update_states: bool,
}

impl<'a> InlineCtx<'a> {
    /// 设置状态（通过 `set_state_no_update`，调度器在回调返回后统一调用 `update_states()`）。
    pub fn set_state(&mut self, state: impl super::StateTrait + 'static) {
        self.owner.set_state_no_update(state);
        self.needs_update_states = true;
    }

    /// 标记需要在回调返回后调用 `update_states()`。
    pub fn mark_update_states(&mut self) { self.needs_update_states = true; }

    pub fn post_damage_meta(&self) -> PostDamageCtx {
        self.post_damage
            .expect("post_damage metadata is only available during post_damage_inline")
    }
}

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
    /// 是否实现了内联行动 — 调度器据此选择 act_inline 或 act_with_level。
    fn has_inline_act(&self) -> bool { false }
    /// 内联版行动 — 通过 InlineCtx 直接访问 owner 字段（方案 J）。
    fn act_inline(&mut self, _level: u32, _targets: Vec<PlrId>, _smart: bool, _ctx: &mut InlineCtx) {}
    /// 内联版 clear_positive_runtime — 通过 InlineCtx 直接访问（方案 J）。
    fn clear_positive_runtime_inline(&mut self, _ctx: &mut InlineCtx) -> Option<&'static str> { None }
    /// 是否实现了内联版 pre_action。
    fn has_inline_pre_action(&self) -> bool { false }
    /// 内联版 pre_action（方案 J）。
    fn pre_action_inline(&mut self, _ctx: &mut InlineCtx) {}
    /// 是否实现了内联版 post_action。
    fn has_inline_post_action(&self) -> bool { false }
    /// 内联版 post_action（方案 J）。
    fn post_action_inline(&mut self, _ctx: &mut InlineCtx) {}
    /// 是否实现了内联版 post_damage。
    fn has_inline_post_damage(&self) -> bool { false }
    /// 内联版 post_damage（方案 J）。
    fn post_damage_inline(&mut self, _level: u32, _ctx: &mut InlineCtx) {}
    /// 是否实现了 owner-inline post_kill。
    fn has_inline_post_kill(&self) -> bool { false }
    /// owner-inline post_kill（方案 J）。
    fn kill_inline(&mut self, level: u32, target: PlrId, ctx: &mut InlineCtx) -> bool {
        self.kill_with_level(level, target, (ctx.ptr, ctx.randomer, ctx.updates, ctx.storage))
    }
    /// 是否支持在 target 已作为 `&mut Player` 存活时执行 post_kill。
    fn has_dead_target_post_kill_inline(&self) -> bool { false }
    /// target-inline post_kill，用于 `on_die_impl` 中避免通过 Storage 重借当前死亡目标。
    fn kill_dead_target_inline(
        &mut self,
        level: u32,
        target: PlrId,
        target_player: &mut super::Player,
        ctx: &mut InlineCtx,
    ) -> bool {
        self.kill_inline(level, target, ctx)
    }
    /// 是否实现了内联版 pre_defend。
    fn has_inline_pre_defend(&self) -> bool { false }
    /// 内联版 pre_defend（方案 J）。
    fn pre_defend_inline(
        &mut self,
        _level: u32,
        _ctx: &mut InlineCtx,
        atp: f64,
        _is_mag: bool,
        _caster: PlrId,
        _on_damage: &OnDamageFunc,
    ) -> f64 {
        atp
    }
    /// 行动!
    fn act(&mut self, targets: Vec<PlrId>, smart: bool, args: SkillArgs) {}
    fn act_with_level(&mut self, _level: u32, targets: Vec<PlrId>, smart: bool, args: SkillArgs) {
        self.act(targets, smart, args)
    }
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
            let target_alive_group_len = args
                .3
                .alive_group_at_team_of(target)
                .map(|group| group.len())
                .unwrap_or(0);
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

#[derive(Debug)]
pub struct Skill {
    /// 是否被增强过
    pub boosted: bool,
    /// 等级
    level: u32,
    /// `post_damage` 优先级在运行时是静态元数据；即使 skill_type 被临时 detach 成 NoneSkill，
    /// 也必须保留原值，避免战中 register_skill_proc 用占位符优先级把顺序排错。
    post_damage_priority: i32,
    /// 类型；`take_skill_type` 取出后会短暂为 `None`，调用方须通过 `put_skill_type` 放回。
    skill_type: Option<Box<dyn SkillTrait>>,
    /// 目标
    pub target: Option<PlrId>,
    /// DIY 技能加成信息（`None` 表示非 DIY 技能）。
    pub diy_boost: Option<SkillBoost>,
}

impl Clone for Skill {
    fn clone(&self) -> Self {
        Self {
            boosted: self.boosted,
            level: self.level,
            post_damage_priority: self.post_damage_priority,
            skill_type: self.skill_type.as_ref().map(|st| st.clone_box()),
            target: self.target,
            diy_boost: self.diy_boost.clone(),
        }
    }
}

impl Skill {
    pub fn new(level: u32, skill_type: Box<dyn SkillTrait>) -> Self {
        let post_damage_priority = skill_type.post_damage_priority();
        Self {
            boosted: false,
            level,
            post_damage_priority,
            skill_type: Some(skill_type),
            target: None,
            diy_boost: None,
        }
    }

    pub fn new_with_id(level: u32, id: u8) -> Self {
        let skill_type = create_skill_from_registry(id);
        let post_damage_priority = skill_type.post_damage_priority();
        Self {
            boosted: false,
            level,
            post_damage_priority,
            skill_type: Some(skill_type),
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
            let base = self.level;
            self.boosted = true;
            self.level *= 2;
            self.diy_boost = Some(SkillBoost::LastBoost(base));
            true
        }
    }

    pub fn boost_level(&mut self, level: u32) -> bool {
        if self.boosted {
            self.level += level;
            false
        } else {
            let base = self.level;
            self.level += level;
            self.boosted = true;
            self.diy_boost = Some(SkillBoost::SlotBoost { base, boost: level });
            true
        }
    }

    /// 获取技能等级
    pub fn level(&self) -> u32 { self.level }

    pub fn set_level(&mut self, level: u32) { self.level = level; }

    /// 临时取出技能实现。
    /// 配合 `put_skill_type` 使用，用于在释放 &mut Player 后安全地调用回调。
    /// 取出后 `skill_type` 暂时为 `None`，调用方必须在完成操作后通过 `put_skill_type` 放回原值。
    pub fn take_skill_type(&mut self) -> Box<dyn SkillTrait> {
        self.skill_type.take().expect("skill_type already taken - put_skill_type not called?")
    }

    /// 将之前取出的技能实现放回。
    pub fn put_skill_type(&mut self, skill_type: Box<dyn SkillTrait>) {
        debug_assert!(
            self.skill_type.is_none(),
            "skill_type already present - take_skill_type not called?"
        );
        self.skill_type = Some(skill_type);
    }

    // ==========
    // 以下是技能 call pre/post 之类的东西
    // ==========

    // ==========
    // 以下访问器在 skill_type 为 None 时返回安全默认值。
    // skill_type 在 take_skill_type 后短暂为 None，此时重入访问应等效于 NoneSkill。
    // ==========

    pub fn update_state(&mut self, args: SkillArgs) {
        if let Some(st) = &mut self.skill_type {
            st.update_state_with_level(self.level, args);
        }
    }

    pub fn update_state_inline(&mut self, status: &mut super::PlayerStatus) {
        if let Some(st) = &mut self.skill_type {
            st.update_state_inline(self.level, status);
        }
    }

    pub fn has_inline_act(&self) -> bool { self.skill_type.as_ref().is_some_and(|st| st.has_inline_act()) }

    pub fn act_inline(&mut self, targets: Vec<PlrId>, smart: bool, ctx: &mut InlineCtx) {
        let Some(st) = &mut self.skill_type else {
            return;
        };
        let current_level = self.level;
        st.act_inline(current_level, targets, smart, ctx);
        let post_level = st.post_act_level(current_level);
        if self.level == current_level || post_level > self.level {
            self.level = post_level;
        }
    }

    pub fn act(&mut self, targets: Vec<PlrId>, smart: bool, args: SkillArgs) {
        let Some(st) = &mut self.skill_type else {
            return;
        };
        let current_level = self.level;
        st.act_with_level(current_level, targets, smart, args);
        let post_level = st.post_act_level(current_level);
        if self.level == current_level || post_level > self.level {
            self.level = post_level;
        }
    }

    pub fn pre_step(&mut self, step: i32, args: SkillArgs) -> i32 {
        match &mut self.skill_type {
            Some(st) => st.pre_step_with_level(self.level, step, args),
            None => step,
        }
    }

    pub fn pre_action(&mut self, args: SkillArgs) {
        if let Some(st) = &mut self.skill_type {
            st.pre_action_with_level(self.level, args);
        }
    }

    pub fn pre_action_select(&mut self, smart: bool, args: SkillArgs) -> bool {
        self.skill_type
            .as_mut()
            .is_some_and(|st| st.pre_action_select_with_level(self.level, smart, args))
    }

    pub fn pre_action_clear_forced(&mut self, smart: bool, args: SkillArgs) -> bool {
        self.skill_type
            .as_mut()
            .is_some_and(|st| st.pre_action_clear_forced_with_level(self.level, smart, args))
    }

    pub fn pre_action_accumulate(
        &mut self,
        current_forced: Option<usize>,
        self_key: usize,
        smart: bool,
        args: SkillArgs,
    ) -> Option<usize> {
        match &mut self.skill_type {
            Some(st) => st.pre_action_accumulate_with_level(self.level, current_forced, self_key, smart, args),
            None => current_forced,
        }
    }

    pub fn dynamic_pre_action_enabled(&self) -> bool {
        self.level > 0 && self.skill_type.as_ref().is_some_and(|st| st.dynamic_pre_action_enabled())
    }

    pub fn manages_dynamic_pre_action(&self) -> bool {
        self.skill_type.as_ref().is_some_and(|st| st.manages_dynamic_pre_action())
    }

    pub fn post_action(&mut self, args: SkillArgs) {
        if let Some(st) = &mut self.skill_type {
            st.post_action_with_level(self.level, args);
        }
    }

    pub fn post_action_phase(&self) -> PostActionPhase {
        self.skill_type.as_ref().map_or(PostActionPhase::Early, |st| st.post_action_phase())
    }

    pub fn on_update_end(&mut self, args: SkillArgs) -> bool {
        self.skill_type.as_mut().is_some_and(|st| st.on_update_end_with_level(self.level, args))
    }

    pub fn pre_defend(&mut self, atp: f64, is_mag: bool, caster: PlrId, on_damage: &OnDamageFunc, args: SkillArgs) -> f64 {
        match &mut self.skill_type {
            Some(st) => st.pre_defend_with_level(self.level, atp, caster, is_mag, on_damage, args),
            None => atp,
        }
    }

    pub fn post_defend(&mut self, dmg: i32, caster: PlrId, on_damage: &OnDamageFunc, args: SkillArgs) -> i32 {
        match &mut self.skill_type {
            Some(st) => st.post_defend_with_level(self.level, dmg, caster, on_damage, args),
            None => dmg,
        }
    }

    pub fn post_defend_priority(&self) -> i32 { self.skill_type.as_ref().map_or(0, |st| st.post_defend_priority()) }

    pub fn post_damage(&mut self, dmg: i32, caster: PlrId, args: SkillArgs) {
        if let Some(st) = &mut self.skill_type {
            st.post_damage_with_level(self.level, dmg, caster, args);
        }
    }

    pub fn post_damage_priority(&self) -> i32 { self.post_damage_priority }

    pub fn die(&mut self, oldhp: i32, caster: PlrId, args: SkillArgs) -> bool {
        self.skill_type
            .as_mut()
            .is_some_and(|st| st.die_with_level(&mut self.level, oldhp, caster, args))
    }

    pub fn kill(&mut self, target: PlrId, args: SkillArgs) -> bool {
        self.skill_type.as_mut().is_some_and(|st| st.kill_with_level(self.level, target, args))
    }

    pub fn clear_positive_runtime(&mut self, args: SkillArgs) -> Option<&'static str> {
        match &mut self.skill_type {
            Some(st) => st.clear_positive_runtime_with_level(self.level, args),
            None => None,
        }
    }

    pub fn clear_positive_runtime_inline(&mut self, ctx: &mut InlineCtx) -> Option<&'static str> {
        match &mut self.skill_type {
            Some(st) => st.clear_positive_runtime_inline(ctx),
            None => None,
        }
    }

    pub fn has_inline_pre_action(&self) -> bool { self.skill_type.as_ref().is_some_and(|st| st.has_inline_pre_action()) }

    pub fn pre_action_inline(&mut self, ctx: &mut InlineCtx) {
        if let Some(st) = &mut self.skill_type {
            st.pre_action_inline(ctx);
        }
    }

    pub fn has_inline_post_action(&self) -> bool { self.skill_type.as_ref().is_some_and(|st| st.has_inline_post_action()) }

    pub fn post_action_inline(&mut self, ctx: &mut InlineCtx) {
        if let Some(st) = &mut self.skill_type {
            st.post_action_inline(ctx);
        }
    }

    pub fn has_inline_post_damage(&self) -> bool { self.skill_type.as_ref().is_some_and(|st| st.has_inline_post_damage()) }

    pub fn post_damage_inline(&mut self, ctx: &mut InlineCtx) {
        if let Some(st) = &mut self.skill_type {
            st.post_damage_inline(self.level, ctx);
        }
    }

    pub fn has_inline_pre_defend(&self) -> bool { self.skill_type.as_ref().is_some_and(|st| st.has_inline_pre_defend()) }

    pub fn pre_defend_inline(
        &mut self,
        ctx: &mut InlineCtx,
        atp: f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: &OnDamageFunc,
    ) -> f64 {
        match &mut self.skill_type {
            Some(st) => st.pre_defend_inline(self.level, ctx, atp, is_mag, caster, on_damage),
            None => atp,
        }
    }

    pub fn proc_kinds(&self) -> &[ProcKind] {
        match &self.skill_type {
            Some(st) => st.proc_kinds(),
            None => &[],
        }
    }

    pub fn clear_protect_to(&mut self) {
        if let Some(st) = &mut self.skill_type {
            st.clear_protect_to();
        }
    }

    pub fn protect_to_id(&self) -> Option<PlrId> { self.skill_type.as_ref().and_then(|st| st.protect_to_id()) }

    pub fn prob(&self, smart: bool, args: SkillArgs) -> bool {
        self.skill_type.as_ref().is_some_and(|st| st.prob(self.level, smart, args))
    }

    pub fn target_domain(&self) -> SkillTargetDomain {
        self.skill_type
            .as_ref()
            .map_or(SkillTargetDomain::EnemyAlive, |st| st.target_domain_with_level(self.level))
    }

    pub fn allows_empty_targets(&self) -> bool {
        self.skill_type.as_ref().is_some_and(|st| st.allows_empty_targets_with_level(self.level))
    }

    pub fn select_target_count(&self, smart: bool) -> usize {
        self.skill_type
            .as_ref()
            .map_or(1, |st| st.select_target_count_with_level(self.level, smart))
    }

    pub fn valid_target(&self, target: PlrId, smart: bool, args: SkillArgs) -> bool {
        self.skill_type
            .as_ref()
            .is_some_and(|st| st.valid_target_with_level(self.level, target, smart, args))
    }

    pub fn score_target(&self, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        self.skill_type
            .as_ref()
            .map_or(f64::MIN, |st| st.score_target_with_level(self.level, target, smart, args))
    }

    pub fn select_targets(&self, candidates: &[PlrId], smart: bool, args: SkillArgs) -> Vec<PlrId> {
        match &self.skill_type {
            Some(st) => st.select_targets_with_level(self.level, candidates, smart, args),
            None => Vec::new(),
        }
    }

    pub fn uses_custom_target_selection(&self) -> bool {
        self.skill_type.as_ref().is_some_and(|st| st.uses_custom_target_selection())
    }

    pub fn uses_attack_aa_sampling(&self) -> bool { self.skill_type.as_ref().is_some_and(|st| st.uses_attack_aa_sampling()) }

    pub fn has_action_impl(&self) -> bool { self.skill_type.as_ref().is_some_and(|st| st.has_action_impl()) }

    pub fn clear_positive_runtime_priority(&self) -> i32 {
        self.skill_type.as_ref().map_or(i32::MAX, |st| st.clear_positive_runtime_priority())
    }

    pub fn charge_runtime_active(&self) -> bool { self.skill_type.as_ref().is_some_and(|st| st.charge_runtime_active()) }

    pub fn charge_step(&self) -> i32 { self.skill_type.as_ref().map_or(0, |st| st.charge_step()) }

    pub fn assassinate_target(&self) -> Option<PlrId> { self.skill_type.as_ref().and_then(|st| st.assassinate_target()) }

    pub fn has_inline_post_kill(&self) -> bool { self.skill_type.as_ref().is_some_and(|st| st.has_inline_post_kill()) }

    pub fn dynamic_update_state_enabled(&self) -> bool {
        self.level > 0 && self.skill_type.as_ref().is_some_and(|st| st.dynamic_update_state_enabled())
    }

    /// 调试/对齐 JS 时用的运行时技能类型名。
    ///
    /// `md5.js` 里的 merge 不是按“技能 key / 技能 id 是否相等”比较，而是按
    /// `k1[slot]` 里的“技能对象类型”逐槽位比较；Rust 这边没有直接暴露 Dart/JS 的
    /// `runtimeType`，因此需要一个稳定的“当前 SkillTrait 实现类型”视图来对照。
    pub fn debug_skill_type_name(&self) -> &'static str { self.skill_type.as_ref().map_or("<None>", |st| st.runtime_kind()) }

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

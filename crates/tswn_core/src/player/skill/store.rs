//! # 技能存储 (store)
//!
//! 本模块定义 [`SkillStorage`]，管理玩家当前装备的技能列表及各阶段触发器。
//!
//! ## 功能说明
//!
//! - **技能存储** — 使用 HashMap 存储所有技能
//! - **技能索引** — 维护技能列表以便快速访问
//! - **触发器管理** — 按流程阶段管理技能触发器
//! - **技能注册** — 提供便捷的技能注册接口
//!
//! ## 触发器阶段
//!
//! | 阶段          | 说明                              |
//! |---------------|-----------------------------------|
//! | `update_states` | 每回合刷新属性快照时            |
//! | `pre_step`     | 行动前（移动点数计算）            |
//! | `pre_action`   | 行动前（目标选择前）             |
//! | `post_action`  | 行动后                          |
//! | `pre_defend`   | 被攻击前（可修改 atp 或截断伤害） |
//! | `post_defend`  | 被攻击后（可修改实际伤害值）       |
//! | `post_damage`  | 造成伤害后                        |
//! | `post_death`   | 死亡时                          |
//! | `post_kill`    | 击杀时                          |
//!
//! ## 技能键
//!
//! 使用 `SkillKey` (usize) 作为稳定技能键，用于在存储中唯一标识技能。
//!
//! ## Pre-Action 结果
//!
//! [`PreActionOutcome`] 存储 pre-action 阶段的执行结果：
//! - `forced_skill` — 强制使用的技能键
//! - `clear_forced_action` — 是否清除强制行动
//!
//! ## 示例
//!
//! ```rust,ignore
//! use tswn_core::player::skill::store::SkillStorage;
//!
//! let mut storage = SkillStorage::new();
//! // 注册技能到各个触发器阶段
//! ```

use crate::player::{
    OnDamageFunc, PlrId,
    skill::{InlineCtx, PostActionPhase, ProcKind, Skill, SkillArgs},
};
use foldhash::{HashMap as FoldHashMap, HashMapExt, HashSet as FoldHashSet, HashSetExt};

/// SkillStorage 内部使用的稳定技能键。
pub type SkillKey = usize;

#[derive(Debug, Clone, Copy, Default)]
pub struct PreActionOutcome {
    pub forced_skill: Option<SkillKey>,
    pub clear_forced_action: bool,
}

#[derive(Debug, Clone)]
pub struct SkillStorage {
    pub store: FoldHashMap<SkillKey, Skill>,
    /// JS `Player.k1` 的固定技能槽位视图。
    ///
    /// 这里表达的是“名字 build 之后，这个实体 16 个技能槽位各自装的是什么技能类型”。
    /// 它不应该随着行动顺序洗牌而变化；像 summon 这种 JS 里会打乱 `k2/k4` 的实体，
    /// 也仍然保留固定的 `k1=[fire, fire, explode]`，供 merge 按槽位继承。
    pub slot_skill: Vec<SkillKey>,
    /// JS `k4` / 主动技能遍历顺序视图。
    ///
    /// 大多数普通玩家这里与 `slot_skill` 相同；但 summon / 战斗中启用的新技能 /
    /// 某些动态插入场景会只改这里，不改 `slot_skill`。
    ///
    /// 因此：
    /// - `slot_skill` 负责“按固定槽位比较”的逻辑（典型是 merge）
    /// - `skill` 负责 `action()` 里的主动技能扫描顺序
    pub skill: Vec<SkillKey>,
    pub disabled_action: FoldHashSet<SkillKey>,
    /// meta??
    pub meta: FoldHashSet<SkillKey>,
    // 自己的状态 (usize: index)
    /// 更新状态时?
    pub update_states: Vec<SkillKey>,
    /// step 之前
    pub pre_step: Vec<SkillKey>,
    /// 动作之前
    pub pre_action: Vec<SkillKey>,
    pre_action_membership: FoldHashSet<SkillKey>,
    /// 动作之后
    pub post_action: Vec<SkillKey>,
    /// 战斗中途新增的 early post_action。
    /// tuple.0 是注册当时的 state 插入游标：它应当在所有 order < cursor 的 state 之后、
    /// 所有 order >= cursor 的 state 之前执行，贴近 JS 统一 x2 队列的插入语义。
    pub post_action_after_states: Vec<(u64, SkillKey)>,
    /// 防御之前
    pub pre_defend: Vec<SkillKey>,
    /// 防御之后
    pub post_defend: Vec<SkillKey>,
    /// 伤害之后
    pub post_damage: Vec<SkillKey>,
    /// 死亡之后
    pub post_death: Vec<SkillKey>,
    /// 干掉目标之后
    pub post_kill: Vec<SkillKey>,
    // 别的什么东西
    pub pending_clear_states: bool,
}

impl SkillStorage {
    pub fn new() -> Self {
        Self {
            store: FoldHashMap::new(),
            slot_skill: Vec::new(),
            skill: Vec::new(),
            disabled_action: FoldHashSet::new(),
            update_states: Vec::new(),
            meta: FoldHashSet::new(),
            pre_step: Vec::new(),
            pre_action: Vec::new(),
            pre_action_membership: FoldHashSet::new(),
            post_action: Vec::new(),
            post_action_after_states: Vec::new(),
            pre_defend: Vec::new(),
            post_defend: Vec::new(),
            post_damage: Vec::new(),
            post_death: Vec::new(),
            post_kill: Vec::new(),
            pending_clear_states: false,
        }
    }

    fn clear_proc(&mut self) {
        self.update_states.clear();
        self.meta.clear();
        self.pre_step.clear();
        self.pre_action.clear();
        self.pre_action_membership.clear();
        self.post_action.clear();
        self.post_action_after_states.clear();
        self.pre_defend.clear();
        self.post_defend.clear();
        self.post_damage.clear();
        self.post_death.clear();
        self.post_kill.clear();
    }

    fn push_proc_key(&mut self, kind: ProcKind, key: SkillKey) {
        match kind {
            ProcKind::UpdateState => {
                if !self.update_states.contains(&key) {
                    self.update_states.push(key);
                }
            }
            ProcKind::PreStep => {
                if !self.pre_step.contains(&key) {
                    self.pre_step.push(key);
                }
            }
            ProcKind::PreAction => {
                if self.pre_action_membership.insert(key) {
                    self.pre_action.push(key);
                }
            }
            ProcKind::PostAction => {
                if !self.post_action.contains(&key) {
                    self.post_action.push(key);
                }
            }
            ProcKind::PreDefend => {
                if !self.pre_defend.contains(&key) {
                    self.pre_defend.push(key);
                }
            }
            ProcKind::PostDefend => {
                if !self.post_defend.contains(&key) {
                    self.post_defend.push(key);
                }
            }
            ProcKind::PostDamage => {
                if self.post_damage.contains(&key) {
                    return;
                }
                let priority = self.store.get(&key).map(|s| s.post_damage_priority()).unwrap_or(0);
                let insert_at = self.post_damage.iter().position(|existing| {
                    self.store.get(existing).map(|skill| skill.post_damage_priority()).unwrap_or(0) > priority
                });
                if let Some(idx) = insert_at {
                    self.post_damage.insert(idx, key);
                } else {
                    self.post_damage.push(key);
                }
            }
            ProcKind::PostDeath => {
                if !self.post_death.contains(&key) {
                    self.post_death.push(key);
                }
            }
            ProcKind::PostKill => {
                if !self.post_kill.contains(&key) {
                    self.post_kill.push(key);
                }
            }
        }
    }

    pub fn register_skill_proc(&mut self, key: SkillKey) {
        let Some(skill) = self.store.get(&key) else {
            return;
        };
        if skill.level() == 0 {
            return;
        }
        let kinds: Vec<ProcKind> = skill.proc_kinds().to_vec();
        for kind in kinds {
            self.push_proc_key(kind, key);
        }
    }

    pub fn register_skill_proc_after_states(&mut self, key: SkillKey, state_order_cursor: u64) {
        let Some(skill) = self.store.get(&key) else {
            return;
        };
        if skill.level() == 0 {
            return;
        }
        let post_action_phase = skill.post_action_phase();
        let kinds: Vec<ProcKind> = skill.proc_kinds().to_vec();
        for kind in kinds {
            if kind == ProcKind::PostAction && post_action_phase == PostActionPhase::Early {
                if !self.post_action_after_states.iter().any(|(_, queued_key)| *queued_key == key) {
                    self.post_action_after_states.push((state_order_cursor, key));
                    self.post_action_after_states.sort_by_key(|(cursor, _)| *cursor);
                }
                continue;
            }
            self.push_proc_key(kind, key);
        }
    }

    pub fn update_proc(&mut self) {
        self.clear_proc();
        let mut keys: Vec<SkillKey> = self.store.keys().copied().collect();
        keys.sort_unstable();
        for key in keys {
            self.register_skill_proc(key);
        }
    }

    /// 最后一个技能 boost
    pub fn boost_last(&mut self) {
        for skill in self.skill.iter().rev() {
            let should_try_boost = {
                let skill_ref = self.store.get(skill).expect("skill not found in store");
                skill_ref.level() > 0 && skill_ref.has_action_impl()
            };
            if !should_try_boost {
                continue;
            }
            if self.store.get_mut(skill).expect("skill not found in store").boost_if_not() {
                break;
            }
        }
    }

    pub fn disable_action_key(&mut self, key: SkillKey) { self.disabled_action.insert(key); }

    pub fn enable_action_key(&mut self, key: SkillKey) { self.disabled_action.remove(&key); }

    pub fn action_enabled(&self, key: SkillKey) -> bool { !self.disabled_action.contains(&key) }

    pub fn add_skill(&mut self, skill: Skill) {
        let id = self.skill.len();
        self.store.insert(id, skill);
        // 默认情况下，手工 add 的技能同时进入固定槽位视图和行动顺序视图。
        // 如果某个实体需要“固定槽位”和“行动顺序”分离（如 summon），调用方会在构造
        // 完成后显式覆写 `slot_skill` / `skill`。
        //
        // 换句话说，`add_skill()` 提供的是“普通玩家 / 测试手工拼装技能”的默认语义，
        // 不是 JS 所有实体都适用的最终排布。
        self.slot_skill.push(id);
        self.skill.push(id);
    }

    fn set_pre_action_membership(&mut self, key: SkillKey, enabled: bool) {
        match enabled {
            true => {
                if self.pre_action_membership.insert(key) {
                    self.pre_action.push(key);
                }
            }
            false => {
                if self.pre_action_membership.remove(&key)
                    && let Some(pos) = self.pre_action.iter().position(|existing| *existing == key)
                {
                    self.pre_action.remove(pos);
                }
            }
        }
    }

    pub fn sync_dynamic_pre_action_state(&mut self, key: SkillKey, manages: bool, enabled: bool) {
        if manages {
            self.set_pre_action_membership(key, enabled);
        }
    }

    pub fn sync_dynamic_pre_action_key(&mut self, key: SkillKey) {
        let Some((manages, enabled)) = self
            .store
            .get(&key)
            .map(|skill| (skill.manages_dynamic_pre_action(), skill.dynamic_pre_action_enabled()))
        else {
            return;
        };
        self.sync_dynamic_pre_action_state(key, manages, enabled);
    }

    pub fn skill_by_idx(&self, idx: usize) -> &Skill { self.store.get(&self.skill[idx]).expect("skill not found in store") }

    pub fn skill_by_idx_mut(&mut self, idx: usize) -> &mut Skill {
        self.store.get_mut(&self.skill[idx]).expect("skill not found in store")
    }

    pub fn skill_by_id(&self, id: SkillKey) -> &Skill { self.store.get(&id).expect("skill not found in store") }

    pub fn skill_by_id_mut(&mut self, id: SkillKey) -> &mut Skill { self.store.get_mut(&id).expect("skill not found in store") }

    // ==========
    // 以下是从 plr 里拆过来的部分, pre/post 之类的东西
    // ==========

    pub fn update_state(&mut self, args: SkillArgs) {
        for idx in 0..self.update_states.len() {
            let skill_key = self.update_states[idx];
            let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
            skill.update_state((args.0, args.1, args.2, args.3));
        }
    }

    pub fn update_state_inline(&mut self, status: &mut crate::player::PlayerStatus) {
        for idx in 0..self.update_states.len() {
            let skill_key = self.update_states[idx];
            let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
            skill.update_state_inline(status);
        }
    }

    pub fn pre_step(&mut self, mut step: i32, args: SkillArgs) -> i32 {
        for idx in 0..self.pre_step.len() {
            let skill_key = self.pre_step[idx];
            let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
            step = skill.pre_step(step, (args.0, args.1, args.2, args.3));
        }
        step
    }

    /// 内联版 pre_action dispatch（方案 J）。
    /// 在常规 pre_action 链之前调用，处理技能的 pre_action 副作用。
    /// 对 HideSkill 等技能，此调用会将 on_update_state 置空，
    /// 使后续 accumulate 链中的旧路径 pre_action 变为空操作。
    pub fn pre_action_inline_dispatch(&mut self, ctx: &mut InlineCtx) {
        for idx in 0..self.pre_action.len() {
            let skill_key = self.pre_action[idx];
            let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
            if skill.has_inline_pre_action() {
                skill.pre_action_inline(ctx);
            }
        }
    }

    pub fn pre_action(&mut self, smart: bool, args: SkillArgs) -> PreActionOutcome {
        let mut forced_skill = None;
        let mut clear_forced_action = false;
        let debug_forced = crate::debug::debug_forced_skill();
        let debug_this = debug_forced
            && args
                .3
                .get_player(&args.0)
                .map(|player| crate::debug::debug_action_matches(&player.id_name()))
                .unwrap_or(false);
        let mut idx = 0usize;
        while idx < self.pre_action.len() {
            let skill_key = self.pre_action[idx];
            let rc4_before = if debug_this { Some((args.1.i, args.1.j)) } else { None };
            let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
            let clear_forced = skill.pre_action_clear_forced(smart, (args.0, args.1, args.2, args.3));
            let prev_forced_skill = forced_skill;
            forced_skill = skill.pre_action_accumulate(forced_skill, skill_key, smart, (args.0, args.1, args.2, args.3));
            let selected = forced_skill == Some(skill_key) && prev_forced_skill != Some(skill_key);
            let manages_dynamic_pre_action = skill.manages_dynamic_pre_action();
            let dynamic_pre_action_enabled = skill.dynamic_pre_action_enabled();
            if clear_forced {
                clear_forced_action = true;
            }
            if forced_skill.is_some() {
                clear_forced_action = false;
            }
            if debug_this {
                let rc4_after = (args.1.i, args.1.j);
                let skill_name = self
                    .store
                    .get(&skill_key)
                    .map(|skill| skill.debug_skill_type_name())
                    .unwrap_or("unknown_skill");
                let (before_i, before_j) = rc4_before.unwrap_or((0, 0));
                eprintln!(
                    "[pre_action_skill] owner={} key={} type={} smart={} clear_forced={} selected={} forced_skill={:?} clear_forced_action={} rc4 {}:{} -> {}:{}",
                    args.3
                        .get_player(&args.0)
                        .map(|player| player.id_name())
                        .unwrap_or_else(|| format!("#{}", args.0)),
                    skill_key,
                    skill_name,
                    smart,
                    clear_forced,
                    selected,
                    forced_skill,
                    clear_forced_action,
                    before_i,
                    before_j,
                    rc4_after.0,
                    rc4_after.1,
                );
            }
            if manages_dynamic_pre_action && !dynamic_pre_action_enabled {
                self.pre_action.remove(idx);
                self.pre_action_membership.remove(&skill_key);
                continue;
            }
            idx += 1;
        }
        if debug_this {
            eprintln!(
                "[pre_action_result] owner={} forced_skill={:?} clear_forced_action={}",
                args.3
                    .get_player(&args.0)
                    .map(|player| player.id_name())
                    .unwrap_or_else(|| format!("#{}", args.0)),
                forced_skill,
                clear_forced_action,
            );
        }
        PreActionOutcome {
            forced_skill,
            clear_forced_action,
        }
    }

    fn post_action_with_phase(&mut self, phase: PostActionPhase, args: SkillArgs) {
        let debug_action = crate::debug::debug_action();
        let debug_this = debug_action
            .as_deref()
            .map(|name| args.3.get_player(&args.0).map(|p| p.id_name() == name).unwrap_or(false))
            .unwrap_or(false);
        for idx in 0..self.post_action.len() {
            let skill_key = self.post_action[idx];
            if self.store.get(&skill_key).map(|skill| skill.post_action_phase()) != Some(phase) {
                continue;
            }
            let rc4_before = (args.1.i, args.1.j);
            let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
            skill.post_action((args.0, args.1, args.2, args.3));
            if debug_this {
                eprintln!(
                    "[post_action_skill/{:?}] key={} rc4 {}:{} -> {}:{}",
                    phase, skill_key, rc4_before.0, rc4_before.1, args.1.i, args.1.j
                );
            }
        }
    }

    pub fn post_action_early(&mut self, args: SkillArgs) { self.post_action_with_phase(PostActionPhase::Early, args) }

    pub fn post_action_late(&mut self, args: SkillArgs) { self.post_action_with_phase(PostActionPhase::Late, args) }

    pub fn run_post_action_key(&mut self, skill_key: SkillKey, args: SkillArgs) {
        let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
        skill.post_action((args.0, args.1, args.2, args.3));
    }

    pub fn run_post_action_key_inline(&mut self, skill_key: SkillKey, ctx: &mut InlineCtx) {
        let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
        if skill.has_inline_post_action() {
            skill.post_action_inline(ctx);
        } else {
            skill.post_action((ctx.ptr, ctx.randomer, ctx.updates, ctx.storage));
        }
    }

    fn post_action_with_phase_inline(&mut self, phase: PostActionPhase, ctx: &mut InlineCtx) {
        for idx in 0..self.post_action.len() {
            let skill_key = self.post_action[idx];
            if self.store.get(&skill_key).map(|skill| skill.post_action_phase()) != Some(phase) {
                continue;
            }
            let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
            if skill.has_inline_post_action() {
                skill.post_action_inline(ctx);
            } else {
                skill.post_action((ctx.ptr, ctx.randomer, ctx.updates, ctx.storage));
            }
        }
    }

    pub fn post_action_early_inline(&mut self, ctx: &mut InlineCtx) {
        self.post_action_with_phase_inline(PostActionPhase::Early, ctx)
    }

    pub fn post_action_late_inline(&mut self, ctx: &mut InlineCtx) {
        self.post_action_with_phase_inline(PostActionPhase::Late, ctx)
    }

    pub fn post_action_after_states(&mut self, args: SkillArgs) {
        for idx in 0..self.post_action_after_states.len() {
            let skill_key = self.post_action_after_states[idx].1;
            self.run_post_action_key(skill_key, (args.0, args.1, args.2, args.3));
        }
    }

    pub fn post_action(&mut self, args: SkillArgs) {
        let (owner, randomer, updates, storage) = args;
        self.post_action_early((owner, randomer, updates, storage));
        self.post_action_after_states((owner, randomer, updates, storage));
        self.post_action_late((owner, randomer, updates, storage));
    }

    pub fn on_update_end(&mut self, args: SkillArgs) -> bool {
        let mut triggered = false;
        for idx in 0..self.skill.len() {
            let skill_key = self.skill[idx];
            let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
            triggered |= skill.on_update_end((args.0, args.1, args.2, args.3));
        }
        triggered
    }

    pub fn pre_defend(&mut self, atp: f64, is_mag: bool, caster: PlrId, on_damage: OnDamageFunc, args: SkillArgs) -> f64 {
        self.pre_defend_range(0, self.pre_defend.len(), atp, is_mag, caster, on_damage, args)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn pre_defend_range(
        &mut self,
        start: usize,
        end: usize,
        mut atp: f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        args: SkillArgs,
    ) -> f64 {
        let debug_action = crate::debug::debug_action();
        let debug_this = debug_action
            .as_deref()
            .map(|name| args.3.get_player(&args.0).map(|p| p.id_name() == name).unwrap_or(false))
            .unwrap_or(false);
        let end = end.min(self.pre_defend.len());
        let start = start.min(end);
        for idx in start..end {
            let skill_key = self.pre_defend[idx];
            let rc4_before = (args.1.i, args.1.j);
            let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
            atp = skill.pre_defend(atp, is_mag, caster, &on_damage, (args.0, args.1, args.2, args.3));
            if debug_this {
                eprintln!(
                    "[pre_defend_skill] owner={} key={} atp={} rc4 {}:{} -> {}:{}",
                    args.3.get_player(&args.0).map(|p| p.id_name()).unwrap_or_else(|| format!("#{}", args.0)),
                    skill_key,
                    atp,
                    rc4_before.0,
                    rc4_before.1,
                    args.1.i,
                    args.1.j,
                );
            }
            if atp == 0.0 {
                return 0.0;
            }
        }
        atp
    }

    /// 内联版 pre_defend_range（方案 J），通过 InlineCtx 直接访问 owner 字段。
    /// skills that have `has_inline_pre_defend() == true` use the inline path;
    /// others fall back to the old SkillArgs-based `pre_defend`.
    #[allow(clippy::too_many_arguments)]
    pub fn pre_defend_range_inline(
        &mut self,
        start: usize,
        end: usize,
        mut atp: f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        ctx: &mut InlineCtx,
    ) -> f64 {
        let end = end.min(self.pre_defend.len());
        let start = start.min(end);
        for idx in start..end {
            let skill_key = self.pre_defend[idx];
            let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
            if skill.has_inline_pre_defend() {
                atp = skill.pre_defend_inline(ctx, atp, is_mag, caster, &on_damage);
            } else {
                atp = skill.pre_defend(atp, is_mag, caster, &on_damage, (ctx.ptr, ctx.randomer, ctx.updates, ctx.storage));
            }
            if atp == 0.0 {
                return 0.0;
            }
        }
        atp
    }

    pub fn post_defend(&mut self, mut dmg: i32, caster: PlrId, on_damage: &OnDamageFunc, args: SkillArgs) -> i32 {
        for idx in 0..self.post_defend.len() {
            let skill_key = self.post_defend[idx];
            let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
            dmg = skill.post_defend(dmg, caster, on_damage, (args.0, args.1, args.2, args.3));
        }
        dmg
    }

    /// 返回 post_defend 技能的 (key, priority) 列表，用于和 state 统一排序。
    pub fn post_defend_keys_with_priority(&self) -> Vec<(SkillKey, i32)> {
        self.post_defend
            .iter()
            .map(|key| {
                let priority = self.store.get(key).map(|s| s.post_defend_priority()).unwrap_or(1000);
                (*key, priority)
            })
            .collect()
    }

    pub fn post_defend_run_one(
        &mut self,
        key: SkillKey,
        dmg: i32,
        caster: PlrId,
        on_damage: &OnDamageFunc,
        args: SkillArgs,
    ) -> i32 {
        let skill = self.store.get_mut(&key).expect("skill not found in store");
        skill.post_defend(dmg, caster, on_damage, args)
    }

    pub fn post_damage(&mut self, dmg: i32, caster: PlrId, args: SkillArgs) {
        let debug_action = crate::debug::debug_action();
        let debug_this = debug_action
            .as_deref()
            .map(|name| args.3.get_player(&args.0).map(|p| p.id_name() == name).unwrap_or(false))
            .unwrap_or(false);
        for idx in 0..self.post_damage.len() {
            let skill_key = self.post_damage[idx];
            let rc4_before = (args.1.i, args.1.j);
            let (manages_dynamic_pre_action, dynamic_pre_action_enabled) = {
                let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
                skill.post_damage(dmg, caster, (args.0, args.1, args.2, args.3));
                (skill.manages_dynamic_pre_action(), skill.dynamic_pre_action_enabled())
            };
            if debug_this {
                eprintln!(
                    "[post_damage_skill] key={} rc4 {}:{} -> {}:{}",
                    skill_key, rc4_before.0, rc4_before.1, args.1.i, args.1.j,
                );
            }
            self.sync_dynamic_pre_action_state(skill_key, manages_dynamic_pre_action, dynamic_pre_action_enabled);
        }
    }

    pub fn clear_positive_runtime(&mut self, args: SkillArgs) -> Vec<&'static str> {
        self.clear_positive_runtime_with_order(args)
            .into_iter()
            .map(|(_, message)| message)
            .collect()
    }

    pub fn clear_positive_runtime_with_order(&mut self, args: SkillArgs) -> Vec<(i32, &'static str)> {
        let mut messages = Vec::new();
        for idx in 0..self.skill.len() {
            let skill_key = self.skill[idx];
            let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
            if let Some(message) = skill.clear_positive_runtime((args.0, args.1, args.2, args.3)) {
                messages.push((skill.clear_positive_runtime_priority(), message));
            }
        }
        messages.sort_unstable_by_key(|(priority, _)| *priority);
        messages
    }

    /// 内联版 clear_positive_runtime（方案 J）。
    ///
    /// 通过 `InlineCtx` 将 owner 字段直接传给技能，避免技能通过 `Storage::just_get_player_mut` 重借 owner。
    pub fn clear_positive_runtime_with_order_inline(
        &mut self,
        ctx: &mut crate::player::skill::InlineCtx,
    ) -> Vec<(i32, &'static str)> {
        let mut messages = Vec::new();
        for idx in 0..self.skill.len() {
            let skill_key = self.skill[idx];
            let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
            let message = skill.clear_positive_runtime_inline(ctx);
            if message.is_none() {
                // fallback: try old SkillArgs-based method
                if let Some(msg) = skill.clear_positive_runtime((ctx.ptr, ctx.randomer, ctx.updates, ctx.storage)) {
                    messages.push((skill.clear_positive_runtime_priority(), msg));
                }
            } else if let Some(msg) = message {
                messages.push((skill.clear_positive_runtime_priority(), msg));
            }
        }
        messages.sort_unstable_by_key(|(priority, _)| *priority);
        messages
    }

    pub fn die(&mut self, oldhp: i32, caster: PlrId, args: SkillArgs) {
        let debug_action = crate::debug::debug_die();
        let debug_this = args
            .3
            .get_player(&args.0)
            .map(|p| crate::debug::debug_action_matches(&p.id_name()))
            .unwrap_or(false)
            || debug_action
                .as_deref()
                .map(|name| args.3.get_player(&args.0).map(|p| p.id_name() == name).unwrap_or(false))
                .unwrap_or(false);
        for idx in 0..self.post_death.len() {
            let skill_key = self.post_death[idx];
            let rc4_before = (args.1.i, args.1.j);
            let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
            let triggered = skill.die(oldhp, caster, (args.0, args.1, args.2, args.3));
            if debug_this {
                eprintln!(
                    "[post_death_skill] key={} triggered={} rc4 {}:{} -> {}:{}",
                    skill_key, triggered, rc4_before.0, rc4_before.1, args.1.i, args.1.j
                );
            }
            if triggered {
                break;
            }
        }
    }

    pub fn kill(&mut self, target: PlrId, args: SkillArgs) {
        let keys = self.post_kill.clone();
        run_post_kill(keys, args.0, target, args.1, args.2, args.3);
    }
}

/// 执行 post_kill 回调。
///
/// 为避免 `&mut Player` 别名 UB，每次回调前先从 storage 中获取并临时取出
/// 技能实现（`skill_type`），释放 player 引用后再调用回调。回调结束后将技能
/// 实现放回。这确保 kill 回调（如吞噬）中通过 `just_get_player_mut` 获取的
/// `&mut Player` 不会与此处的引用重叠。
pub fn run_post_kill(
    keys: Vec<usize>,
    caster: PlrId,
    target: PlrId,
    randomer: &mut crate::rc4::RC4,
    updates: &mut crate::engine::update::RunUpdates,
    storage: &std::sync::Arc<crate::engine::storage::Storage>,
) {
    let debug_action = crate::debug::debug_action();
    let debug_this = debug_action
        .as_deref()
        .map(|name| storage.get_player(&caster).map(|p| p.id_name() == name).unwrap_or(false))
        .unwrap_or(false);
    for skill_key in keys {
        let rc4_before = (randomer.i, randomer.j);
        // 取出技能实现和等级，释放 killer 的 &mut Player
        let (mut skill_type, level) = {
            let killer = storage.just_get_player_mut(caster).expect("killer not found in storage");
            let skill = killer.skills.store.get_mut(&skill_key).expect("skill not found in store");
            (skill.take_skill_type(), skill.level())
            // killer 引用在此处结束
        };
        // 此时无 &mut Player 引用存活，回调中 just_get_player_mut 安全
        let triggered = skill_type.kill_with_level(level, target, (caster, randomer, updates, storage));
        // 将技能实现放回
        {
            let killer = storage.just_get_player_mut(caster).expect("killer not found in storage");
            let skill = killer.skills.store.get_mut(&skill_key).expect("skill not found in store");
            skill.put_skill_type(skill_type);
        }
        if debug_this {
            eprintln!(
                "[post_kill_skill] key={} triggered={} rc4 {}:{} -> {}:{}",
                skill_key, triggered, rc4_before.0, rc4_before.1, randomer.i, randomer.j
            );
        }
        if triggered {
            break;
        }
    }
}

impl Default for SkillStorage {
    fn default() -> Self { Self::new() }
}

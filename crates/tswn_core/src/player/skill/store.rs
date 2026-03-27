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
    skill::{ProcKind, Skill, SkillArgs},
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
    /// 动作之后
    pub post_action: Vec<SkillKey>,
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
            skill: Vec::new(),
            disabled_action: FoldHashSet::new(),
            update_states: Vec::new(),
            meta: FoldHashSet::new(),
            pre_step: Vec::new(),
            pre_action: Vec::new(),
            post_action: Vec::new(),
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
        self.post_action.clear();
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
                if !self.pre_action.contains(&key) {
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
        self.skill.push(id);
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

    pub fn pre_action(&mut self, smart: bool, args: SkillArgs) -> PreActionOutcome {
        let mut forced_skill = None;
        let mut clear_forced_action = false;
        for idx in 0..self.pre_action.len() {
            let skill_key = self.pre_action[idx];
            let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
            if skill.pre_action_clear_forced(smart, (args.0, args.1, args.2, args.3)) {
                // Only set clear_forced_action to block state-based forced attacks (berserk/charm).
                // Do NOT clear forced_skill — it may have been set by Assassinate's pre_action_select,
                // and in JS the assassinate entry is always processed after berserk/hide in x1.
                clear_forced_action = true;
            }
            skill.pre_action((args.0, args.1, args.2, args.3));
            if forced_skill.is_none() && skill.pre_action_select(smart, (args.0, args.1, args.2, args.3)) {
                forced_skill = Some(skill_key);
                clear_forced_action = false;
            }
        }
        PreActionOutcome {
            forced_skill,
            clear_forced_action,
        }
    }

    pub fn post_action(&mut self, args: SkillArgs) {
        let debug_action = crate::debug::debug_action();
        let debug_this = debug_action
            .as_deref()
            .map(|name| args.3.get_player(&args.0).map(|p| p.id_name() == name).unwrap_or(false))
            .unwrap_or(false);
        for idx in 0..self.post_action.len() {
            let skill_key = self.post_action[idx];
            let rc4_before = (args.1.i, args.1.j);
            let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
            skill.post_action((args.0, args.1, args.2, args.3));
            if debug_this {
                eprintln!(
                    "[post_action_skill] key={} rc4 {}:{} -> {}:{}",
                    skill_key, rc4_before.0, rc4_before.1, args.1.i, args.1.j
                );
            }
        }
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

    pub fn pre_defend(&mut self, mut atp: f64, is_mag: bool, caster: PlrId, on_damage: OnDamageFunc, args: SkillArgs) -> f64 {
        let debug_action = crate::debug::debug_action();
        let debug_this = debug_action
            .as_deref()
            .map(|name| args.3.get_player(&args.0).map(|p| p.id_name() == name).unwrap_or(false))
            .unwrap_or(false);
        for idx in 0..self.pre_defend.len() {
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

    pub fn post_defend(&mut self, mut dmg: i32, caster: PlrId, on_damage: &OnDamageFunc, args: SkillArgs) -> i32 {
        for idx in 0..self.post_defend.len() {
            let skill_key = self.post_defend[idx];
            let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
            dmg = skill.post_defend(dmg, caster, on_damage, (args.0, args.1, args.2, args.3));
        }
        dmg
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
            let skill = self.store.get_mut(&skill_key).expect("skill not found in store");
            skill.post_damage(dmg, caster, (args.0, args.1, args.2, args.3));
            if debug_this {
                eprintln!(
                    "[post_damage_skill] key={} rc4 {}:{} -> {}:{}",
                    skill_key, rc4_before.0, rc4_before.1, args.1.i, args.1.j,
                );
            }
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

    pub fn die(&mut self, oldhp: i32, caster: PlrId, args: SkillArgs) {
        let debug_action = crate::debug::debug_die();
        let debug_this = debug_action
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

//! # 玩家状态系统 (state)
//!
//! 本模块定义可扩展的玩家状态模型，用于展示命中/冰冻/狂暴/魅惑等持续性战斗效果。
//!
//! ## 设计思路
//!
//! 每种持续性技能效果（如 `IceState`、`PoisonState`）实现 [`StateTrait`] 并通过
//! `Player::set_state` 挂载到玩家上。
//! 引擎在相应时机遍历所有已挂载的状态并调用对应的动词方法。
//!
//! ## 状态类型标识
//!
//! - `meta_type() == -1` — 供 `Player::build` 阶段的扩展属性模块（如 `FireState`）
//! - `meta_type() == 0`  — 战斗运行期状态（如 `BerserkState`、`IceState`）
//!
//! ## 重要动词一览
//!
//! | 动词                   | 说明                                              |
//! |--------------------------|--------------------------------------------------------|
//! | `on_pre_action`          | 行动前，返回 `true` 则跳过本次行动                  |
//! | `on_forced_action`       | 强制行动（魅惑状态下攻击己方）               |
//! | `on_pre_defend`          | 被攻击前，可修改 atp 或截断伤害                    |
//! | `on_post_defend`         | 被攻击后，可修改实际伤害值                        |
//! | `on_post_action`         | 我方行动结束后，用于计时型持续效果的 tick 计数 |
//! | `on_post_damage`         | 我方造成伤害后（属于我方的状态响应）             |
//! | `apply_update_state`     | 每回合刷新属性快照（如将防御提高部分）             |

use std::any::TypeId;
use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::RunUpdates;
use crate::player::status::PlayerStatus;
use crate::player::{ActionTargets, OnDamageFunc, PlrId};
use crate::rc4::RC4;
use foldhash::HashMap as FastHashMap;
use smallvec::SmallVec;

/// 状态类型标识，使用稳定的类型名字符串。
/// 相比 `TypeId` 更容易跨动态库/插件边界做协议映射。
pub type StateTag = &'static str;

/// 返回类型 `T` 对应的状态标识（稳定类型名）。
#[inline]
pub fn state_tag<T: StateTrait + 'static>() -> StateTag { std::any::type_name::<T>() }

pub trait StateTrait: std::fmt::Debug + Send + Sync + 'static {
    /// 跨边界可读的稳定状态标识。
    fn state_tag(&self) -> StateTag { std::any::type_name::<Self>() }

    /// 用于类型安全转回具体状态类型。
    /// 仅在内部 `get/get_mut` 处校验后做指针转换。
    fn state_type_id(&self) -> TypeId { TypeId::of::<Self>() }

    fn meta_type(&self) -> i32 { 0 }
    fn clear_positive_priority(&self) -> i32 { 1000 }
    fn clear_updates_status(&self) -> bool { true }

    fn action_mode_priority(&self) -> i32 { 1000 }
    fn on_action_mode(&self, _smart: bool, _forced_attack: &mut Option<crate::player::action_targets::ForcedAttackConfig>) {}
    fn on_forced_action(
        &mut self,
        _owner: PlrId,
        _alive: bool,
        _randomer: &mut RC4,
        _updates: &mut RunUpdates,
        _storage: &Arc<Storage>,
    ) -> bool {
        false
    }

    fn update_state_priority(&self) -> i32 { 1000 }
    fn apply_update_state(&self, _status: &mut PlayerStatus) {}

    fn pre_step_priority(&self) -> i32 { 1000 }
    fn on_pre_step(&mut self, _owner: PlrId, _status: &PlayerStatus, _step: &mut i32, _updates: &mut RunUpdates) -> bool { false }

    fn pre_defend_priority(&self) -> i32 { 1000 }
    #[allow(clippy::too_many_arguments)]
    fn on_pre_defend(
        &mut self,
        _owner: PlrId,
        _atp: &mut f64,
        _is_mag: bool,
        _caster: PlrId,
        _on_damage: OnDamageFunc,
        _randomer: &mut RC4,
        _updates: &mut RunUpdates,
        _storage: &Arc<Storage>,
    ) -> bool {
        false
    }

    fn post_action_priority(&self) -> i32 { 1000 }
    fn on_post_action(
        &mut self,
        _owner: PlrId,
        _alive: bool,
        _randomer: &mut RC4,
        _updates: &mut RunUpdates,
        _storage: &Arc<Storage>,
    ) -> bool {
        false
    }

    fn post_defend_priority(&self) -> i32 { 1000 }
    fn on_post_defend(
        &mut self,
        _owner: PlrId,
        _dmg: &mut i32,
        _caster: PlrId,
        _randomer: &mut RC4,
        _updates: &mut RunUpdates,
        _storage: &Arc<Storage>,
    ) -> bool {
        false
    }

    fn post_damage_priority(&self) -> i32 { 1000 }
    fn on_post_damage(
        &mut self,
        _owner: PlrId,
        _dmg: i32,
        _caster: PlrId,
        _randomer: &mut RC4,
        _updates: &mut RunUpdates,
        _storage: &Arc<Storage>,
    ) {
    }

    fn pre_action_priority(&self) -> i32 { 1000 }
    #[allow(clippy::too_many_arguments)]
    fn on_pre_action(
        &mut self,
        _owner: PlrId,
        _smart: bool,
        _randomer: &mut RC4,
        _updates: &mut RunUpdates,
        _storage: &Arc<Storage>,
        _targets: &ActionTargets,
    ) -> bool {
        false
    }

    fn die_message_priority(&self) -> i32 { 1000 }
    fn die_message(&self) -> Option<&'static str> { None }

    fn cancel_message(&self, _alive: bool) -> Option<&'static str> { None }

    fn linked_owner(&self) -> Option<PlrId> { None }
    fn on_linked_owner_die(&mut self, _owner: PlrId, _self_id: PlrId, _updates: &mut RunUpdates) -> bool { false }

    fn clone_box(&self) -> Box<dyn StateTrait>;
}

impl Clone for Box<dyn StateTrait> {
    fn clone(&self) -> Self { self.clone_box() }
}

#[derive(Clone, Debug)]
pub(crate) struct StateEntry {
    pub(crate) state: Box<dyn StateTrait>,
    pub(crate) order: u64,
}

#[derive(Clone, Debug, Default)]
pub struct PlayerStateStore {
    pub(crate) entries: FastHashMap<StateTag, StateEntry>,
    next_state_order: u64,
}

type PriorityPairs = SmallVec<[(StateTag, i32, u64); 8]>;
type OrderedTagWithOrder = SmallVec<[(StateTag, u64); 8]>;

impl PlayerStateStore {
    #[inline]
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    #[inline]
    fn cast_ref<T: StateTrait + 'static>(state: &dyn StateTrait) -> Option<&T> {
        if state.state_type_id() != TypeId::of::<T>() {
            return None;
        }
        let ptr = state as *const dyn StateTrait as *const T;
        // SAFETY:
        // 1. 上面已通过 state_type_id 与 T::TypeId 做严格等值校验；
        // 2. 该 trait object 的底层动态类型即为 T；
        // 3. 生命周期由入参 `state` 保证。
        Some(unsafe { &*ptr })
    }

    #[inline]
    fn cast_mut<T: StateTrait + 'static>(state: &mut dyn StateTrait) -> Option<&mut T> {
        if state.state_type_id() != TypeId::of::<T>() {
            return None;
        }
        let ptr = state as *mut dyn StateTrait as *mut T;
        // SAFETY:
        // 与 cast_ref 同理，且 `state` 为独占可变引用，别名规则成立。
        Some(unsafe { &mut *ptr })
    }

    #[inline]
    fn remove_tag_internal(&mut self, tag: StateTag) -> Option<Box<dyn StateTrait>> {
        self.entries.remove(&tag).map(|entry| entry.state)
    }

    #[inline]
    fn ordered_pairs_by<F>(&self, mut priority: F) -> PriorityPairs
    where
        F: FnMut(&dyn StateTrait) -> i32,
    {
        let mut ordered: PriorityPairs = self
            .entries
            .iter()
            .map(|(tag, entry)| (*tag, priority(entry.state.as_ref()), entry.order))
            .collect();
        // JS 的 postAction 在同优先级下保持注册顺序；这里不能再退化成 tag 字典序，
        // 否则像 Slow / Iron 这类同层 timer 状态会在 Rust 侧固定成错误的结束顺序。
        ordered.sort_unstable_by(|(tag_a, priority_a, order_a), (tag_b, priority_b, order_b)| {
            priority_a
                .cmp(priority_b)
                .then_with(|| order_a.cmp(order_b))
                .then_with(|| tag_a.cmp(tag_b))
        });
        ordered
    }

    #[inline]
    fn ordered_tags_by<F>(&self, priority: F) -> SmallVec<[StateTag; 8]>
    where
        F: FnMut(&dyn StateTrait) -> i32,
    {
        let ordered = self.ordered_pairs_by(priority);
        ordered.into_iter().map(|(tag, _, _)| tag).collect()
    }

    #[inline]
    pub(crate) fn ordered_post_action_tags_with_order(&self) -> OrderedTagWithOrder {
        self.ordered_pairs_by(|state| state.post_action_priority())
            .into_iter()
            .map(|(tag, _, order)| (tag, order))
            .collect()
    }

    #[inline]
    pub(crate) fn post_action_registration_cursor(&self) -> u64 { self.next_state_order }

    #[inline]
    pub fn set<T: StateTrait + 'static>(&mut self, state: T) {
        let tag = state_tag::<T>();
        #[cfg(not(feature = "no_debug"))]
        let had = self.entries.contains_key(&tag);
        #[cfg(not(feature = "no_debug"))]
        if had && crate::debug::debug_state() {
            eprintln!(
                "[STATE_SET] OVERWRITING existing state tag={:?} meta_type={}",
                tag,
                state.meta_type()
            );
        }
        #[cfg(not(feature = "no_debug"))]
        if std::any::type_name::<T>().contains("CovidInfection") && crate::debug::debug_state() {
            eprintln!(
                "[STATE_TRACE] SET CovidInfection store_addr={:p} tag={:?}",
                self as *const _, tag
            );
        }
        let order = if let Some(existing) = self.entries.get(&tag) {
            existing.order
        } else {
            let order = self.next_state_order;
            self.next_state_order += 1;
            order
        };
        self.entries.insert(
            tag,
            StateEntry {
                state: Box::new(state),
                order,
            },
        );
    }

    #[inline]
    pub fn get<T: StateTrait + 'static>(&self) -> Option<&T> {
        let entry = self.entries.get(&state_tag::<T>())?;
        Self::cast_ref::<T>(entry.state.as_ref())
    }

    #[inline]
    pub fn get_mut<T: StateTrait + 'static>(&mut self) -> Option<&mut T> {
        let entry = self.entries.get_mut(&state_tag::<T>())?;
        Self::cast_mut::<T>(entry.state.as_mut())
    }

    #[inline]
    pub fn has<T: StateTrait + 'static>(&self) -> bool {
        let result = self.entries.contains_key(&state_tag::<T>());
        #[cfg(not(feature = "no_debug"))]
        if std::any::type_name::<T>().contains("CovidInfection") && crate::debug::debug_state() {
            eprintln!(
                "[STATE_TRACE] HAS CovidInfection store_addr={:p} result={} all_tags={:?}",
                self as *const _,
                result,
                self.entries.keys().collect::<Vec<_>>()
            );
        }
        result
    }

    #[inline]
    pub fn clear<T: StateTrait + 'static>(&mut self) {
        let tag = state_tag::<T>();
        #[cfg(not(feature = "no_debug"))]
        if self.entries.contains_key(&tag) && crate::debug::debug_state() {
            eprintln!("[STATE_CLEAR] removing tag={:?}", tag);
        }
        self.remove_tag_internal(tag);
    }

    #[inline]
    pub fn clear_tag(&mut self, tag: StateTag) {
        #[cfg(not(feature = "no_debug"))]
        if self.entries.contains_key(&tag) && crate::debug::debug_state() {
            eprintln!(
                "[STATE_CLEAR_TAG] removing tag={:?} meta_type={:?}",
                tag,
                self.entries.get(&tag).map(|entry| entry.state.meta_type())
            );
        }
        self.remove_tag_internal(tag);
    }

    #[inline]
    pub fn tag_clear_updates_status(&self, tag: StateTag) -> bool {
        self.entries.get(&tag).map(|entry| entry.state.clear_updates_status()).unwrap_or(true)
    }

    #[inline]
    pub fn meta_type(&self, tag: StateTag) -> Option<i32> { self.entries.get(&tag).map(|entry| entry.state.meta_type()) }

    pub fn clear_negative_states(&mut self) -> bool {
        #[cfg(not(feature = "no_debug"))]
        let debug_state = crate::debug::debug_state();
        let mut to_remove = Vec::new();
        let mut should_update_states = false;
        for (tag, entry) in self.entries.iter() {
            if entry.state.meta_type() < 0 {
                #[cfg(not(feature = "no_debug"))]
                if debug_state {
                    eprintln!("[CLEAR_NEG] removing tag={:?} meta_type={}", tag, entry.state.meta_type());
                }
                should_update_states |= entry.state.clear_updates_status();
                to_remove.push(*tag);
            }
        }
        for tag in to_remove {
            self.remove_tag_internal(tag);
        }
        should_update_states
    }

    pub fn clear_positive_states(&mut self) {
        let mut to_remove = Vec::new();
        for (tag, entry) in self.entries.iter() {
            if entry.state.meta_type() > 0 {
                to_remove.push(*tag);
            }
        }
        for tag in to_remove {
            self.remove_tag_internal(tag);
        }
    }

    pub fn clear_positive_states_with_ordered_messages(&mut self, alive: bool) -> Vec<(i32, &'static str)> {
        let mut to_remove = Vec::new();
        let mut messages = Vec::new();
        for (tag, entry) in self.entries.iter() {
            if entry.state.meta_type() > 0 {
                if let Some(msg) = entry.state.cancel_message(alive) {
                    messages.push((entry.state.clear_positive_priority(), entry.order, *tag, msg));
                }
                to_remove.push(*tag);
            }
        }
        // JS 中同优先级的正面状态清除消息按注册顺序排列，
        // 这里必须使用 registration_order 作为 tiebreaker，而不是只靠 tag 字典序。
        messages.sort_unstable_by(|(priority_a, order_a, tag_a, _), (priority_b, order_b, tag_b, _)| {
            priority_a
                .cmp(priority_b)
                .then_with(|| order_a.cmp(order_b))
                .then_with(|| tag_a.cmp(tag_b))
        });
        for tag in to_remove {
            self.remove_tag_internal(tag);
        }
        messages.into_iter().map(|(priority, _, _, message)| (priority, message)).collect()
    }

    pub fn clear_positive_states_with_messages(&mut self, alive: bool) -> Vec<&'static str> {
        self.clear_positive_states_with_ordered_messages(alive)
            .into_iter()
            .map(|(_, message)| message)
            .collect()
    }

    #[inline]
    pub fn negative_state_count(&self) -> usize { self.entries.values().filter(|entry| entry.state.meta_type() < 0).count() }

    pub fn apply_update_state_effects(&self, status: &mut PlayerStatus) {
        for tag in self.ordered_tags_by(|state| state.update_state_priority()) {
            if let Some(entry) = self.entries.get(&tag) {
                entry.state.apply_update_state(status);
            }
        }
    }

    pub fn resolve_action_mode(&self, smart: bool) -> Option<crate::player::action_targets::ForcedAttackConfig> {
        let mut forced = None;
        for tag in self.ordered_tags_by(|state| state.action_mode_priority()) {
            if let Some(entry) = self.entries.get(&tag) {
                entry.state.on_action_mode(smart, &mut forced);
                if forced.is_some() {
                    break;
                }
            }
        }
        forced
    }

    pub fn on_forced_action_states(
        &mut self,
        owner: PlrId,
        alive: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> Vec<StateTag> {
        let mut clear_tags = SmallVec::<[StateTag; 8]>::new();
        for tag in self.ordered_tags_by(|state| state.action_mode_priority()) {
            let should_clear = self
                .entries
                .get_mut(&tag)
                .map(|entry| entry.state.on_forced_action(owner, alive, randomer, updates, storage))
                .unwrap_or(false);
            if should_clear {
                clear_tags.push(tag);
            }
        }
        clear_tags.into_vec()
    }

    pub fn on_pre_step_states(
        &mut self,
        owner: PlrId,
        status: &PlayerStatus,
        step: &mut i32,
        updates: &mut RunUpdates,
    ) -> Vec<StateTag> {
        let mut clear_tags = SmallVec::<[StateTag; 8]>::new();
        for tag in self.ordered_tags_by(|state| state.pre_step_priority()) {
            let should_clear = self
                .entries
                .get_mut(&tag)
                .map(|entry| entry.state.on_pre_step(owner, status, step, updates))
                .unwrap_or(false);
            if should_clear {
                clear_tags.push(tag);
            }
        }
        clear_tags.into_vec()
    }

    pub fn on_post_action_states(
        &mut self,
        owner: PlrId,
        alive: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> Vec<StateTag> {
        let mut clear_tags = SmallVec::<[StateTag; 8]>::new();
        #[cfg(not(feature = "no_debug"))]
        let debug_this = storage
            .get_player(&owner)
            .map(|player| crate::debug::debug_action_matches(&player.id_name()))
            .unwrap_or(false);
        for tag in self.ordered_tags_by(|state| state.post_action_priority()) {
            // JS 的各状态 post_action 回调会动态检查当前 hp > 0，而非使用循环开始前的快照。
            // 因此每次迭代前重新从 storage 获取存活状态，避免前序状态（如 Poison）杀死玩家后
            // 后序状态（如 Charm）仍看到旧的 alive=true。
            let current_alive = storage.get_player(&owner).map(|p| p.alive()).unwrap_or(alive);
            #[cfg(not(feature = "no_debug"))]
            let rc4_before = (randomer.i, randomer.j);
            let should_clear = self
                .entries
                .get_mut(&tag)
                .map(|entry| entry.state.on_post_action(owner, current_alive, randomer, updates, storage))
                .unwrap_or(false);
            #[cfg(not(feature = "no_debug"))]
            if debug_this {
                eprintln!(
                    "[post_action_state] owner={} tag={:?} rc4 {}:{} -> {}:{} clear={} alive={}",
                    storage.get_player(&owner).map(|p| p.id_name()).unwrap_or_else(|| format!("#{}", owner)),
                    tag,
                    rc4_before.0,
                    rc4_before.1,
                    randomer.i,
                    randomer.j,
                    should_clear,
                    current_alive,
                );
            }
            if should_clear {
                clear_tags.push(tag);
            }
        }
        clear_tags.into_vec()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn on_pre_defend_states(
        &mut self,
        owner: PlrId,
        atp: &mut f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> Vec<StateTag> {
        let mut clear_tags = SmallVec::<[StateTag; 8]>::new();
        for tag in self.ordered_tags_by(|state| state.pre_defend_priority()) {
            let should_clear = self
                .entries
                .get_mut(&tag)
                .map(|entry| {
                    entry
                        .state
                        .on_pre_defend(owner, atp, is_mag, caster, on_damage, randomer, updates, storage)
                })
                .unwrap_or(false);
            if should_clear {
                clear_tags.push(tag);
            }
            if *atp == 0.0 {
                break;
            }
        }
        clear_tags.into_vec()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn on_pre_defend_state_tag(
        &mut self,
        tag: StateTag,
        owner: PlrId,
        atp: &mut f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> Vec<StateTag> {
        let Some(entry) = self.entries.get_mut(&tag) else {
            return Vec::new();
        };
        let should_clear = entry
            .state
            .on_pre_defend(owner, atp, is_mag, caster, on_damage, randomer, updates, storage);
        if should_clear { vec![tag] } else { Vec::new() }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn on_pre_defend_states_except_tag(
        &mut self,
        skip_tag: StateTag,
        owner: PlrId,
        atp: &mut f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> Vec<StateTag> {
        let mut clear_tags = SmallVec::<[StateTag; 8]>::new();
        for tag in self.ordered_tags_by(|state| state.pre_defend_priority()) {
            if tag == skip_tag {
                continue;
            }
            let should_clear = self
                .entries
                .get_mut(&tag)
                .map(|entry| {
                    entry
                        .state
                        .on_pre_defend(owner, atp, is_mag, caster, on_damage, randomer, updates, storage)
                })
                .unwrap_or(false);
            if should_clear {
                clear_tags.push(tag);
            }
            if *atp == 0.0 {
                break;
            }
        }
        clear_tags.into_vec()
    }

    pub fn on_post_defend_states(
        &mut self,
        owner: PlrId,
        dmg: &mut i32,
        caster: PlrId,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> Vec<StateTag> {
        let mut clear_tags = SmallVec::<[StateTag; 8]>::new();
        for tag in self.ordered_tags_by(|state| state.post_defend_priority()) {
            if let Some(entry) = self.entries.get_mut(&tag) {
                if entry.state.on_post_defend(owner, dmg, caster, randomer, updates, storage) {
                    clear_tags.push(tag);
                }
            }
        }
        clear_tags.into_vec()
    }

    /// 返回 post_defend 状态的 (tag, priority) 列表，用于和 skill 统一排序。
    ///
    /// JS 中 post_defend 回调同样通过 MList 管理，同优先级下保持注册顺序。
    /// 这里必须加入 registration_order 作为 tiebreaker，否则同优先级的 state
    /// 会按 HashMap 迭代顺序（非确定性）排列，导致与 JS 行为不一致。
    pub fn post_defend_tags_with_priority(&self) -> SmallVec<[(StateTag, i32); 8]> {
        let mut result: SmallVec<[(StateTag, i32, u64); 8]> = SmallVec::new();
        for (&tag, entry) in &self.entries {
            result.push((tag, entry.state.post_defend_priority(), entry.order));
        }
        result.sort_unstable_by(|(tag_a, p_a, o_a), (tag_b, p_b, o_b)| {
            p_a.cmp(p_b).then_with(|| o_a.cmp(o_b)).then_with(|| tag_a.cmp(tag_b))
        });
        result.into_iter().map(|(tag, priority, _)| (tag, priority)).collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_one_post_defend(
        &mut self,
        tag: StateTag,
        owner: PlrId,
        dmg: &mut i32,
        caster: PlrId,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> bool {
        if let Some(entry) = self.entries.get_mut(&tag) {
            entry.state.on_post_defend(owner, dmg, caster, randomer, updates, storage)
        } else {
            false
        }
    }

    pub fn on_post_damage_states(
        &mut self,
        owner: PlrId,
        dmg: i32,
        caster: PlrId,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) {
        #[cfg(not(feature = "no_debug"))]
        let debug_this = storage
            .get_player(&owner)
            .map(|player| crate::debug::debug_action_matches(&player.id_name()))
            .unwrap_or(false);
        for tag in self.ordered_tags_by(|state| state.post_damage_priority()) {
            #[cfg(not(feature = "no_debug"))]
            let rc4_before = (randomer.i, randomer.j);
            if let Some(entry) = self.entries.get_mut(&tag) {
                entry.state.on_post_damage(owner, dmg, caster, randomer, updates, storage);
                #[cfg(not(feature = "no_debug"))]
                if debug_this {
                    eprintln!(
                        "[post_damage_state] owner={} tag={:?} dmg={} caster={} rc4 {}:{} -> {}:{}",
                        storage.get_player(&owner).map(|p| p.id_name()).unwrap_or_else(|| format!("#{}", owner)),
                        tag,
                        dmg,
                        caster,
                        rc4_before.0,
                        rc4_before.1,
                        randomer.i,
                        randomer.j,
                    );
                }
            }
        }
    }

    pub fn on_pre_action_states(
        &mut self,
        owner: PlrId,
        smart: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
        targets: &ActionTargets,
    ) -> bool {
        for tag in self.ordered_tags_by(|state| state.pre_action_priority()) {
            if let Some(entry) = self.entries.get_mut(&tag)
                && entry.state.on_pre_action(owner, smart, randomer, updates, storage, targets)
            {
                return true;
            }
        }
        false
    }

    pub fn die_message_override(&self) -> Option<&'static str> {
        for tag in self.ordered_tags_by(|state| state.die_message_priority()) {
            if let Some(msg) = self.entries.get(&tag).and_then(|entry| entry.state.die_message()) {
                return Some(msg);
            }
        }
        None
    }

    pub fn linked_to_owner(&self, owner: PlrId) -> bool {
        self.entries
            .values()
            .any(|entry| entry.state.linked_owner().map(|id| id == owner).unwrap_or(false))
    }

    pub fn on_linked_owner_die(&mut self, owner: PlrId, self_id: PlrId, updates: &mut RunUpdates) -> bool {
        let mut should_remove = false;
        for entry in self.entries.values_mut() {
            should_remove |= entry.state.on_linked_owner_die(owner, self_id, updates);
        }
        should_remove
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct AlphaState;

    impl StateTrait for AlphaState {
        fn pre_action_priority(&self) -> i32 { 1000 }

        fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(self.clone()) }
    }

    #[derive(Debug, Clone)]
    struct BetaState;

    impl StateTrait for BetaState {
        fn pre_action_priority(&self) -> i32 { 1000 }

        fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(self.clone()) }
    }

    #[test]
    fn ordered_tags_follow_registration_order_when_priorities_match() {
        let mut store = PlayerStateStore::default();
        store.set(BetaState);
        store.set(AlphaState);

        let ordered = store.ordered_tags_by(|state| state.pre_action_priority());

        assert_eq!(ordered.into_vec(), vec![state_tag::<BetaState>(), state_tag::<AlphaState>()]);
    }

    #[test]
    fn readded_state_gets_new_registration_order() {
        let mut store = PlayerStateStore::default();
        store.set(BetaState);
        store.set(AlphaState);
        store.clear::<BetaState>();
        store.set(BetaState);

        let ordered = store.ordered_tags_by(|state| state.pre_action_priority());

        assert_eq!(ordered.into_vec(), vec![state_tag::<AlphaState>(), state_tag::<BetaState>()]);
    }
}

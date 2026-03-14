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

use std::any::{Any, TypeId};
use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::RunUpdates;
use crate::player::status::PlayerStatus;
use crate::player::{ActionTargets, OnDamageFunc, PlrId};
use crate::rc4::RC4;
use foldhash::HashMap as FastHashMap;

/// 状态类型标识，内部使用 [`TypeId`] 区分不同状态，与 Dart 的 `Type<T>` 类似。
pub type StateTag = TypeId;

/// 返回类型 `T` 对应的状态标识。
#[inline]
pub fn state_tag<T: StateTrait + 'static>() -> StateTag { TypeId::of::<T>() }

pub trait StateTrait: std::fmt::Debug + Any + Send + Sync {
    fn meta_type(&self) -> i32 { 0 }

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
    ) {
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

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn clone_box(&self) -> Box<dyn StateTrait>;
}

impl Clone for Box<dyn StateTrait> {
    fn clone(&self) -> Self { self.clone_box() }
}

#[derive(Clone, Debug, Default)]
pub struct PlayerStateStore {
    pub(crate) states: FastHashMap<StateTag, Box<dyn StateTrait>>,
}

impl PlayerStateStore {
    #[inline]
    pub fn set<T: StateTrait + 'static>(&mut self, state: T) {
        let tag = state_tag::<T>();
        #[cfg(not(feature = "no_debug"))]
        let had = self.states.contains_key(&tag);
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
        self.states.insert(tag, Box::new(state));
    }

    #[inline]
    pub fn get<T: StateTrait + 'static>(&self) -> Option<&T> { self.states.get(&state_tag::<T>())?.as_any().downcast_ref::<T>() }

    #[inline]
    pub fn get_mut<T: StateTrait + 'static>(&mut self) -> Option<&mut T> {
        self.states.get_mut(&state_tag::<T>())?.as_any_mut().downcast_mut::<T>()
    }

    #[inline]
    pub fn has<T: StateTrait + 'static>(&self) -> bool {
        let result = self.states.contains_key(&state_tag::<T>());
        #[cfg(not(feature = "no_debug"))]
        if std::any::type_name::<T>().contains("CovidInfection") && crate::debug::debug_state() {
            eprintln!(
                "[STATE_TRACE] HAS CovidInfection store_addr={:p} result={} all_tags={:?}",
                self as *const _,
                result,
                self.states.keys().collect::<Vec<_>>()
            );
        }
        result
    }

    #[inline]
    pub fn clear<T: StateTrait + 'static>(&mut self) {
        let tag = state_tag::<T>();
        #[cfg(not(feature = "no_debug"))]
        if self.states.contains_key(&tag) && crate::debug::debug_state() {
            eprintln!("[STATE_CLEAR] removing tag={:?}", tag);
        }
        self.states.remove(&tag);
    }

    #[inline]
    pub fn clear_tag(&mut self, tag: StateTag) {
        #[cfg(not(feature = "no_debug"))]
        if self.states.contains_key(&tag) && crate::debug::debug_state() {
            eprintln!(
                "[STATE_CLEAR_TAG] removing tag={:?} meta_type={:?}",
                tag,
                self.states.get(&tag).map(|s| s.meta_type())
            );
        }
        self.states.remove(&tag);
    }

    #[inline]
    pub fn meta_type(&self, tag: StateTag) -> Option<i32> { self.states.get(&tag).map(|state| state.meta_type()) }

    pub fn clear_negative_states(&mut self) {
        #[cfg(not(feature = "no_debug"))]
        let debug_state = crate::debug::debug_state();
        let mut to_remove = Vec::new();
        for (tag, state) in self.states.iter() {
            if state.meta_type() < 0 {
                #[cfg(not(feature = "no_debug"))]
                if debug_state {
                    eprintln!("[CLEAR_NEG] removing tag={:?} meta_type={}", tag, state.meta_type());
                }
                to_remove.push(*tag);
            }
        }
        for tag in to_remove {
            self.states.remove(&tag);
        }
    }

    pub fn clear_positive_states(&mut self) {
        let mut to_remove = Vec::new();
        for (tag, state) in self.states.iter() {
            if state.meta_type() > 0 {
                to_remove.push(*tag);
            }
        }
        for tag in to_remove {
            self.states.remove(&tag);
        }
    }

    pub fn clear_positive_states_with_messages(&mut self, alive: bool) -> Vec<&'static str> {
        let mut to_remove = Vec::new();
        let mut messages = Vec::new();
        for (tag, state) in self.states.iter() {
            if state.meta_type() > 0 {
                if let Some(msg) = state.cancel_message(alive) {
                    messages.push(msg);
                }
                to_remove.push(*tag);
            }
        }
        for tag in to_remove {
            self.states.remove(&tag);
        }
        messages
    }

    #[inline]
    pub fn negative_state_count(&self) -> usize { self.states.values().filter(|state| state.meta_type() < 0).count() }

    pub fn apply_update_state_effects(&self, status: &mut PlayerStatus) {
        let mut ordered = self
            .states
            .iter()
            .map(|(tag, state)| (*tag, state.update_state_priority()))
            .collect::<Vec<(StateTag, i32)>>();
        ordered.sort_unstable_by_key(|(_, priority)| *priority);
        for (tag, _) in ordered {
            if let Some(state) = self.states.get(&tag) {
                state.apply_update_state(status);
            }
        }
    }

    pub fn resolve_action_mode(&self, smart: bool) -> Option<crate::player::action_targets::ForcedAttackConfig> {
        let mut ordered = self
            .states
            .iter()
            .map(|(tag, state)| (*tag, state.action_mode_priority()))
            .collect::<Vec<(StateTag, i32)>>();
        ordered.sort_unstable_by_key(|(_, priority)| *priority);
        let mut forced = None;
        for (tag, _) in ordered {
            if let Some(state) = self.states.get(&tag) {
                state.on_action_mode(smart, &mut forced);
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
        let mut ordered = self
            .states
            .iter()
            .map(|(tag, state)| (*tag, state.action_mode_priority()))
            .collect::<Vec<(StateTag, i32)>>();
        ordered.sort_unstable_by_key(|(_, priority)| *priority);
        let mut clear_tags = Vec::new();
        for (tag, _) in ordered {
            let should_clear = self
                .states
                .get_mut(&tag)
                .map(|state| state.on_forced_action(owner, alive, randomer, updates, storage))
                .unwrap_or(false);
            if should_clear {
                clear_tags.push(tag);
            }
        }
        clear_tags
    }

    pub fn on_pre_step_states(
        &mut self,
        owner: PlrId,
        status: &PlayerStatus,
        step: &mut i32,
        updates: &mut RunUpdates,
    ) -> Vec<StateTag> {
        let mut ordered = self
            .states
            .iter()
            .map(|(tag, state)| (*tag, state.pre_step_priority()))
            .collect::<Vec<(StateTag, i32)>>();
        ordered.sort_unstable_by_key(|(_, priority)| *priority);
        let mut clear_tags = Vec::new();
        for (tag, _) in ordered {
            let should_clear = self
                .states
                .get_mut(&tag)
                .map(|state| state.on_pre_step(owner, status, step, updates))
                .unwrap_or(false);
            if should_clear {
                clear_tags.push(tag);
            }
        }
        clear_tags
    }

    pub fn on_post_action_states(
        &mut self,
        owner: PlrId,
        alive: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> Vec<StateTag> {
        let mut ordered = self
            .states
            .iter()
            .map(|(tag, state)| (*tag, state.post_action_priority()))
            .collect::<Vec<(StateTag, i32)>>();
        ordered.sort_unstable_by_key(|(_, priority)| *priority);
        let mut clear_tags = Vec::new();
        for (tag, _) in ordered {
            let should_clear = self
                .states
                .get_mut(&tag)
                .map(|state| state.on_post_action(owner, alive, randomer, updates, storage))
                .unwrap_or(false);
            if should_clear {
                clear_tags.push(tag);
            }
        }
        clear_tags
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
        let mut ordered = self
            .states
            .iter()
            .map(|(tag, state)| (*tag, state.pre_defend_priority()))
            .collect::<Vec<(StateTag, i32)>>();
        ordered.sort_unstable_by_key(|(_, priority)| *priority);
        let mut clear_tags = Vec::new();
        for (tag, _) in ordered {
            let should_clear = self
                .states
                .get_mut(&tag)
                .map(|state| state.on_pre_defend(owner, atp, is_mag, caster, on_damage, randomer, updates, storage))
                .unwrap_or(false);
            if should_clear {
                clear_tags.push(tag);
            }
            if *atp == 0.0 {
                break;
            }
        }
        clear_tags
    }

    pub fn on_post_defend_states(
        &mut self,
        owner: PlrId,
        dmg: &mut i32,
        caster: PlrId,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) {
        let mut ordered = self
            .states
            .iter()
            .map(|(tag, state)| (*tag, state.post_defend_priority()))
            .collect::<Vec<(StateTag, i32)>>();
        ordered.sort_unstable_by_key(|(_, priority)| *priority);
        for (tag, _) in ordered {
            if let Some(state) = self.states.get_mut(&tag) {
                state.on_post_defend(owner, dmg, caster, randomer, updates, storage);
            }
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
        let mut ordered = self
            .states
            .iter()
            .map(|(tag, state)| (*tag, state.post_damage_priority()))
            .collect::<Vec<(StateTag, i32)>>();
        ordered.sort_unstable_by_key(|(_, priority)| *priority);
        for (tag, _) in ordered {
            if let Some(state) = self.states.get_mut(&tag) {
                state.on_post_damage(owner, dmg, caster, randomer, updates, storage);
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
        let mut ordered = self
            .states
            .iter()
            .map(|(tag, state)| (*tag, state.pre_action_priority()))
            .collect::<Vec<(StateTag, i32)>>();
        ordered.sort_unstable_by_key(|(_, priority)| *priority);
        for (tag, _) in ordered {
            if let Some(state) = self.states.get_mut(&tag)
                && state.on_pre_action(owner, smart, randomer, updates, storage, targets)
            {
                return true;
            }
        }
        false
    }

    pub fn die_message_override(&self) -> Option<&'static str> {
        let mut ordered = self
            .states
            .iter()
            .map(|(tag, state)| (*tag, state.die_message_priority()))
            .collect::<Vec<(StateTag, i32)>>();
        ordered.sort_unstable_by_key(|(_, priority)| *priority);
        for (tag, _) in ordered {
            if let Some(msg) = self.states.get(&tag).and_then(|state| state.die_message()) {
                return Some(msg);
            }
        }
        None
    }

    pub fn linked_to_owner(&self, owner: PlrId) -> bool {
        self.states
            .values()
            .any(|state| state.linked_owner().map(|id| id == owner).unwrap_or(false))
    }

    pub fn on_linked_owner_die(&mut self, owner: PlrId, self_id: PlrId, updates: &mut RunUpdates) -> bool {
        let mut should_remove = false;
        for state in self.states.values_mut() {
            should_remove |= state.on_linked_owner_die(owner, self_id, updates);
        }
        should_remove
    }
}

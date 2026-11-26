//! 状态存储、注册序及查询缓存；不参与载荷构造。

use super::*;

/// 压成位标记的 legacy 状态类别：护盾、保护、垂死、尸体与使魔。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressedLegacyState {
    Shield,
    Protect,
    Upgrade,
    Corpse,
    Minion,
}

impl CompressedLegacyState {
    /// 把压缩状态类别换算为位标记（`1 << 序号`）。
    fn bit(self) -> u8 { 1 << self as u8 }
}

/// 调度器快速否定位：存在冰冻类状态。
const SCHEDULER_STATE_ICE: u8 = 1 << 0;
/// 调度器快速否定位：存在影响速度类的状态。
const SCHEDULER_STATE_SPEED: u8 = 1 << 1;
/// 调度器关心的全部状态位。
const SCHEDULER_STATE_ALL: u8 = SCHEDULER_STATE_ICE | SCHEDULER_STATE_SPEED;

/// [`StatePayload`] 的种类标记，用于对状态表做"肯定不存在"的快速否定判断。
///
/// 战斗里有一批查询（懒惰 boss 加成、感染、魅惑改队、诅咒攻和、铁壁吸引）
/// 每场要跑几十次，但绝大多数实体根本没有对应状态。先测一位再决定是否遍历，
/// 可以把这些扫描的常见情况压成一次按位与。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatePayloadKind {
    None,
    FireMagHalfSteps,
    Ice,
    ShieldValue,
    Curse,
    Poison,
    Haste,
    Berserk,
    Charm,
    Slow,
    Iron,
    CovidBoss,
    CovidInfection,
    SaitamaBoss,
    LazyBoss,
    LazyInfection,
    MarioBoss,
    SkullFracture,
    TestSubjectNoEntity,
}

impl StatePayloadKind {
    /// 该种类对应的位图位，用于 `may_contain` 快速否定。
    #[inline]
    pub const fn bit(self) -> u32 { 1 << self as u32 }

    #[inline]
    const fn of(payload: &StatePayload) -> Self {
        match payload {
            StatePayload::None => Self::None,
            StatePayload::FireMagHalfSteps(_) => Self::FireMagHalfSteps,
            StatePayload::Ice { .. } => Self::Ice,
            StatePayload::ShieldValue(_) => Self::ShieldValue,
            StatePayload::Curse { .. } => Self::Curse,
            StatePayload::Poison { .. } => Self::Poison,
            StatePayload::Haste { .. } => Self::Haste,
            StatePayload::Berserk { .. } => Self::Berserk,
            StatePayload::Charm { .. } => Self::Charm,
            StatePayload::Slow { .. } => Self::Slow,
            StatePayload::Iron { .. } => Self::Iron,
            StatePayload::CovidBoss { .. } => Self::CovidBoss,
            StatePayload::CovidInfection { .. } => Self::CovidInfection,
            StatePayload::SaitamaBoss { .. } => Self::SaitamaBoss,
            StatePayload::LazyBoss { .. } => Self::LazyBoss,
            StatePayload::LazyInfection { .. } => Self::LazyInfection,
            StatePayload::MarioBoss { .. } => Self::MarioBoss,
            StatePayload::SkullFracture { .. } => Self::SkullFracture,
            StatePayload::TestSubjectNoEntity => Self::TestSubjectNoEntity,
        }
    }
}

/// 所有种类都可能存在；用于无法确定新 payload 种类的保守路径。
const PAYLOAD_KIND_ALL: u32 = u32::MAX;

#[inline]
const fn payload_kind_bit(payload: &StatePayload) -> u32 { StatePayloadKind::of(payload).bit() }

#[inline]
fn scheduler_state_flags(payload: &StatePayload) -> u8 {
    match payload {
        StatePayload::Ice { .. } => SCHEDULER_STATE_ICE,
        StatePayload::Haste { .. } | StatePayload::Slow { .. } | StatePayload::LazyInfection { .. } => SCHEDULER_STATE_SPEED,
        _ => 0,
    }
}

/// 实体的状态存储：条目、hook 掩码、注册序与查询缓存。
#[derive(Debug, Default, Clone)]
pub struct StateStore {
    entries: SmallVec<[StateEntry; 8]>,
    hook_mask: ProcMask,
    generation: u32,
    /// 与 `entries` 同下标保存运行期注册顺序，常见状态数不超过内联容量。
    runtime_registration_orders: SmallVec<[u64; 8]>,
    next_runtime_registration_order: u64,
    /// 蓄力本次入列时的游标；蓄力保存在专用字段，不占普通状态登记槽。
    charge_post_action_cursor: u64,
    assassinate_pre_action_cursor: u64,
    compressed_legacy_states: u8,
    /// 调度器只关心冻结和速度类状态；无对应位时可跳过完整 entries 扫描。
    scheduler_state_flags: u8,
    /// 当前可能存在的 [`StatePayloadKind`] 位图，允许是真实集合的超集。
    payload_kinds: u32,
}

impl PartialEq for StateStore {
    /// 状态存储相等：比较条目、hook 掩码、代数、注册序与压缩位标记等全部持久字段。
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
            && self.hook_mask == other.hook_mask
            && self.generation == other.generation
            && self.runtime_registration_orders == other.runtime_registration_orders
            && self.next_runtime_registration_order == other.next_runtime_registration_order
            && self.charge_post_action_cursor == other.charge_post_action_cursor
            && self.assassinate_pre_action_cursor == other.assassinate_pre_action_cursor
            && self.compressed_legacy_states == other.compressed_legacy_states
    }
}

impl Eq for StateStore {}

impl StateStore {
    pub fn model_compressed_flags(&self) -> u8 { self.compressed_legacy_states }

    /// 按登记顺序返回状态条目。
    pub fn entries(&self) -> &[StateEntry] { &self.entries }

    /// 恢复场前状态并复用现有 SmallVec 容量，避免通用 Clone 反复切换内联/堆存储。
    pub fn reset_battle_state_from(&mut self, prepared: &Self) {
        self.entries.clear();
        self.entries.extend(prepared.entries.iter().cloned());
        self.hook_mask = prepared.hook_mask;
        self.generation = prepared.generation;
        self.runtime_registration_orders.clear();
        self.runtime_registration_orders.extend_from_slice(&prepared.runtime_registration_orders);
        self.next_runtime_registration_order = prepared.next_runtime_registration_order;
        self.charge_post_action_cursor = prepared.charge_post_action_cursor;
        self.assassinate_pre_action_cursor = prepared.assassinate_pre_action_cursor;
        self.compressed_legacy_states = prepared.compressed_legacy_states;
        self.scheduler_state_flags = prepared.scheduler_state_flags;
        self.payload_kinds = prepared.payload_kinds;
    }

    /// 清空数字 score profile 的战斗状态，同时保留 SmallVec 容量。
    pub fn clear_score_profile_for_reuse(&mut self) {
        self.entries.clear();
        self.hook_mask = ProcMask::NONE;
        self.generation = 0;
        if !self.runtime_registration_orders.is_empty() {
            self.runtime_registration_orders.clear();
        }
        self.next_runtime_registration_order = 0;
        self.charge_post_action_cursor = 0;
        self.assassinate_pre_action_cursor = 0;
        self.compressed_legacy_states = 0;
        self.scheduler_state_flags = 0;
        self.payload_kinds = 0;
    }

    /// 全部条目 hook 掩码的并集。
    pub fn hook_mask(&self) -> ProcMask { self.hook_mask }

    /// 状态表里是否**可能**存在该种类的 payload。
    ///
    /// 位图允许是真实集合的超集：返回 `false` 一定不存在，返回 `true` 仍需遍历确认。
    #[inline]
    pub fn may_contain(&self, kind: StatePayloadKind) -> bool { self.payload_kinds & kind.bit() != 0 }

    /// 状态表代数；条目结构变化时递增。
    pub fn generation(&self) -> u32 { self.generation }

    /// 下一个运行期注册序号，供 POST_ACTION 期间追加登记。
    pub fn post_action_registration_cursor(&self) -> u64 { self.next_runtime_registration_order }

    pub fn reserve_runtime_registration_order(&mut self) -> u64 {
        let order = self.next_runtime_registration_order;
        self.next_runtime_registration_order = order.wrapping_add(1);
        self.generation = self.generation.wrapping_add(1);
        order
    }

    pub fn register_charge_post_action(&mut self) { self.charge_post_action_cursor = self.next_runtime_registration_order; }

    pub fn charge_post_action_cursor(&self) -> u64 { self.charge_post_action_cursor }

    pub fn register_assassinate_pre_action(&mut self) {
        self.assassinate_pre_action_cursor = self.reserve_runtime_registration_order();
    }

    pub fn assassinate_pre_action_cursor(&self) -> u64 { self.assassinate_pre_action_cursor }

    /// 按兼容键查运行期注册顺序；键不存在返回 None。
    pub fn runtime_registration_order(&self, legacy_order_key: u32) -> Option<u64> {
        let index = self.entries.iter().position(|entry| entry.legacy_order_key == legacy_order_key)?;
        self.runtime_registration_orders.get(index).copied()
    }

    /// 按 `entries()` 的下标顺序返回运行期注册顺序。
    ///
    /// 需要为每个条目取注册顺序的调用方应该直接 zip 这两个切片；
    /// 逐条走 [`Self::runtime_registration_order`] 会退化成 O(n^2)。
    pub fn runtime_registration_orders(&self) -> &[u64] { &self.runtime_registration_orders }

    /// 登记压缩状态位并占用一个注册序号；已设置返回 false。
    pub fn register_compressed_legacy_state(&mut self, state: CompressedLegacyState) -> bool {
        let bit = state.bit();
        if self.compressed_legacy_states & bit != 0 {
            return false;
        }
        // 有些 legacy State 在 Main 中被压成专用 runtime 字段，虽然不再需要执行通用钩子，
        // 但它们仍会占据统一状态注册队列的位置，影响 Merge 中途新增钩子的先后顺序。
        self.compressed_legacy_states |= bit;
        self.next_runtime_registration_order = self.next_runtime_registration_order.wrapping_add(1);
        true
    }

    /// 清除压缩状态位；此前未设置返回 false。
    pub fn clear_compressed_legacy_state(&mut self, state: CompressedLegacyState) -> bool {
        let bit = state.bit();
        if self.compressed_legacy_states & bit == 0 {
            return false;
        }
        self.compressed_legacy_states &= !bit;
        true
    }

    /// 是否被冰冻而无法行动。
    pub fn is_frozen(&self) -> bool {
        self.scheduler_state_flags & SCHEDULER_STATE_ICE != 0
            && self.entries.iter().any(|entry| matches!(&entry.payload, StatePayload::Ice { .. }))
    }

    /// 应用疾走、缓慢与懒羊羊传染后的有效速度。
    pub fn effective_speed(&self, base_speed: i32) -> i32 {
        if self.scheduler_state_flags & SCHEDULER_STATE_SPEED == 0 {
            return base_speed;
        }
        let mut speed = base_speed;
        let mut lazy_infections = 0;
        for entry in &self.entries {
            match &entry.payload {
                StatePayload::Haste { effective_faster, .. } => speed *= *effective_faster,
                StatePayload::Slow { .. } => speed /= 2,
                StatePayload::LazyInfection { .. } => lazy_infections += 1,
                _ => {}
            }
        }
        for _ in 0..lazy_infections {
            speed /= 2;
        }
        speed
    }

    /// 把待生效的疾走加速提交为有效值，让本回合之后开始生效。
    pub fn refresh_effective_haste_faster(&mut self) {
        let mut changed = false;
        for entry in &mut self.entries {
            let StatePayload::Haste {
                faster,
                effective_faster,
                ..
            } = &mut entry.payload
            else {
                continue;
            };
            if *effective_faster != *faster {
                *effective_faster = *faster;
                changed = true;
            }
        }
        if changed {
            self.generation = self.generation.wrapping_add(1);
        }
    }

    /// 将仍挂载的魅惑来源队伍提交为有效队伍；缺失来源的兼容载荷保持原值。
    pub fn refresh_effective_charm_team(&mut self) {
        if !self.may_contain(StatePayloadKind::Charm) {
            return;
        }
        let mut changed = false;
        for entry in &mut self.entries {
            if let StatePayload::Charm {
                effective_team_idx,
                source_team_idx: Some(source_team_idx),
                ..
            } = &mut entry.payload
                && *effective_team_idx != Some(*source_team_idx)
            {
                *effective_team_idx = Some(*source_team_idx);
                changed = true;
            }
        }
        if changed {
            self.generation = self.generation.wrapping_add(1);
        }
    }

    /// 应用诅咒倍率后的攻击总和。
    pub fn effective_atk_sum(&self, base_atk_sum: i32) -> i32 {
        if !self.may_contain(StatePayloadKind::Curse) {
            return base_atk_sum;
        }
        self.entries.iter().fold(base_atk_sum, |atk_sum, entry| {
            if matches!(entry.payload, StatePayload::Curse { .. }) {
                atk_sum.saturating_mul(4)
            } else {
                atk_sum
            }
        })
    }

    /// 应用铁壁层数后的吸引力。
    pub fn effective_attract(&self, base_attract: f64) -> f64 {
        if !self.may_contain(StatePayloadKind::Iron) {
            return base_attract;
        }
        self.entries.iter().fold(base_attract, |attract, entry| match entry.payload {
            StatePayload::Iron { step, .. } if step > 0 => attract * 1.1200000047683716,
            _ => attract,
        })
    }

    /// 按兼容键查状态条目。
    pub fn entry(&self, legacy_order_key: u32) -> Option<&StateEntry> {
        self.entries.iter().find(|entry| entry.legacy_order_key == legacy_order_key)
    }

    /// 按兼容键取可变条目；取得后查询缓存退化为保守全集。
    pub fn entry_mut(&mut self, legacy_order_key: u32) -> Option<&mut StateEntry> {
        let index = self.entries.iter().position(|entry| entry.legacy_order_key == legacy_order_key)?;
        // 调用方可通过公开可变引用替换 payload；升为保守全集，确保快路只会少命中而不会误跳过。
        self.scheduler_state_flags = SCHEDULER_STATE_ALL;
        self.payload_kinds = PAYLOAD_KIND_ALL;
        self.entries.get_mut(index)
    }

    /// 读取火焰精通倍率；状态不存在时为 0。
    pub fn fire_mag(&self, legacy_order_key: u32) -> f64 {
        self.entry(legacy_order_key).and_then(StateEntry::fire_mag_value).unwrap_or(0.0)
    }

    /// 读取冰冻剩余步数；没有冰冻状态返回 None。
    pub fn ice_frozen_step(&self, legacy_order_key: u32) -> Option<i32> {
        self.entry(legacy_order_key).and_then(StateEntry::ice_value)
    }

    /// 累加冰冻步数；尚无冰冻状态时新建条目。
    pub fn add_ice_frozen_step(&mut self, legacy_order_key: u32, frozen_step: i32) {
        if let Some(entry) = self.entry_mut(legacy_order_key) {
            let current = match &entry.payload {
                StatePayload::Ice { frozen_step } => *frozen_step,
                _ => 0,
            };
            entry.payload = StatePayload::Ice {
                frozen_step: current + frozen_step,
            };
            self.generation = self.generation.wrapping_add(1);
            return;
        }

        self.add_entry(StateEntry::ice(legacy_order_key, frozen_step));
    }

    /// 行动前结算冰冻：返回 (剩余步数, 本次是否解除冰冻)。
    pub fn apply_ice_pre_step(&mut self, step: i32, move_points: i32) -> (i32, bool) {
        if step <= 0 || self.scheduler_state_flags & SCHEDULER_STATE_ICE == 0 {
            return (step, false);
        }

        let Some((legacy_order_key, frozen_step)) = self.entries.iter_mut().find_map(|entry| {
            if let StatePayload::Ice { frozen_step } = &mut entry.payload {
                Some((entry.legacy_order_key, frozen_step))
            } else {
                None
            }
        }) else {
            return (step, false);
        };

        if *frozen_step > 0 {
            *frozen_step -= step;
            self.generation = self.generation.wrapping_add(1);
            return (0, false);
        }
        if step + move_points >= MOVE_POINT_THRESHOLD {
            assert!(
                self.clear_legacy_key(legacy_order_key),
                "runtime ice state disappeared during pre-step"
            );
            // legacy 清除冰冻后会调用 update_states；这一步也会让疾走等待生效的
            // 强化倍率立即提交，不能只删除 Ice 状态。
            self.refresh_effective_haste_faster();
            return (0, true);
        }
        (step, false)
    }

    /// 累加火焰精通半步；尚无该状态时新建条目。
    pub fn add_fire_mag_half_step(&mut self, legacy_order_key: u32) {
        if let Some(idx) = self.entries.iter().position(|entry| entry.legacy_order_key == legacy_order_key)
            && let Some(entry) = self.entries.get_mut(idx)
        {
            match &mut entry.payload {
                StatePayload::FireMagHalfSteps(half_steps) => {
                    *half_steps += 1;
                }
                StatePayload::None
                | StatePayload::Ice { .. }
                | StatePayload::ShieldValue(_)
                | StatePayload::Curse { .. }
                | StatePayload::Iron { .. }
                | StatePayload::CovidBoss { .. }
                | StatePayload::CovidInfection { .. }
                | StatePayload::SaitamaBoss { .. }
                | StatePayload::LazyBoss { .. }
                | StatePayload::LazyInfection { .. }
                | StatePayload::MarioBoss { .. } => {
                    entry.payload = StatePayload::FireMagHalfSteps(1);
                }
                StatePayload::Poison { .. }
                | StatePayload::Haste { .. }
                | StatePayload::Berserk { .. }
                | StatePayload::Charm { .. }
                | StatePayload::Slow { .. }
                | StatePayload::SkullFracture { .. }
                | StatePayload::TestSubjectNoEntity => {
                    entry.payload = StatePayload::FireMagHalfSteps(1);
                }
            }
            self.rebuild_cached_metadata();
            self.generation = self.generation.wrapping_add(1);
            return;
        }

        self.add_entry(StateEntry::fire_mag(legacy_order_key, 1));
    }

    /// 设置护盾值，负值按 0；键不存在返回 false。
    pub fn set_shield_value(&mut self, legacy_order_key: u32, shield: i32) -> bool {
        self.set_payload(legacy_order_key, StatePayload::ShieldValue(shield.max(0)))
    }

    /// 替换指定键的载荷并刷新缓存；键不存在返回 false。
    pub fn set_payload(&mut self, legacy_order_key: u32, payload: StatePayload) -> bool {
        let Some(index) = self.entries.iter().position(|entry| entry.legacy_order_key == legacy_order_key) else {
            return false;
        };
        self.entries[index].payload = payload;
        self.rebuild_cached_metadata();
        self.generation = self.generation.wrapping_add(1);
        true
    }

    /// 添加一个空 legacy 状态条目；同键已存在返回 false。
    pub fn add_legacy_key(&mut self, legacy_order_key: u32) -> bool { self.add_entry(StateEntry::legacy(legacy_order_key)) }

    /// 添加状态条目并更新缓存；同键已存在返回 false。
    pub fn add_entry(&mut self, entry: StateEntry) -> bool {
        if self.entries.iter().any(|current| current.legacy_order_key == entry.legacy_order_key) {
            return false;
        }

        let runtime_registration_order = self.next_runtime_registration_order;
        self.next_runtime_registration_order = self.next_runtime_registration_order.wrapping_add(1);
        self.runtime_registration_orders.push(runtime_registration_order);
        self.hook_mask |= entry.hook_mask;
        self.scheduler_state_flags |= scheduler_state_flags(&entry.payload);
        self.payload_kinds |= payload_kind_bit(&entry.payload);
        self.entries.push(entry);
        self.generation = self.generation.wrapping_add(1);
        true
    }

    /// 按兼容键移除条目并刷新缓存；键不存在返回 false。
    pub fn clear_legacy_key(&mut self, legacy_order_key: u32) -> bool {
        let Some(idx) = self.entries.iter().position(|entry| entry.legacy_order_key == legacy_order_key) else {
            return false;
        };

        self.entries.remove(idx);
        self.runtime_registration_orders.remove(idx);
        self.rebuild_cached_metadata();
        self.generation = self.generation.wrapping_add(1);
        true
    }

    /// 清除全部正面状态，返回按优先级与注册序排序的解除消息。
    pub fn clear_positive_states_with_ordered_messages(&mut self, owner_alive: bool) -> Vec<(i32, &'static str)> {
        let mut messages = Vec::new();
        let mut to_remove = Vec::new();

        for entry in &self.entries {
            if entry.is_positive_state() {
                if let Some(message) = entry.positive_clear_message(owner_alive) {
                    messages.push((message.0, entry.registration_order, entry.legacy_order_key, message.1));
                }
                to_remove.push(entry.legacy_order_key);
            }
        }

        messages.sort_unstable_by(|(priority_a, order_a, key_a, _), (priority_b, order_b, key_b, _)| {
            priority_a
                .cmp(priority_b)
                .then_with(|| order_a.cmp(order_b))
                .then_with(|| key_a.cmp(key_b))
        });
        for legacy_order_key in to_remove {
            self.clear_legacy_key(legacy_order_key);
        }
        messages.into_iter().map(|(priority, _, _, message)| (priority, message)).collect()
    }

    /// 按优先级与注册序排序的全部条目。
    pub fn entries_in_hook_order(&self) -> SmallVec<[&StateEntry; 8]> {
        let mut entries: SmallVec<[&StateEntry; 8]> = self.entries.iter().collect();
        entries.sort_by_key(|entry| (entry.priority, entry.registration_order));
        entries
    }

    /// 过滤出参与指定 hook 的条目，按该 hook 的优先级排序。
    pub fn entries_in_hook_order_for(&self, hook: ProcMask) -> SmallVec<[&StateEntry; 8]> {
        let mut entries: SmallVec<[&StateEntry; 8]> =
            self.entries.iter().filter(|entry| entry.hook_mask.intersects(hook)).collect();
        entries.sort_by_key(|entry| (entry.priority_for_hook(hook), entry.registration_order));
        entries
    }

    /// 由全部条目重建缓存元数据：聚合 hook 掩码、调度器状态位与载荷种类位图。
    fn rebuild_cached_metadata(&mut self) {
        let mut hook_mask = ProcMask::default();
        let mut state_flags = 0u8;
        let mut payload_kinds = 0u32;
        for entry in &self.entries {
            hook_mask |= entry.hook_mask;
            state_flags |= scheduler_state_flags(&entry.payload);
            payload_kinds |= payload_kind_bit(&entry.payload);
        }
        self.hook_mask = hook_mask;
        self.scheduler_state_flags = state_flags;
        self.payload_kinds = payload_kinds;
    }
}

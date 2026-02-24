pub mod eval_name;
pub mod skill;
pub mod utils;
pub mod weapons;

use std::any::{Any, TypeId};
use std::cmp::{Ordering, min};
use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::error::player::{PlayerError, PlayerResult};
use crate::player::skill::{Skill, SkillTargetDomain, store::SkillStorage};
use crate::rc4::RC4;
use foldhash::HashMap as FastHashMap;

/// 名字本体最大长度
pub const NAME_MAX_LEN: usize = 256;
/// 队伍名最大长度
pub const TEAM_MAX_LEN: usize = 256;

/// 大于 2048 才行动
pub const MOVE_POINT_THRESHOLD: i32 = 2048;

/// 玩家句柄（运行期唯一 ID）。
/// 为兼容旧命名仍叫 `PlrId`，但语义已从“裸指针”切到“稳定 ID”。
pub type PlrId = usize;

#[derive(Clone, Debug, Default)]
pub struct ActionTargets {
    pub enemy_alive: Vec<PlrId>,
    pub ally_alive: Vec<PlrId>,
    pub ally_all: Vec<PlrId>,
    pub ally_dead: Vec<PlrId>,
    pub all_alive: Vec<PlrId>,
}

impl ActionTargets {
    pub fn from_enemy_alive(enemy_alive: &[PlrId]) -> Self {
        Self {
            enemy_alive: enemy_alive.to_vec(),
            all_alive: enemy_alive.to_vec(),
            ..Self::default()
        }
    }
}

pub type StateTag = TypeId;

#[inline]
pub fn state_tag<T: StateTrait + 'static>() -> StateTag { TypeId::of::<T>() }

pub trait StateTrait: std::fmt::Debug + Any {
    fn meta_type(&self) -> i32 { 0 }

    /// 状态在 action 决策阶段的执行顺序（数字越小越先执行）。
    fn action_mode_priority(&self) -> i32 { 1000 }
    /// action 决策钩子，可强制默认攻击（Some(bool) 表示 default_attack 的 smart 值）。
    fn on_action_mode(&self, _smart: bool, _force_default_attack_smart: &mut Option<bool>) {}

    /// 状态在 update_states 阶段的执行顺序（数字越小越先执行）。
    fn update_state_priority(&self) -> i32 { 1000 }
    /// 在 update_states 阶段对 PlayerStatus 做修正。
    fn apply_update_state(&self, _status: &mut PlayerStatus) {}

    /// 状态在 pre_step 阶段的执行顺序（数字越小越先执行）。
    fn pre_step_priority(&self) -> i32 { 1000 }
    /// pre_step 钩子。返回 true 表示该状态应在本次流程后被清理。
    fn on_pre_step(&mut self, _owner: PlrId, _status: &PlayerStatus, _step: &mut i32, _updates: &mut RunUpdates) -> bool { false }

    /// 状态在 pre_defend 阶段的执行顺序（数字越小越先执行）。
    fn pre_defend_priority(&self) -> i32 { 1000 }
    /// pre_defend 钩子。返回 true 表示该状态应在本次流程后被清理。
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

    /// 状态在 post_action 阶段的执行顺序（数字越小越先执行）。
    fn post_action_priority(&self) -> i32 { 1000 }
    /// post_action 钩子。返回 true 表示该状态应在本次流程后被清理。
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

    /// 状态在 post_defend 阶段的执行顺序（数字越小越先执行）。
    fn post_defend_priority(&self) -> i32 { 1000 }
    /// post_defend 钩子，可直接修正伤害值。
    fn on_post_defend(&mut self, _owner: PlrId, _dmg: &mut i32, _caster: PlrId, _randomer: &mut RC4, _updates: &mut RunUpdates) {}

    /// 状态在死亡文案选择阶段的执行顺序（数字越小越先执行）。
    fn die_message_priority(&self) -> i32 { 1000 }
    /// 覆盖死亡文案。
    fn die_message(&self) -> Option<&'static str> { None }

    /// 若该状态绑定了某个 owner，返回 owner id。
    fn linked_owner(&self) -> Option<PlrId> { None }
    /// owner 死亡时的回调，返回 true 表示应清理该实体。
    fn on_linked_owner_die(&mut self, _owner: PlrId, _self_id: PlrId, _updates: &mut RunUpdates) -> bool { false }

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn clone_box(&self) -> Box<dyn StateTrait>;
}

impl Clone for Box<dyn StateTrait> {
    fn clone(&self) -> Self { self.clone_box() }
}

/// 玩家状态容器（用于承载各种技能运行时状态）。
#[derive(Clone, Debug, Default)]
pub struct PlayerStateStore {
    states: FastHashMap<StateTag, Box<dyn StateTrait>>,
}

impl PlayerStateStore {
    #[inline]
    pub fn set<T: StateTrait + 'static>(&mut self, state: T) { self.states.insert(state_tag::<T>(), Box::new(state)); }

    #[inline]
    pub fn get<T: StateTrait + 'static>(&self) -> Option<&T> { self.states.get(&state_tag::<T>())?.as_any().downcast_ref::<T>() }

    #[inline]
    pub fn get_mut<T: StateTrait + 'static>(&mut self) -> Option<&mut T> {
        self.states.get_mut(&state_tag::<T>())?.as_any_mut().downcast_mut::<T>()
    }

    #[inline]
    pub fn has<T: StateTrait + 'static>(&self) -> bool { self.states.contains_key(&state_tag::<T>()) }

    #[inline]
    pub fn clear<T: StateTrait + 'static>(&mut self) { self.states.remove(&state_tag::<T>()); }

    #[inline]
    pub fn clear_tag(&mut self, tag: StateTag) { self.states.remove(&tag); }

    #[inline]
    pub fn meta_type(&self, tag: StateTag) -> Option<i32> { self.states.get(&tag).map(|state| state.meta_type()) }

    pub fn clear_negative_states(&mut self) {
        let mut to_remove = Vec::new();
        for (tag, state) in self.states.iter() {
            if state.meta_type() < 0 {
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

    pub fn resolve_action_mode(&self, smart: bool) -> Option<bool> {
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
    ) {
        let mut ordered = self
            .states
            .iter()
            .map(|(tag, state)| (*tag, state.post_defend_priority()))
            .collect::<Vec<(StateTag, i32)>>();
        ordered.sort_unstable_by_key(|(_, priority)| *priority);
        for (tag, _) in ordered {
            if let Some(state) = self.states.get_mut(&tag) {
                state.on_post_defend(owner, dmg, caster, randomer, updates);
            }
        }
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

/// OnDamage 函数
///
/// 为什么 dart 里函数类型这么写的啊(恼)
///
/// ```dart
/// typedef OnDamage(Plr caster, Plr target, int dmg, R r, RunUpdates updates);
/// ```
pub type OnDamageFunc = fn(PlrId, PlrId, i32, &mut RC4, &mut RunUpdates);

fn noop_on_damage(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates) {}

/// 通过玩家句柄从存储层取可变玩家引用。
#[inline]
pub fn player_id_as_mut_plr<'a>(ptr: PlrId, storage: &'a Arc<Storage>) -> &'a mut Player {
    storage.just_get_player_mut(ptr).expect("cannot get mutable player by player handle")
}

// /// Player 的自增 ID
// pub static PLAYER_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug)]
pub struct PlayerStatus {
    /// 是否被冻结
    frozen: bool,
    /// 是否存活
    alive: bool,
    /// 分数
    point: u32,
    /// 原文: spsum
    /// > 2048 时才行动
    ///
    /// 单调递增, > 2048 时 -= 2048
    /// 然后接着单增
    pub move_point: i32,
    /// 血量
    pub hp: i32,
    /// 最大血量
    pub max_hp: i32,
    /// 攻击力 (atk)
    pub attack: i32,
    /// 防御 (def)
    pub defense: i32,
    /// 速度 (spd)
    pub speed: i32,
    /// 敏捷 (agl)
    pub agility: i32,
    /// 魔法 (mag)
    pub magic: i32,
    /// 蓝条
    pub mp: i32,
    /// 抗性 (mdf)
    pub resistance: i32,
    /// 智力 (itl)
    pub wisdom: i32,
    /// 蓄力速度?
    pub at_boost: f64,
    /// attract ?
    pub attract: f64,
    /// 总属性和
    pub attr_sum: u32,
    /// 攻击和?
    pub atk_sum: i32,
    /// 总和?
    pub all_sum: u32,
}

impl PlayerStatus {
    #[inline]
    pub fn frozed(&self) -> bool { self.frozen }
    #[inline]
    pub fn alive(&self) -> bool { self.alive }
    #[deprecated(note = "请使用 move_point()")]
    #[inline]
    pub fn spsum(&self) -> i32 { self.move_point }
    #[inline]
    pub fn check_move(&self) -> bool { self.move_point > MOVE_POINT_THRESHOLD }

    pub fn set_frozen(&mut self, val: bool) { self.frozen = val }

    pub fn set_alive(&mut self, val: bool) { self.alive = val }

    pub fn set_point(&mut self, val: u32) { self.point = val }

    #[inline]
    #[deprecated(note = "self.resistance")]
    pub fn mdf(&self) -> i32 { self.resistance }

    #[inline]
    #[deprecated(note = "self.wisdom")]
    pub fn itl(&self) -> i32 { self.wisdom }
}

impl Default for PlayerStatus {
    fn default() -> Self {
        PlayerStatus {
            frozen: false,
            alive: true,
            point: 0,
            move_point: 0,
            hp: 0,
            max_hp: 0,
            attack: 0,
            defense: 0,
            speed: 0,
            agility: 0,
            magic: 0,
            mp: 0,
            resistance: 0,
            wisdom: 0,
            at_boost: 1.0,
            attract: 32768.0,
            attr_sum: 0,
            atk_sum: 0,
            all_sum: 0,
        }
    }
}

impl std::fmt::Display for PlayerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PlayerStatus{{{},{} 分数: {}, hp: {} 移动点数: {} sums:{},{},{} 攻|{} 防|{} 速|{} 敏|{} 魔|{} mp|{} 抗|{} 智|{} }}",
            // 冻结/正常
            // 存活/死亡
            if self.frozen { "冻结" } else { "正常" },
            if self.alive { "存活" } else { "死亡" },
            self.point,
            self.hp,
            self.move_point,
            self.attr_sum,
            self.atk_sum,
            self.all_sum,
            self.attack,
            self.defense,
            self.speed,
            self.agility,
            self.magic,
            self.mp,
            self.resistance,
            self.wisdom
        )
    }
}

/// boss 玩家的名字
pub const BOSS_NAMES: [&str; 11] = [
    "mario", "sonic", "mosquito", "yuri", "slime", "ikaruga", "conan", "aokiji", "lazy", "covid", "saitama",
];

/// ["田一人", 18, "云剑狄卡敢", 25, "云剑穸跄祇", 35]
pub const BOOST_NAMES: [&str; 3] = ["云剑狄卡敢", "云剑穸跄祇", "田一人"];

pub fn boost_value(name: &str) -> u32 {
    match name {
        "云剑狄卡敢" => 25,
        "云剑穸跄祇" => 35,
        "田一人" => 18,
        _ => 0,
    }
}

/// 种子玩家的前缀
pub const SEED_PREFIX: &str = "seed:";

/// 匹配字符的 Unicode 码点
///
/// 其实就是过滤一下不可见字符
///
/// NOTE: 原始函数的实现方式是在内部有一个 match, 然后外面手动排除了 `13(\r)` 和 `32(空格)`
pub fn filter_char(s: char) -> bool {
    matches!(s as u32 , 9..12 | 133 | 160 | 5760 | 8192..8202 | 8232..8233 | 8239 | 8287 | 12288 | 65279)
}

pub fn median<T>(x: T, y: T, z: T) -> T
where
    T: std::cmp::Ord + std::marker::Copy,
{
    if x < y {
        if y < z {
            y
        } else if x < z {
            z
        } else {
            x
        }
    } else if x < z {
        x
    } else if y < z {
        z
    } else {
        y
    }
}
#[derive(Default, PartialEq, Eq, Debug, Clone, Copy)]
pub enum PlayerType {
    #[default]
    Normal,
    /// 种子玩家
    ///
    /// # marker: `seed:`
    Seed,
    /// 被克隆的玩家
    ///
    /// 似乎有个三种?
    Clone,
    /// Boss 玩家
    /// 其实应该是一大堆
    Boss,
    /// 被特殊增强的玩家
    ///
    /// 有一堆玩家都被增强了
    Boost,
    /// 标准测号用靶子
    ///
    /// # marker: `\u0002`
    Test1,
    /// 没用到的测号用玩家
    ///
    /// # marker: `\u0003`
    Test2,
    /// 比标准测号再强一点的测号用靶子
    ///
    /// # marker: `!`
    TestEx,
}

#[derive(Clone, Debug)]
pub struct Player {
    /// 队伍
    team: Option<String>,
    /// 玩家名
    name: String,
    /// 武器
    weapon: Option<String>,
    /// 玩家类型
    player_type: PlayerType,
    /// skl id
    skil_id: Vec<u32>,
    /// skl prop
    skil_prop: Vec<u32>,
    /// 玩家的 sort int
    /// 用于在排序中比较两个玩家
    pub sort_int: i32,
    /// RC4
    pub rand: RC4,
    /// name base
    /// ```python
    /// len(list(i for i in range(256) if (i * 181 + 160) % 256 > 88 and (i * 181 + 160) % 256 < 217 )) == 128
    /// ```
    pub name_base: Vec<u8>,
    /// 没 upgrade 过的 name base
    raw_name_base: [u8; 128],
    /// 原始的属性数据
    attr: [u32; 8],
    /// 玩家状态
    ///
    /// 主要是我懒得加一大堆字段
    status: PlayerStatus,
    /// 运行时状态（meta）
    state: PlayerStateStore,
    /// 技能相关
    skills: SkillStorage,
    /// 名字长度系数
    name_factor: f64,
    // /// store
    // pub storage: Arc<Storage>,
    /// plr id
    id: u64,
}

mod impl_attr;
mod impl_ctor;
mod impl_runtime;

impl PartialOrd for Player {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.p_cmp(other)) }
}

impl PartialEq for Player {
    fn eq(&self, other: &Self) -> bool { self.p_cmp(other) == Ordering::Equal }
}

impl std::fmt::Display for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Player{{{}{}, status: {}}}",
            if let Some(team) = &self.team {
                format!("{}@{}", self.name, team)
            } else {
                self.name.to_string()
            },
            if let Some(weapon) = &self.weapon {
                format!("+{}", weapon)
            } else {
                "".to_string()
            },
            self.status
        )
    }
}

#[cfg(test)]
mod test;

//! # 实体仓库 (storage)
//!
//! 本模块提供 [`Storage`]，是引擎的 ECS 风格实体容器。
//!
//! ## 设计说明
//!
//! `Storage` 持有运行期所有实体的所有权，并以共享引用的方式（`Arc<Storage>`）
//! 传递给引擎各子系统和技能实现。
//!
//! ### 内容概览
//!
//! | 字段                     | 类型                                | 说明                                  |
//! |--------------------------|-------------------------------------|-----------------------------------------|
//! | `players`                | `Vec<Option<UnsafeCell<Player>>>` | 所有玩家实体（含已死+存活），按 PlrId 稠密下标存储 |
//! | `skills`                 | `FastHashMap<usize, Skill>`         | 所有技能实例，以内存地址为 key        |
//! | `groups`                 | `Vec<Vec<PlrId>>`                   | 队伍分组 roster                       |
//! | `player_group`           | `Vec<Option<usize>>`                | 玩家→所属分组索引的反向映射           |
//! | `alive_groups`           | `Vec<Vec<PlrId>>`                   | 存活分组，由 WorldState 同步维护       |
//! | `alive_group_count`      | `usize`                             | JS Engine.y.a.Q 兼容队伍计数           |
//! | `pending_spawns`         | `Vec<PendingSpawn>`                 | 待 tick 同步的新实体（召唤物等）       |
//! | `pending_remove_players` | `Vec<PlrId>`                        | 待 tick 同步的移除实体                 |
//! | `death_queue`            | `Vec<PlrId>`                        | 按发生顺序的死亡记录，对齐 Dart 死亡顺序 |
//! | `pending_revivals`       | `Vec<PlrId>`                        | 待 tick 同步回 WorldState 的复活队列   |
//! | `needs_sync`             | `bool`                              | 脏标记，有死亡/移除/复活/召唤时置 true |
//! | `player_id_counter`      | `u64`                               | 玩家 ID 自增计数器                     |
//! | `eval_rq`                | `f64`                               | 名字强度评估使用的 `$.rq()` 等价值     |
//! | `in_post_damage_player`  | `UnsafeCell<Option<PlrId>>`         | 正在执行 post_damage 回调的使魔 ID     |
//!
//! ### 不安全访问说明
//!
//! `Storage` 内部使用了多处 `unsafe` 的 `just_get_*_mut` 方法，
//! 主要是为了得到 `&mut Player` 而不破坏共享的 `&Arc<Storage>`。
//! 调用方需确保在单一 tick 内不会有两个代码路径同时可变地引用同一玩家。

use std::cell::UnsafeCell;
use std::sync::Arc;

use crate::player::skill::Skill;
use crate::player::{Player, PlrId};

use foldhash::{HashMap as FastHashMap, HashMapExt};

/// 技能的 ID（ECS 内部标识）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkillId(usize);

impl SkillId {
    pub fn new(id: usize) -> SkillId { SkillId(id) }

    /// 根据一个 Skill 实例创建 SkillId。
    /// 当前实现直接使用内存地址。
    pub fn new_from_skill(skill: &Skill) -> SkillId {
        let ptr = skill as *const Skill;
        SkillId(ptr as usize)
    }

    pub fn raw(self) -> usize { self.0 }
}

#[derive(Debug, Clone)]
pub struct PendingSpawn {
    pub owner: PlrId,
    pub player: Player,
}

/// 运行期数据仓库。
///
/// 使用 `foldhash::HashMap`，性能优先于标准库 `HashMap`。
#[derive(Debug)]
pub struct Storage {
    /// 存技能（`usize` 为 SkillId 的原始值）。
    skills: UnsafeCell<FastHashMap<usize, Skill>>,
    /// 存队伍分组。
    groups: UnsafeCell<Vec<Vec<PlrId>>>,
    /// 运行期每队存活视图，由 world 同步。
    alive_groups: UnsafeCell<Vec<Vec<PlrId>>>,
    /// JS Engine.y.a.Q 兼容的 alive group 计数。
    alive_group_count: UnsafeCell<usize>,
    /// 存玩家实体。
    players: UnsafeCell<Vec<Option<UnsafeCell<Player>>>>,
    /// 玩家 -> 所属分组索引的反向映射，由 `sync_groups` 维护。
    player_group: UnsafeCell<Vec<Option<usize>>>,
    /// 延迟到引擎 tick 同步的新增实体。
    pending_spawns: UnsafeCell<Vec<PendingSpawn>>,
    /// 延迟到引擎 tick 同步的移除实体。
    pending_remove_players: UnsafeCell<Vec<PlrId>>,
    /// 按死亡发生顺序记录的死亡队列（对齐 Dart 的即时死亡处理顺序）。
    death_queue: UnsafeCell<Vec<PlrId>>,
    /// 技能/触发器复活已有玩家后，延迟到 tick 同步回 WorldState 的复活队列。
    pending_revivals: UnsafeCell<Vec<PlrId>>,
    /// 脏标记：当有死亡/移除/复活/召唤入队时置 true，sync_runtime_entities 据此跳过无用同步。
    needs_sync: UnsafeCell<bool>,
    /// 玩家 ID 自增计数器。
    player_id_counter: UnsafeCell<u64>,
    /// 名字强度评估时使用的 `$.rq()` 等价值。
    eval_rq: f64,
    /// JS `aR` flag: 正在执行 post_damage 回调的使魔 ID。
    /// 当使魔的 SummonShareDamageSkill 把伤害分摊给 owner 导致 owner 死亡时，
    /// owner 的 on_die_impl 不应立即处理该使魔的死亡（应由使魔自身的 on_damaged 路径处理），
    /// 以确保死亡顺序为 [owner, summon] 而非 [summon, owner]。
    in_post_damage_player: UnsafeCell<Option<PlrId>>,
}

// Storage 通过 UnsafeCell 实现运行期内部可变性。
// 引擎保证单线程推进单场战斗，不会并发地可变访问同一个 Storage。
unsafe impl Send for Storage {}
unsafe impl Sync for Storage {}

#[allow(clippy::mut_from_ref)] // UnsafeCell 内部可变性：从 &self 返回 &mut T 是预期行为
impl Storage {
    /// 创建一个新的 Storage。
    pub fn new() -> Storage { Self::new_with_eval_rq(crate::player::eval_name::DEFAULT_EVAL_RQ) }

    /// 创建一个新的 Storage，并显式指定名字强度评估使用的 `$.rq()` 等价值。
    pub fn new_with_eval_rq(eval_rq: f64) -> Storage {
        Storage {
            skills: UnsafeCell::new(FastHashMap::new()),
            groups: UnsafeCell::new(Vec::new()),
            alive_groups: UnsafeCell::new(Vec::new()),
            alive_group_count: UnsafeCell::new(0),
            players: UnsafeCell::new(Vec::new()),
            player_group: UnsafeCell::new(Vec::new()),
            pending_spawns: UnsafeCell::new(Vec::new()),
            pending_remove_players: UnsafeCell::new(Vec::new()),
            death_queue: UnsafeCell::new(Vec::new()),
            pending_revivals: UnsafeCell::new(Vec::new()),
            needs_sync: UnsafeCell::new(false),
            player_id_counter: UnsafeCell::new(0),
            eval_rq,
            in_post_damage_player: UnsafeCell::new(None),
        }
    }

    pub fn new_arc() -> Arc<Self> { Arc::new(Self::new()) }

    pub fn new_arc_with_eval_rq(eval_rq: f64) -> Arc<Self> { Arc::new(Self::new_with_eval_rq(eval_rq)) }

    pub fn clear(&mut self) {
        self.skills_mut().clear();
        self.groups_mut().clear();
        self.alive_groups_mut().clear();
        *self.alive_group_count_mut() = 0;
        self.players_mut().clear();
        self.player_group_mut().clear();
        self.pending_spawns_mut().clear();
        self.pending_remove_players_mut().clear();
        self.death_queue_mut().clear();
        self.pending_revivals_mut().clear();
        *self.needs_sync_mut() = false;
        self.set_in_post_damage_player(None);
    }

    #[inline]
    fn skills_ref(&self) -> &FastHashMap<usize, Skill> { unsafe { &*self.skills.get() } }

    #[inline]
    fn skills_mut(&self) -> &mut FastHashMap<usize, Skill> { unsafe { &mut *self.skills.get() } }

    #[inline]
    fn groups_ref(&self) -> &Vec<Vec<PlrId>> { unsafe { &*self.groups.get() } }

    #[inline]
    fn groups_mut(&self) -> &mut Vec<Vec<PlrId>> { unsafe { &mut *self.groups.get() } }

    #[inline]
    fn alive_groups_ref(&self) -> &Vec<Vec<PlrId>> { unsafe { &*self.alive_groups.get() } }

    #[inline]
    fn alive_groups_mut(&self) -> &mut Vec<Vec<PlrId>> { unsafe { &mut *self.alive_groups.get() } }

    #[inline]
    fn alive_group_count_ref(&self) -> &usize { unsafe { &*self.alive_group_count.get() } }

    #[inline]
    fn alive_group_count_mut(&self) -> &mut usize { unsafe { &mut *self.alive_group_count.get() } }

    #[inline]
    fn players_ref(&self) -> &Vec<Option<UnsafeCell<Player>>> { unsafe { &*self.players.get() } }

    #[inline]
    fn players_mut(&self) -> &mut Vec<Option<UnsafeCell<Player>>> { unsafe { &mut *self.players.get() } }

    #[inline]
    fn player_group_ref(&self) -> &Vec<Option<usize>> { unsafe { &*self.player_group.get() } }

    #[inline]
    fn player_group_mut(&self) -> &mut Vec<Option<usize>> { unsafe { &mut *self.player_group.get() } }

    #[inline]
    fn pending_spawns_ref(&self) -> &Vec<PendingSpawn> { unsafe { &*self.pending_spawns.get() } }

    #[inline]
    fn pending_spawns_mut(&self) -> &mut Vec<PendingSpawn> { unsafe { &mut *self.pending_spawns.get() } }

    #[inline]
    fn pending_remove_players_ref(&self) -> &Vec<PlrId> { unsafe { &*self.pending_remove_players.get() } }

    #[inline]
    fn pending_remove_players_mut(&self) -> &mut Vec<PlrId> { unsafe { &mut *self.pending_remove_players.get() } }

    #[inline]
    fn death_queue_ref(&self) -> &Vec<PlrId> { unsafe { &*self.death_queue.get() } }

    #[inline]
    fn death_queue_mut(&self) -> &mut Vec<PlrId> { unsafe { &mut *self.death_queue.get() } }

    #[inline]
    fn pending_revivals_ref(&self) -> &Vec<PlrId> { unsafe { &*self.pending_revivals.get() } }

    #[inline]
    fn pending_revivals_mut(&self) -> &mut Vec<PlrId> { unsafe { &mut *self.pending_revivals.get() } }

    #[inline]
    fn in_post_damage_player_ref(&self) -> Option<PlrId> { unsafe { *self.in_post_damage_player.get() } }

    #[inline]
    fn needs_sync_ref(&self) -> &bool { unsafe { &*self.needs_sync.get() } }

    #[inline]
    fn needs_sync_mut(&self) -> &mut bool { unsafe { &mut *self.needs_sync.get() } }

    #[inline]
    fn player_id_counter_ref(&self) -> &u64 { unsafe { &*self.player_id_counter.get() } }

    #[inline]
    fn player_id_counter_mut(&self) -> &mut u64 { unsafe { &mut *self.player_id_counter.get() } }

    #[inline]
    fn set_in_post_damage_player(&self, value: Option<PlrId>) {
        unsafe {
            *self.in_post_damage_player.get() = value;
        }
    }

    /// 获取当前玩家 ID 计数器的值（不增加）。
    pub fn current_plr_id(&self) -> u64 { *self.player_id_counter_ref() }

    /// 获取当前 Storage 使用的名字强度评估 `rq`。
    #[inline]
    pub fn eval_rq(&self) -> f64 { self.eval_rq }

    /// 生成一个新的玩家 ID。
    pub fn new_plr_id(&self) -> u64 {
        let counter = self.player_id_counter_mut();
        let id = *counter;
        *counter += 1;
        id
    }

    /// 标记需要同步（有死亡/移除/复活/召唤入队时调用）。
    #[inline]
    fn mark_dirty(&self) { *self.needs_sync_mut() = true; }

    /// 检查是否需要同步。
    #[inline]
    pub fn needs_sync(&self) -> bool { *self.needs_sync_ref() }

    /// 清除同步标记（sync_runtime_entities 完成后调用）。
    #[inline]
    pub fn clear_sync_flag(&self) { *self.needs_sync_mut() = false; }

    /// 标记某个使魔正在执行 post_damage 回调（对应 JS PlrSummon.aR 标志）。
    /// 在 SummonShareDamageSkill::post_damage 中设置，防止 owner 死亡时立即处理该使魔的死亡。
    pub fn set_in_post_damage(&self, plr: PlrId) { self.set_in_post_damage_player(Some(plr)); }

    /// 清除 post_damage 标记。
    pub fn clear_in_post_damage(&self) { self.set_in_post_damage_player(None); }

    /// 检查某个玩家是否正在执行 post_damage 回调。
    #[inline]
    pub fn is_in_post_damage(&self, plr: PlrId) -> bool { self.in_post_damage_player_ref() == Some(plr) }

    pub fn insert_group(&mut self, id: usize, plrs: Vec<PlrId>) {
        let groups = self.groups_mut();
        if id >= groups.len() {
            groups.resize_with(id + 1, Vec::new);
        }
        groups[id] = plrs;
    }

    pub fn get_group(&self, id: usize) -> Option<&Vec<PlrId>> { self.groups_ref().get(id) }

    pub fn alive_group_at(&self, team_idx: usize) -> Option<&Vec<PlrId>> { self.alive_groups_ref().get(team_idx) }

    pub fn group_containing(&self, actor: PlrId) -> Option<&Vec<PlrId>> {
        self.player_group_ref()
            .get(actor)
            .and_then(|idx| *idx)
            .and_then(|idx| self.groups_ref().get(idx))
    }

    pub fn group_index_of(&self, actor: PlrId) -> Option<usize> { self.player_group_ref().get(actor).and_then(|idx| *idx) }

    pub fn alive_group_containing(&self, actor: PlrId) -> Option<&Vec<PlrId>> {
        self.alive_groups_ref().iter().find(|group| group.contains(&actor))
    }

    /// 返回 actor 当前所在 alive 队伍的人数；若 actor 不在任何 alive_group 中则返回 0。
    pub fn alive_group_len_containing(&self, actor: PlrId) -> usize {
        self.player_group_ref()
            .get(actor)
            .and_then(|idx| *idx)
            .and_then(|team_idx| self.alive_groups_ref().get(team_idx))
            .filter(|group| group.contains(&actor))
            .map_or(0, Vec::len)
    }

    /// 通过 roster 找到 actor 所在队伍的索引，再返回该队伍的 alive 列表。
    /// 即使 actor 已死亡也能找到正确的 alive 列表（因为 roster 不移除死亡成员）。
    pub fn alive_group_at_team_of(&self, actor: PlrId) -> Option<&Vec<PlrId>> {
        let team_idx = self.player_group_ref().get(actor).and_then(|idx| *idx)?;
        self.alive_groups_ref().get(team_idx)
    }

    pub fn alive_group_count(&self) -> usize { *self.alive_group_count_ref() }

    pub fn all_alive_ids(&self) -> Vec<PlrId> { self.alive_groups_ref().iter().flat_map(|group| group.iter().copied()).collect() }

    pub fn all_player_ids(&self) -> Vec<PlrId> {
        self.players_ref()
            .iter()
            .enumerate()
            .filter_map(|(id, player)| player.is_some().then_some(id))
            .collect()
    }

    pub fn iter_player_ids(&self) -> impl Iterator<Item = PlrId> + '_ {
        self.players_ref()
            .iter()
            .enumerate()
            .filter_map(|(id, player)| player.is_some().then_some(id))
    }

    pub fn iter_pending_spawns(&self) -> impl Iterator<Item = &PendingSpawn> + '_ { self.pending_spawns_ref().iter() }

    pub fn sync_groups(&self, groups: &[Vec<PlrId>]) {
        let storage_groups = self.groups_mut();
        storage_groups.clear();
        let player_group = self.player_group_mut();
        player_group.clear();
        for (idx, group) in groups.iter().enumerate() {
            for &id in group {
                if id >= player_group.len() {
                    player_group.resize(id + 1, None);
                }
                player_group[id] = Some(idx);
            }
            storage_groups.push(group.clone());
        }
    }

    pub fn sync_alive_groups(&self, groups: &[Vec<PlrId>]) {
        *self.alive_group_count_mut() = groups.iter().filter(|group| !group.is_empty()).count();
        *self.alive_groups_mut() = groups.to_vec();
    }

    /// 接收 owned 数据直接存入，避免再次 clone。
    pub fn sync_alive_groups_owned(&self, groups: Vec<Vec<PlrId>>) {
        *self.alive_group_count_mut() = groups.iter().filter(|group| !group.is_empty()).count();
        *self.alive_groups_mut() = groups;
    }

    /// 接收 WorldState 中按 JS `Engine.y.a.Q` 语义维护的计数。
    pub fn sync_alive_groups_owned_with_count(&self, groups: Vec<Vec<PlrId>>, alive_group_count: usize) {
        *self.alive_group_count_mut() = alive_group_count;
        *self.alive_groups_mut() = groups;
    }

    /// 获取技能。
    pub fn get_skill(&self, id: SkillId) -> Option<&Skill> { self.skills_ref().get(&id.0) }

    /// 获取玩家。
    pub fn get_player(&self, ptr: &PlrId) -> Option<&Player> {
        self.players_ref()
            .get(*ptr)
            .and_then(Option::as_ref)
            .map(|player| unsafe { &*player.get() })
    }

    pub fn get_player_or_pending(&self, ptr: &PlrId) -> Option<&Player> {
        self.get_player(ptr).or_else(|| {
            self.pending_spawns_ref()
                .iter()
                .find(|pending| pending.player.as_ptr() == *ptr)
                .map(|pending| &pending.player)
        })
    }

    /// 获取玩家（不做 Option 检查）。
    pub fn get_player_unchecked(&self, ptr: &PlrId) -> &Player { self.get_player(ptr).expect("cannot get player from storage") }

    /// 获取技能的可变引用（安全版本）。
    pub fn get_skill_mut(&mut self, id: SkillId) -> Option<&mut Skill> { self.skills_mut().get_mut(&id.0) }

    /// 强行从 `&self` 获取 `&mut Skill`。
    /// 这个方法依赖 `unsafe`，需要调用方保证不会违反别名规则。
    #[allow(clippy::mut_from_ref)]
    pub fn just_get_skill_mut(&self, id: SkillId) -> Option<&mut Skill> { self.skills_mut().get_mut(&id.0) }

    /// 强行从 `&self` 获取 `&mut Player`。
    /// 这个方法依赖 `unsafe`，需要调用方保证不会违反别名规则。
    #[allow(clippy::mut_from_ref)]
    pub fn just_get_player_mut(&self, ptr: PlrId) -> Option<&mut Player> {
        self.players_ref()
            .get(ptr)
            .and_then(Option::as_ref)
            .map(|player| unsafe { &mut *player.get() })
    }

    #[allow(clippy::mut_from_ref)]
    pub fn just_get_pending_spawn_player_mut(&self, ptr: PlrId) -> Option<&mut Player> {
        self.pending_spawns_mut()
            .iter_mut()
            .find(|pending| pending.player.as_ptr() == ptr)
            .map(|pending| &mut pending.player)
    }

    #[allow(clippy::mut_from_ref)]
    pub fn just_get_player_or_pending_mut(&self, ptr: PlrId) -> Option<&mut Player> {
        self.just_get_player_mut(ptr).or_else(|| self.just_get_pending_spawn_player_mut(ptr))
    }

    /// 插入技能，并返回技能 ID。
    pub fn insert_skill(&mut self, skill: Skill) -> SkillId {
        let id = SkillId::new_from_skill(&skill);
        self.skills_mut().insert(id.0, skill);
        id
    }

    pub fn just_insert_skill(&self, skill: Skill) -> SkillId {
        let id = SkillId::new_from_skill(&skill);
        self.skills_mut().insert(id.0, skill);
        id
    }

    pub fn insert_player(&mut self, player: Player) -> PlrId {
        let id: PlrId = player.id().try_into().expect("player id overflow usize");
        Self::insert_player_slot(self.players_mut(), id, player);
        id
    }

    pub fn just_insert_player(&self, player: Player) -> PlrId {
        let id: PlrId = player.id().try_into().expect("player id overflow usize");
        Self::insert_player_slot(self.players_mut(), id, player);
        id
    }

    fn insert_player_slot(players: &mut Vec<Option<UnsafeCell<Player>>>, id: PlrId, player: Player) {
        if id >= players.len() {
            players.resize_with(id + 1, || None);
        }
        players[id] = Some(UnsafeCell::new(player));
    }

    pub fn queue_spawn(&self, owner: PlrId, player: Player) {
        self.pending_spawns_mut().push(PendingSpawn { owner, player });
        self.mark_dirty();
    }

    pub fn take_pending_spawns(&self) -> Vec<PendingSpawn> { std::mem::take(self.pending_spawns_mut()) }

    pub fn pending_spawn_count_for_owner(&self, owner: PlrId) -> usize {
        self.pending_spawns_ref().iter().filter(|pending| pending.owner == owner).count()
    }

    pub fn iter_pending_spawn_ids_for_owner(&self, owner: PlrId) -> impl Iterator<Item = PlrId> + '_ {
        self.pending_spawns_ref()
            .iter()
            .filter(move |pending| pending.owner == owner)
            .map(|pending| pending.player.as_ptr())
    }

    pub fn pending_spawn_ids_for_owner(&self, owner: PlrId) -> Vec<PlrId> {
        self.iter_pending_spawn_ids_for_owner(owner).collect()
    }

    /// 返回所有 owner 在指定队员集合内的 pending spawn 的 PlrId。
    pub fn pending_spawn_ids_for_group(&self, group_members: &[PlrId]) -> Vec<PlrId> {
        self.pending_spawns_ref()
            .iter()
            .filter(|pending| group_members.contains(&pending.owner))
            .map(|pending| pending.player.as_ptr())
            .collect()
    }

    /// 返回指定 roster 中已 queue_revival、但尚未 sync 回 alive_group 的成员。
    pub fn iter_pending_revival_ids_for_group<'a>(&'a self, group_members: &'a [PlrId]) -> impl Iterator<Item = PlrId> + 'a {
        self.pending_revivals_ref().iter().copied().filter(move |id| group_members.contains(id))
    }

    /// 返回指定 roster 中已 queue_revival、但尚未 sync 回 alive_group 的成员。
    pub fn pending_revival_ids_for_group(&self, group_members: &[PlrId]) -> Vec<PlrId> {
        self.iter_pending_revival_ids_for_group(group_members).collect()
    }

    pub fn get_pending_spawn_player(&self, ptr: PlrId) -> Option<&Player> {
        self.pending_spawns_ref()
            .iter()
            .find(|pending| pending.player.as_ptr() == ptr)
            .map(|pending| &pending.player)
    }

    pub fn queue_remove_player(&self, ptr: PlrId) {
        if !self.pending_remove_players_ref().contains(&ptr) {
            self.pending_remove_players_mut().push(ptr);
        }
        self.mark_dirty();
    }

    pub fn take_pending_remove_players(&self) -> Vec<PlrId> { std::mem::take(self.pending_remove_players_mut()) }

    /// 记录一次死亡（按发生顺序），对齐 Dart 的即时死亡处理顺序。
    pub fn record_death(&self, ptr: PlrId) {
        if !self.death_queue_ref().contains(&ptr) {
            self.death_queue_mut().push(ptr);
        }
        self.mark_dirty();
    }

    /// 取出并清空死亡队列。
    pub fn take_death_queue(&self) -> Vec<PlrId> { std::mem::take(self.death_queue_mut()) }

    /// 技能/触发器复活已有玩家时，注册到复活队列，待 tick 同步回 WorldState。
    pub fn queue_revival(&self, ptr: PlrId) {
        if !self.pending_revivals_ref().contains(&ptr) {
            self.pending_revivals_mut().push(ptr);
        }
        self.mark_dirty();
    }

    /// 取出并清空复活队列。
    pub fn take_pending_revivals(&self) -> Vec<PlrId> { std::mem::take(self.pending_revivals_mut()) }

    /// 当前待处理的召唤数量（用于快速路径判断）。
    pub fn pending_spawn_count(&self) -> usize { self.pending_spawns_ref().len() }

    /// 删除技能（安全版本）。
    pub fn remove_skill(&mut self, id: SkillId) -> Option<Skill> { self.skills_mut().remove(&id.0) }

    pub fn just_remove_skill(&self, id: SkillId) -> Option<Skill> { self.skills_mut().remove(&id.0) }

    pub fn just_remove_player(&self, ptr: PlrId) -> Option<Player> {
        self.players_mut().get_mut(ptr).and_then(Option::take).map(UnsafeCell::into_inner)
    }
}

impl std::default::Default for Storage {
    fn default() -> Self { Self::new() }
}

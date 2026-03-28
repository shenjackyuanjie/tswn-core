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
//! | 字段                   | 类型                    | 说明                              |
//! |------------------------|------------------------|-------------------------------------|
//! | `players`              | `HashMap<PlrId, Player>` | 所有玩家实体（含已死/待展开+存活）   |
//! | `skills`               | `HashMap<usize, Skill>`  | 所有技能实例，以内存地址为 key      |
//! | `groups`               | `HashMap<usize, Vec<PlrId>>` | 队伍分组 roster            |
//! | `alive_groups`         | `Vec<Vec<PlrId>>`      | 存活分组，由 WorldState 同步维护 |
//! | `pending_spawns`       | `Vec<PendingSpawn>`    | 待 tick 同步的新在1实体（召唤物等）   |
//! | `pending_remove_players` | `Vec<PlrId>`         | 待 tick 同步的当回转移除              |
//! | `death_queue`          | `Vec<PlrId>`           | 实际斶序的死亡记录，对齐 Dart 死亡顺序 |
//!
//! ### 不安全访问说明
//!
//! `Storage` 内部使用了多处 `unsafe` 的 `just_get_*_mut` 方法，
//! 主要是为了得到 `&mut Player` 而不破坏共享的 `&Arc<Storage>`。
//! 调用方需确保在单一 tick 内不会有两个代码路径同时可变地引用同一玩家。

use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};

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
    groups: UnsafeCell<FastHashMap<usize, Vec<PlrId>>>,
    /// 运行期每队存活视图，由 world 同步。
    alive_groups: UnsafeCell<Vec<Vec<PlrId>>>,
    /// 存玩家实体。
    players: UnsafeCell<FastHashMap<PlrId, UnsafeCell<Player>>>,
    /// 延迟到引擎 tick 同步的新增实体。
    pending_spawns: UnsafeCell<Vec<PendingSpawn>>,
    /// 延迟到引擎 tick 同步的移除实体。
    pending_remove_players: UnsafeCell<Vec<PlrId>>,
    /// 按死亡发生顺序记录的死亡队列（对齐 Dart 的即时死亡处理顺序）。
    death_queue: UnsafeCell<Vec<PlrId>>,
    /// 技能/触发器复活已有玩家后，延迟到 tick 同步回 WorldState 的复活队列。
    pending_revivals: UnsafeCell<Vec<PlrId>>,
    /// 脏标记：当有死亡/移除/复活/召唤入队时置 true，sync_runtime_entities 据此跳过无用同步。
    needs_sync: AtomicBool,
    /// 玩家 ID 自增计数器。
    player_id_counter: AtomicU64,
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
            groups: UnsafeCell::new(FastHashMap::new()),
            alive_groups: UnsafeCell::new(Vec::new()),
            players: UnsafeCell::new(FastHashMap::new()),
            pending_spawns: UnsafeCell::new(Vec::new()),
            pending_remove_players: UnsafeCell::new(Vec::new()),
            death_queue: UnsafeCell::new(Vec::new()),
            pending_revivals: UnsafeCell::new(Vec::new()),
            needs_sync: AtomicBool::new(false),
            player_id_counter: AtomicU64::new(0),
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
        self.players_mut().clear();
        self.pending_spawns_mut().clear();
        self.pending_remove_players_mut().clear();
        self.death_queue_mut().clear();
        self.pending_revivals_mut().clear();
        self.needs_sync.store(false, std::sync::atomic::Ordering::Relaxed);
        self.set_in_post_damage_player(None);
    }

    #[inline]
    fn skills_ref(&self) -> &FastHashMap<usize, Skill> { unsafe { &*self.skills.get() } }

    #[inline]
    fn skills_mut(&self) -> &mut FastHashMap<usize, Skill> { unsafe { &mut *self.skills.get() } }

    #[inline]
    fn groups_ref(&self) -> &FastHashMap<usize, Vec<PlrId>> { unsafe { &*self.groups.get() } }

    #[inline]
    fn groups_mut(&self) -> &mut FastHashMap<usize, Vec<PlrId>> { unsafe { &mut *self.groups.get() } }

    #[inline]
    fn alive_groups_ref(&self) -> &Vec<Vec<PlrId>> { unsafe { &*self.alive_groups.get() } }

    #[inline]
    fn alive_groups_mut(&self) -> &mut Vec<Vec<PlrId>> { unsafe { &mut *self.alive_groups.get() } }

    #[inline]
    fn players_ref(&self) -> &FastHashMap<PlrId, UnsafeCell<Player>> { unsafe { &*self.players.get() } }

    #[inline]
    fn players_mut(&self) -> &mut FastHashMap<PlrId, UnsafeCell<Player>> { unsafe { &mut *self.players.get() } }

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
    fn set_in_post_damage_player(&self, value: Option<PlrId>) {
        unsafe {
            *self.in_post_damage_player.get() = value;
        }
    }

    /// 获取当前玩家 ID 计数器的值（不增加）。
    pub fn current_plr_id(&self) -> u64 { self.player_id_counter.load(std::sync::atomic::Ordering::Relaxed) }

    /// 获取当前 Storage 使用的名字强度评估 `rq`。
    #[inline]
    pub fn eval_rq(&self) -> f64 { self.eval_rq }

    /// 生成一个新的玩家 ID。
    pub fn new_plr_id(&self) -> u64 { self.player_id_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) }

    /// 标记需要同步（有死亡/移除/复活/召唤入队时调用）。
    #[inline]
    fn mark_dirty(&self) { self.needs_sync.store(true, std::sync::atomic::Ordering::Relaxed); }

    /// 检查是否需要同步。
    #[inline]
    pub fn needs_sync(&self) -> bool { self.needs_sync.load(std::sync::atomic::Ordering::Relaxed) }

    /// 清除同步标记（sync_runtime_entities 完成后调用）。
    #[inline]
    pub fn clear_sync_flag(&self) { self.needs_sync.store(false, std::sync::atomic::Ordering::Relaxed); }

    /// 标记某个使魔正在执行 post_damage 回调（对应 JS PlrSummon.aR 标志）。
    /// 在 SummonShareDamageSkill::post_damage 中设置，防止 owner 死亡时立即处理该使魔的死亡。
    pub fn set_in_post_damage(&self, plr: PlrId) { self.set_in_post_damage_player(Some(plr)); }

    /// 清除 post_damage 标记。
    pub fn clear_in_post_damage(&self) { self.set_in_post_damage_player(None); }

    /// 检查某个玩家是否正在执行 post_damage 回调。
    #[inline]
    pub fn is_in_post_damage(&self, plr: PlrId) -> bool { self.in_post_damage_player_ref() == Some(plr) }

    pub fn insert_group(&mut self, id: usize, plrs: Vec<PlrId>) { self.groups_mut().insert(id, plrs); }

    pub fn get_group(&self, id: usize) -> Option<&Vec<PlrId>> { self.groups_ref().get(&id) }

    pub fn alive_group_at(&self, team_idx: usize) -> Option<&Vec<PlrId>> { self.alive_groups_ref().get(team_idx) }

    pub fn group_containing(&self, actor: PlrId) -> Option<&Vec<PlrId>> {
        self.groups_ref().values().find(|group| group.contains(&actor))
    }

    pub fn group_index_of(&self, actor: PlrId) -> Option<usize> {
        self.groups_ref().iter().find(|(_, group)| group.contains(&actor)).map(|(idx, _)| *idx)
    }

    pub fn alive_group_containing(&self, actor: PlrId) -> Option<&Vec<PlrId>> {
        self.alive_groups_ref().iter().find(|group| group.contains(&actor))
    }

    /// 通过 roster 找到 actor 所在队伍的索引，再返回该队伍的 alive 列表。
    /// 即使 actor 已死亡也能找到正确的 alive 列表（因为 roster 不移除死亡成员）。
    pub fn alive_group_at_team_of(&self, actor: PlrId) -> Option<&Vec<PlrId>> {
        let team_idx = self.groups_ref().iter().find(|(_, group)| group.contains(&actor)).map(|(idx, _)| *idx)?;
        self.alive_groups_ref().get(team_idx)
    }

    pub fn alive_group_count(&self) -> usize { self.alive_groups_ref().iter().filter(|group| !group.is_empty()).count() }

    pub fn all_alive_ids(&self) -> Vec<PlrId> { self.alive_groups_ref().iter().flat_map(|group| group.iter().copied()).collect() }

    pub fn all_player_ids(&self) -> Vec<PlrId> { self.players_ref().keys().copied().collect() }

    pub fn sync_groups(&self, groups: &[Vec<PlrId>]) {
        let storage_groups = self.groups_mut();
        storage_groups.clear();
        for (idx, group) in groups.iter().enumerate() {
            storage_groups.insert(idx, group.clone());
        }
    }

    pub fn sync_alive_groups(&self, groups: &[Vec<PlrId>]) { *self.alive_groups_mut() = groups.to_vec(); }

    /// 接收 owned 数据直接存入，避免再次 clone。
    pub fn sync_alive_groups_owned(&self, groups: Vec<Vec<PlrId>>) { *self.alive_groups_mut() = groups; }

    /// 获取技能。
    pub fn get_skill(&self, id: SkillId) -> Option<&Skill> { self.skills_ref().get(&id.0) }

    /// 获取玩家。
    pub fn get_player(&self, ptr: &PlrId) -> Option<&Player> {
        self.players_ref().get(ptr).map(|player| unsafe { &*player.get() })
    }

    pub fn get_player_or_pending(&self, ptr: &PlrId) -> Option<&Player> {
        self.players_ref().get(ptr).map(|player| unsafe { &*player.get() }).or_else(|| {
            self.pending_spawns_ref()
                .iter()
                .find(|pending| pending.player.as_ptr() == *ptr)
                .map(|pending| &pending.player)
        })
    }

    /// 获取玩家（不做 Option 检查）。
    pub fn get_player_unchecked(&self, ptr: &PlrId) -> &Player {
        self.players_ref()
            .get(ptr)
            .map(|player| unsafe { &*player.get() })
            .expect("cannot get player from storage")
    }

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
        self.players_ref().get(&ptr).map(|player| unsafe { &mut *player.get() })
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
        self.players_mut().insert(id, UnsafeCell::new(player));
        id
    }

    pub fn just_insert_player(&self, player: Player) -> PlrId {
        let id: PlrId = player.id().try_into().expect("player id overflow usize");
        self.players_mut().insert(id, UnsafeCell::new(player));
        id
    }

    pub fn queue_spawn(&self, owner: PlrId, player: Player) {
        self.pending_spawns_mut().push(PendingSpawn { owner, player });
        self.mark_dirty();
    }

    pub fn take_pending_spawns(&self) -> Vec<PendingSpawn> { std::mem::take(self.pending_spawns_mut()) }

    pub fn pending_spawn_count_for_owner(&self, owner: PlrId) -> usize {
        self.pending_spawns_ref().iter().filter(|pending| pending.owner == owner).count()
    }

    pub fn pending_spawn_ids_for_owner(&self, owner: PlrId) -> Vec<PlrId> {
        self.pending_spawns_ref()
            .iter()
            .filter(|pending| pending.owner == owner)
            .map(|pending| pending.player.as_ptr())
            .collect()
    }

    /// 返回所有 owner 在指定队员集合内的 pending spawn 的 PlrId。
    pub fn pending_spawn_ids_for_group(&self, group_members: &[PlrId]) -> Vec<PlrId> {
        self.pending_spawns_ref()
            .iter()
            .filter(|pending| group_members.contains(&pending.owner))
            .map(|pending| pending.player.as_ptr())
            .collect()
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

    pub fn just_remove_player(&self, ptr: PlrId) -> Option<Player> { self.players_mut().remove(&ptr).map(UnsafeCell::into_inner) }
}

impl std::default::Default for Storage {
    fn default() -> Self { Self::new() }
}

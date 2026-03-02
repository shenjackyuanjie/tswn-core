use std::sync::Arc;
use std::sync::atomic::AtomicU64;

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
    skills: FastHashMap<usize, Skill>,
    /// 存队伍分组。
    groups: FastHashMap<usize, Vec<PlrId>>,
    /// 存玩家实体。
    players: FastHashMap<PlrId, Player>,
    /// 延迟到引擎 tick 同步的新增实体。
    pending_spawns: Vec<PendingSpawn>,
    /// 延迟到引擎 tick 同步的移除实体。
    pending_remove_players: Vec<PlrId>,
    /// 按死亡发生顺序记录的死亡队列（对齐 Dart 的即时死亡处理顺序）。
    death_queue: Vec<PlrId>,
    /// 玩家 ID 自增计数器。
    player_id_counter: AtomicU64,
}

impl Storage {
    /// 创建一个新的 Storage。
    pub fn new() -> Storage {
        Storage {
            skills: FastHashMap::new(),
            groups: FastHashMap::new(),
            players: FastHashMap::new(),
            pending_spawns: Vec::new(),
            pending_remove_players: Vec::new(),
            death_queue: Vec::new(),
            player_id_counter: AtomicU64::new(0),
        }
    }

    pub fn new_arc() -> Arc<Self> { Arc::new(Self::new()) }

    pub fn clear(&mut self) {
        self.skills.clear();
        self.groups.clear();
        self.players.clear();
        self.pending_spawns.clear();
        self.pending_remove_players.clear();
        self.death_queue.clear();
    }

    /// 生成一个新的玩家 ID。
    pub fn new_plr_id(&self) -> u64 { self.player_id_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) }

    pub fn insert_group(&mut self, id: usize, plrs: Vec<PlrId>) { self.groups.insert(id, plrs); }

    pub fn get_group(&self, id: usize) -> Option<&Vec<PlrId>> { self.groups.get(&id) }

    pub fn group_containing(&self, actor: PlrId) -> Option<&Vec<PlrId>> {
        self.groups.values().find(|group| group.contains(&actor))
    }

    pub fn all_player_ids(&self) -> Vec<PlrId> { self.players.keys().copied().collect() }

    pub fn sync_groups(&self, groups: &[Vec<PlrId>]) {
        unsafe {
            let mut_slf = self as *const Storage as *mut Storage;
            (*mut_slf).groups.clear();
            for (idx, group) in groups.iter().enumerate() {
                (*mut_slf).groups.insert(idx, group.clone());
            }
        }
    }

    /// 获取技能。
    pub fn get_skill(&self, id: SkillId) -> Option<&Skill> { self.skills.get(&id.0) }

    /// 获取玩家。
    pub fn get_player(&self, ptr: &PlrId) -> Option<&Player> { self.players.get(ptr) }

    pub fn get_player_or_pending(&self, ptr: &PlrId) -> Option<&Player> {
        self.players.get(ptr).or_else(|| {
            self.pending_spawns
                .iter()
                .find(|pending| pending.player.as_ptr() == *ptr)
                .map(|pending| &pending.player)
        })
    }

    /// 获取玩家（不做 Option 检查）。
    pub fn get_player_unchecked(&self, ptr: &PlrId) -> &Player { self.players.get(ptr).expect("cannot get player from storage") }

    /// 获取技能的可变引用（安全版本）。
    pub fn get_skill_mut(&mut self, id: SkillId) -> Option<&mut Skill> { self.skills.get_mut(&id.0) }

    /// 强行从 `&self` 获取 `&mut Skill`。
    /// 这个方法依赖 `unsafe`，需要调用方保证不会违反别名规则。
    #[allow(clippy::mut_from_ref)]
    pub fn just_get_skill_mut(&self, id: SkillId) -> Option<&mut Skill> {
        unsafe {
            let mut_slf = self as *const Storage as *mut Storage;
            (*mut_slf).skills.get_mut(&id.0)
        }
    }

    /// 强行从 `&self` 获取 `&mut Player`。
    /// 这个方法依赖 `unsafe`，需要调用方保证不会违反别名规则。
    #[allow(clippy::mut_from_ref)]
    pub fn just_get_player_mut(&self, ptr: PlrId) -> Option<&mut Player> {
        unsafe {
            let mut_slf = self as *const Storage as *mut Storage;
            (*mut_slf).players.get_mut(&ptr)
        }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn just_get_pending_spawn_player_mut(&self, ptr: PlrId) -> Option<&mut Player> {
        unsafe {
            let mut_slf = self as *const Storage as *mut Storage;
            (*mut_slf)
                .pending_spawns
                .iter_mut()
                .find(|pending| pending.player.as_ptr() == ptr)
                .map(|pending| &mut pending.player)
        }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn just_get_player_or_pending_mut(&self, ptr: PlrId) -> Option<&mut Player> {
        self.just_get_player_mut(ptr).or_else(|| self.just_get_pending_spawn_player_mut(ptr))
    }

    /// 插入技能，并返回技能 ID。
    pub fn insert_skill(&mut self, skill: Skill) -> SkillId {
        let id = SkillId::new_from_skill(&skill);
        self.skills.insert(id.0, skill);
        id
    }

    pub fn just_insert_skill(&self, skill: Skill) -> SkillId {
        unsafe {
            let mut_slf = self as *const Storage as *mut Storage;
            let id = SkillId::new_from_skill(&skill);
            (*mut_slf).skills.insert(id.0, skill);
            id
        }
    }

    pub fn insert_player(&mut self, player: Player) -> PlrId {
        let id: PlrId = player.id().try_into().expect("player id overflow usize");
        self.players.insert(id, player);
        id
    }

    pub fn just_insert_player(&self, player: Player) -> PlrId {
        unsafe {
            let mut_slf = self as *const Storage as *mut Storage;
            let id: PlrId = player.id().try_into().expect("player id overflow usize");
            (*mut_slf).players.insert(id, player);
            id
        }
    }

    pub fn queue_spawn(&self, owner: PlrId, player: Player) {
        unsafe {
            let mut_slf = self as *const Storage as *mut Storage;
            (*mut_slf).pending_spawns.push(PendingSpawn { owner, player });
        }
    }

    pub fn take_pending_spawns(&self) -> Vec<PendingSpawn> {
        unsafe {
            let mut_slf = self as *const Storage as *mut Storage;
            std::mem::take(&mut (*mut_slf).pending_spawns)
        }
    }

    pub fn pending_spawn_count_for_owner(&self, owner: PlrId) -> usize {
        self.pending_spawns.iter().filter(|pending| pending.owner == owner).count()
    }

    pub fn pending_spawn_ids_for_owner(&self, owner: PlrId) -> Vec<PlrId> {
        self.pending_spawns
            .iter()
            .filter(|pending| pending.owner == owner)
            .map(|pending| pending.player.as_ptr())
            .collect()
    }

    pub fn get_pending_spawn_player(&self, ptr: PlrId) -> Option<&Player> {
        self.pending_spawns
            .iter()
            .find(|pending| pending.player.as_ptr() == ptr)
            .map(|pending| &pending.player)
    }

    pub fn queue_remove_player(&self, ptr: PlrId) {
        unsafe {
            let mut_slf = self as *const Storage as *mut Storage;
            if !(*mut_slf).pending_remove_players.contains(&ptr) {
                (*mut_slf).pending_remove_players.push(ptr);
            }
        }
    }

    pub fn take_pending_remove_players(&self) -> Vec<PlrId> {
        unsafe {
            let mut_slf = self as *const Storage as *mut Storage;
            std::mem::take(&mut (*mut_slf).pending_remove_players)
        }
    }

    /// 记录一次死亡（按发生顺序），对齐 Dart 的即时死亡处理顺序。
    pub fn record_death(&self, ptr: PlrId) {
        unsafe {
            let mut_slf = self as *const Storage as *mut Storage;
            if !(*mut_slf).death_queue.contains(&ptr) {
                (*mut_slf).death_queue.push(ptr);
            }
        }
    }

    /// 取出并清空死亡队列。
    pub fn take_death_queue(&self) -> Vec<PlrId> {
        unsafe {
            let mut_slf = self as *const Storage as *mut Storage;
            std::mem::take(&mut (*mut_slf).death_queue)
        }
    }

    /// 删除技能（安全版本）。
    pub fn remove_skill(&mut self, id: SkillId) -> Option<Skill> { self.skills.remove(&id.0) }

    pub fn just_remove_skill(&self, id: SkillId) -> Option<Skill> {
        unsafe {
            let mut_slf = self as *const Storage as *mut Storage;
            (*mut_slf).skills.remove(&id.0)
        }
    }

    pub fn just_remove_player(&self, ptr: PlrId) -> Option<Player> {
        unsafe {
            let mut_slf = self as *const Storage as *mut Storage;
            (*mut_slf).players.remove(&ptr)
        }
    }
}

impl std::default::Default for Storage {
    fn default() -> Self { Self::new() }
}

//! # 世界状态 (world_state)
//!
//! 本模块定义 [`WorldState`] 和 [`TeamState`]，作为战斗世界的轻量级快照。
//!
//! ## 设计说明
//!
//! `WorldState` **不直接持有玩家实体**，只保存 [`PlrId`]（玩家句柄/ID）列表。
//! 玩家实体存储在 [`Storage`](crate::engine::storage::Storage) 中，
//! 通过 `PlrId` 与世界状态相互配合使用。
//!
//! ### 数据结构
//!
//! ```text
//! WorldState
//!   ├── teams: Vec<TeamState>     ← 每支队伍的 roster（全员）和 alive（存活）
//!   ├── groups: Vec<Vec<PlrId>>   ← 与 teams.roster 保持同步（供 Storage 查询）
//!   ├── players: Vec<PlrId>       ← 当前回合行动顺序列表（存活成员）
//!   ├── round_pos: i32            ← 当前轮次指针（循环推进）
//!   └── winner: Option<Vec<PlrId>>← 胜者 roster，Some 表示战斗已结束
//! ```
//!
//! ### 关键操作
//!
//! - [`remove_player`](WorldState::remove_player) — 死亡时同时从 `alive` 和 `players` 轮次表中移除
//! - [`revive_player`](WorldState::revive_player) — 复活时重新加入 `alive` 列表
//! - [`add_new_player`](WorldState::add_new_player) — 召唤物出现时动态加入队伍

use crate::player::PlrId;
use foldhash::{HashMap as FoldHashMap, HashSet as FoldHashSet};

/// 单支队伍的状态快照。
#[derive(Debug, Clone)]
pub struct TeamState {
    /// 队伍成员列表（不区分存活与否）
    pub roster: Vec<PlrId>,
    /// 存活成员列表（roster 的子集）
    pub alive: Vec<PlrId>,
}

#[derive(Debug, Clone)]
pub struct WorldState {
    /// 所有队伍的状态信息，包含每支队伍的 roster（全员）和 alive（存活成员）
    pub teams: Vec<TeamState>,
    /// 队伍分组信息，与 teams.roster 保持同步，供 Storage 查询使用
    pub groups: Vec<Vec<PlrId>>,
    /// 胜者队伍，Some 表示战斗已结束，包含胜者队伍的 roster
    pub winner: Option<Vec<PlrId>>,
    /// 当前回合行动顺序列表（仅包含存活成员）
    pub players: Vec<PlrId>,
    /// 当前轮次指针，用于循环推进行动顺序
    pub round_pos: i32,
    /// 存活玩家集合，用于 O(1) contains_alive 查询
    alive_set: FoldHashSet<PlrId>,
    /// 玩家 → 所属队伍索引映射，用于 O(1) team_index_of 查询
    player_team: FoldHashMap<PlrId, usize>,
    /// 已在行动轮次中的玩家集合，用于 O(1) ensure_player_in_round 去重检查
    players_set: FoldHashSet<PlrId>,
    /// JS Engine.e 兼容的全局存活列表。
    ///
    /// 与 JS 的 `Engine.e`（`all_alive`）保持相同的插入/删除语义：
    /// - 初始化时按队伍顺序拼接
    /// - 新实体插入到最后一个存活队友之后
    /// - 复活时如果队伍无存活成员，追加到末尾
    /// - 死亡时 splice 移除，保持剩余元素相对顺序
    ///
    /// `select_targets` 应使用此列表（而非从 teams 重建）来构建 `all_alive`，
    /// 以确保 `pickSkipRange` 的索引映射与 JS 一致。
    pub flat_alive: Vec<PlrId>,
}

impl WorldState {
    pub fn new(groups: Vec<Vec<PlrId>>) -> Self {
        let teams = groups
            .iter()
            .map(|group| TeamState {
                roster: group.clone(),
                alive: group.clone(),
            })
            .collect::<Vec<TeamState>>();
        let players: Vec<PlrId> = teams.iter().flat_map(|team| team.roster.iter().copied()).collect();
        let alive_set: FoldHashSet<PlrId> = teams.iter().flat_map(|t| t.alive.iter().copied()).collect();
        let player_team: FoldHashMap<PlrId, usize> = teams
            .iter()
            .enumerate()
            .flat_map(|(idx, team)| team.roster.iter().map(move |id| (*id, idx)))
            .collect();
        let players_set: FoldHashSet<PlrId> = players.iter().copied().collect();
        // 初始 flat_alive 按队伍顺序拼接，与 JS Engine 构造函数中 _.e 的初始顺序一致
        let flat_alive: Vec<PlrId> = teams.iter().flat_map(|team| team.alive.iter().copied()).collect();
        Self {
            teams,
            groups,
            winner: None,
            players,
            round_pos: -1,
            alive_set,
            player_team,
            players_set,
            flat_alive,
        }
    }

    #[inline]
    pub fn have_winner(&self) -> bool { self.winner.is_some() }

    #[inline]
    pub fn all_plrs(&self) -> Vec<PlrId> { self.teams.iter().flat_map(|team| team.roster.iter().copied()).collect() }

    #[inline]
    pub fn all_plr_len(&self) -> usize { self.teams.iter().map(|team| team.roster.len()).sum() }

    #[inline]
    pub fn team_index_of(&self, actor: PlrId) -> Option<usize> { self.player_team.get(&actor).copied() }

    #[inline]
    pub fn team_roster(&self, team_idx: usize) -> Option<&[PlrId]> { self.teams.get(team_idx).map(|team| team.roster.as_slice()) }

    #[inline]
    pub fn team_alive(&self, team_idx: usize) -> Option<&[PlrId]> { self.teams.get(team_idx).map(|team| team.alive.as_slice()) }

    #[inline]
    pub fn contains_alive(&self, plr: PlrId) -> bool { self.alive_set.contains(&plr) }

    fn sync_group_rosters(&mut self) { self.groups = self.teams.iter().map(|team| team.roster.clone()).collect(); }

    pub fn alives_by_group(&self, _storage: &std::sync::Arc<crate::engine::storage::Storage>) -> Vec<Vec<PlrId>> {
        self.teams.iter().map(|team| team.alive.clone()).collect()
    }

    pub fn alives_flat(&self, _storage: &std::sync::Arc<crate::engine::storage::Storage>) -> Vec<PlrId> {
        self.flat_alive.clone()
    }

    pub fn next_round_index(&mut self, total: usize) -> usize {
        if total == 0 {
            return 0;
        }
        self.round_pos = (self.round_pos + 1).rem_euclid(total as i32);
        self.round_pos as usize
    }

    pub fn remove_alive(&mut self, plr: PlrId) {
        self.alive_set.remove(&plr);
        if let Some(&team_idx) = self.player_team.get(&plr)
            && let Some(team) = self.teams.get_mut(team_idx)
        {
            team.alive.retain(|id| *id != plr);
        }
        // JS Engine.e: dj 中 C.Array.U(r, a) 会 splice 移除，保持剩余元素顺序
        self.flat_alive.retain(|id| *id != plr);
    }

    pub fn remove_player(&mut self, plr: PlrId) {
        self.remove_alive(plr);

        if let Some(idx) = self.players.iter().position(|x| *x == plr) {
            if self.round_pos <= idx as i32 {
                self.round_pos -= 1;
            }
            self.players.remove(idx);
            self.players_set.remove(&plr);
        }
    }

    pub fn remove_from_roster(&mut self, plr: PlrId) {
        if let Some(&team_idx) = self.player_team.get(&plr)
            && let Some(team) = self.teams.get_mut(team_idx)
        {
            team.roster.retain(|id| *id != plr);
            team.alive.retain(|id| *id != plr);
        }
        self.player_team.remove(&plr);
        self.alive_set.remove(&plr);
        self.flat_alive.retain(|id| *id != plr);
        self.remove_player(plr);
        self.sync_group_rosters();
    }

    fn ensure_player_in_round(&mut self, plr: PlrId) {
        if self.players_set.insert(plr) {
            self.players.push(plr);
        }
    }

    pub fn revive_into_team(&mut self, plr: PlrId, team_idx: usize) {
        self.ensure_player_in_round(plr);
        self.alive_set.insert(plr);
        let already_in_team = self.teams.get(team_idx).map(|t| t.alive.contains(&plr)).unwrap_or(false);
        if let Some(team) = self.teams.get_mut(team_idx)
            && !already_in_team
        {
            team.alive.push(plr);
        }
        // JS Engine.e (aZ) 的插入语义：
        //   r = grp.f (team alive)
        //   if (r.length > 0) splice(indexOf(all_alive, last(r)) + 1, 0, new)
        //   else              push(new)
        //
        // 注意：这里要用 team.alive 在 push 之后的状态，但排除 plr 自身来找
        // "已有的最后一个队友"，因为 plr 刚被 push 进去。
        if !self.flat_alive.contains(&plr) {
            let team_alive = self.teams.get(team_idx).map(|t| &t.alive);
            // JS aZ 的语义：
            //   r = grp.f (team alive，此时尚未 push 新成员)
            //   gbl(r) → r 的最后一个元素
            //   indexOf(all_alive, gbl(r)) → 在 flat list 中找到那个元素的位置
            //   splice(pos + 1, 0, new)
            //
            // 关键：JS 取的是 team alive 数组中的 **最后一个元素**（gbl），
            // 然后查它在 all_alive 中的位置。不是"所有队友中在 flat_alive 里最靠右的"。
            let last_teammate_pos = team_alive.and_then(|alive| {
                // 从 team.alive 尾部往前找第一个不是 plr 的成员（= JS 的 gbl(r)）
                alive
                    .iter()
                    .rev()
                    .find(|id| **id != plr)
                    .and_then(|last_id| self.flat_alive.iter().position(|x| x == last_id))
            });
            if let Some(pos) = last_teammate_pos {
                // 插入到该队友之后（与 JS splice(indexOf + 1, 0, a) 一致）
                self.flat_alive.insert(pos + 1, plr);
            } else {
                // 队伍中无其他存活成员 → 追加到末尾（与 JS aZ 的 else push 分支一致）
                self.flat_alive.push(plr);
            }
        }
    }

    pub fn add_new_player(&mut self, plr: PlrId, owner: PlrId) {
        let Some(team_idx) = self.player_team.get(&owner).copied() else {
            let new_idx = self.teams.len();
            self.teams.push(TeamState {
                roster: vec![plr],
                alive: vec![plr],
            });
            self.player_team.insert(plr, new_idx);
            self.alive_set.insert(plr);
            self.ensure_player_in_round(plr);
            self.sync_group_rosters();
            return;
        };
        if let Some(team) = self.teams.get_mut(team_idx)
            && !team.roster.contains(&plr)
        {
            team.roster.push(plr);
        }
        self.player_team.entry(plr).or_insert(team_idx);
        self.revive_into_team(plr, team_idx);
        self.sync_group_rosters();
    }

    pub fn revive_player(&mut self, plr: PlrId, owner: PlrId) {
        if let Some(team_idx) = self.player_team.get(&plr).copied().or_else(|| self.player_team.get(&owner).copied()) {
            self.revive_into_team(plr, team_idx);
        } else {
            let new_idx = self.teams.len();
            self.teams.push(TeamState {
                roster: vec![plr],
                alive: vec![plr],
            });
            self.player_team.insert(plr, new_idx);
            self.alive_set.insert(plr);
            self.ensure_player_in_round(plr);
            self.sync_group_rosters();
        }
    }

    #[inline]
    pub fn roster_count(&self) -> usize { self.teams.len() }

    pub fn winner_roster(&self, team_idx: usize) -> Option<Vec<PlrId>> {
        self.teams.get(team_idx).map(|team| team.roster.clone())
    }
}

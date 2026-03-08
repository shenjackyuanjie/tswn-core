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

/// 单支队伍的状态快照。
#[derive(Debug, Clone)]
pub struct TeamState {
    pub roster: Vec<PlrId>,
    pub alive: Vec<PlrId>,
}

#[derive(Debug, Clone)]
pub struct WorldState {
    pub teams: Vec<TeamState>,
    pub groups: Vec<Vec<PlrId>>,
    pub winner: Option<Vec<PlrId>>,
    pub players: Vec<PlrId>,
    pub round_pos: i32,
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
        let players = teams.iter().flat_map(|team| team.roster.iter().copied()).collect::<Vec<PlrId>>();
        Self {
            teams,
            groups,
            winner: None,
            players,
            round_pos: -1,
        }
    }

    #[inline]
    pub fn have_winner(&self) -> bool { self.winner.is_some() }

    #[inline]
    pub fn all_plrs(&self) -> Vec<PlrId> { self.teams.iter().flat_map(|team| team.roster.iter().copied()).collect() }

    #[inline]
    pub fn all_plr_len(&self) -> usize { self.teams.iter().map(|team| team.roster.len()).sum() }

    pub fn team_index_of(&self, actor: PlrId) -> Option<usize> { self.teams.iter().position(|team| team.roster.contains(&actor)) }

    #[inline]
    pub fn team_roster(&self, team_idx: usize) -> Option<&[PlrId]> { self.teams.get(team_idx).map(|team| team.roster.as_slice()) }

    #[inline]
    pub fn team_alive(&self, team_idx: usize) -> Option<&[PlrId]> { self.teams.get(team_idx).map(|team| team.alive.as_slice()) }

    #[inline]
    pub fn contains_alive(&self, plr: PlrId) -> bool { self.teams.iter().any(|team| team.alive.contains(&plr)) }

    fn sync_group_rosters(&mut self) {
        self.groups = self.teams.iter().map(|team| team.roster.clone()).collect();
    }

    pub fn alives_by_group(&self, _storage: &std::sync::Arc<crate::engine::storage::Storage>) -> Vec<Vec<PlrId>> {
        self.teams.iter().map(|team| team.alive.clone()).collect()
    }

    pub fn alives_flat(&self, _storage: &std::sync::Arc<crate::engine::storage::Storage>) -> Vec<PlrId> {
        self.teams.iter().flat_map(|team| team.alive.iter().copied()).collect()
    }

    pub fn next_round_index(&mut self, total: usize) -> usize {
        if total == 0 {
            return 0;
        }
        self.round_pos = (self.round_pos + 1).rem_euclid(total as i32);
        self.round_pos as usize
    }

    pub fn remove_alive(&mut self, plr: PlrId) {
        if let Some(team_idx) = self.team_index_of(plr)
            && let Some(team) = self.teams.get_mut(team_idx)
        {
            team.alive.retain(|id| *id != plr);
        }
    }

    pub fn remove_player(&mut self, plr: PlrId) {
        self.remove_alive(plr);

        if let Some(idx) = self.players.iter().position(|x| *x == plr) {
            if self.round_pos <= idx as i32 {
                self.round_pos -= 1;
            }
            self.players.remove(idx);
        }
    }

    pub fn remove_from_roster(&mut self, plr: PlrId) {
        if let Some(team_idx) = self.team_index_of(plr)
            && let Some(team) = self.teams.get_mut(team_idx)
        {
            team.roster.retain(|id| *id != plr);
            team.alive.retain(|id| *id != plr);
        }
        self.remove_player(plr);
        self.sync_group_rosters();
    }

    fn ensure_player_in_round(&mut self, plr: PlrId) {
        if !self.players.contains(&plr) {
            self.players.push(plr);
        }
    }

    pub fn revive_into_team(&mut self, plr: PlrId, team_idx: usize) {
        self.ensure_player_in_round(plr);
        if let Some(team) = self.teams.get_mut(team_idx)
            && !team.alive.contains(&plr)
        {
            team.alive.push(plr);
        }
    }

    pub fn add_new_player(&mut self, plr: PlrId, owner: PlrId) {
        let Some(team_idx) = self.team_index_of(owner) else {
            self.teams.push(TeamState {
                roster: vec![plr],
                alive: vec![plr],
            });
            self.ensure_player_in_round(plr);
            self.sync_group_rosters();
            return;
        };
        if let Some(team) = self.teams.get_mut(team_idx)
            && !team.roster.contains(&plr)
        {
            team.roster.push(plr);
        }
        self.revive_into_team(plr, team_idx);
        self.sync_group_rosters();
    }

    pub fn revive_player(&mut self, plr: PlrId, owner: PlrId) {
        if let Some(team_idx) = self.team_index_of(plr).or_else(|| self.team_index_of(owner)) {
            self.revive_into_team(plr, team_idx);
        } else {
            self.teams.push(TeamState {
                roster: vec![plr],
                alive: vec![plr],
            });
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

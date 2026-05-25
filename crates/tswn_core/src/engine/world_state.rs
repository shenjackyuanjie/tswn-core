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
    /// 存活玩家集合，用 PlrId 稠密下标支持 O(1) contains_alive 查询
    alive_set: Vec<bool>,
    /// 玩家 → 所属队伍索引映射，用 PlrId 稠密下标支持 O(1) team_index_of 查询
    player_team: Vec<Option<usize>>,
    /// 已在行动轮次中的玩家集合，用 PlrId 稠密下标支持 O(1) ensure_player_in_round 去重检查
    players_set: Vec<bool>,
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
    /// JS Engine.y.a.Q 兼容的存活队伍计数。
    ///
    /// 该值在初始化时等于队伍数；当一个队伍被移成空 alive 时递减。
    /// JS 的 `Grp.aZ` 在复活 / addNew 时不会补回这个计数，因此这里也不在
    /// `revive_into_team` 中递增。
    alive_group_count: usize,
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
        let max_id = players.iter().copied().max().unwrap_or(0);
        let mut alive_set = vec![false; max_id + 1];
        for id in teams.iter().flat_map(|t| t.alive.iter().copied()) {
            alive_set[id] = true;
        }
        let mut player_team = vec![None; max_id + 1];
        for (idx, team) in teams.iter().enumerate() {
            for &id in &team.roster {
                player_team[id] = Some(idx);
            }
        }
        let mut players_set = vec![false; max_id + 1];
        for &id in &players {
            players_set[id] = true;
        }
        // 初始 flat_alive 按队伍顺序拼接，与 JS Engine 构造函数中 _.e 的初始顺序一致
        let flat_alive: Vec<PlrId> = teams.iter().flat_map(|team| team.alive.iter().copied()).collect();
        let alive_group_count = teams.iter().filter(|team| !team.alive.is_empty()).count();
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
            alive_group_count,
        }
    }

    #[inline]
    pub fn have_winner(&self) -> bool { self.winner.is_some() }

    #[inline]
    pub fn all_plrs(&self) -> Vec<PlrId> { self.teams.iter().flat_map(|team| team.roster.iter().copied()).collect() }

    #[inline]
    pub fn all_plr_len(&self) -> usize { self.teams.iter().map(|team| team.roster.len()).sum() }

    #[inline]
    pub fn team_index_of(&self, actor: PlrId) -> Option<usize> { self.player_team.get(actor).copied().flatten() }

    #[inline]
    pub fn team_roster(&self, team_idx: usize) -> Option<&[PlrId]> { self.teams.get(team_idx).map(|team| team.roster.as_slice()) }

    #[inline]
    pub fn team_alive(&self, team_idx: usize) -> Option<&[PlrId]> { self.teams.get(team_idx).map(|team| team.alive.as_slice()) }

    #[inline]
    pub fn contains_alive(&self, plr: PlrId) -> bool { self.alive_set.get(plr).copied().unwrap_or(false) }

    #[inline]
    pub fn alive_group_count(&self) -> usize { self.alive_group_count }

    #[inline]
    fn set_alive_index(&mut self, plr: PlrId, alive: bool) {
        if plr >= self.alive_set.len() {
            self.alive_set.resize(plr + 1, false);
        }
        self.alive_set[plr] = alive;
    }

    #[inline]
    fn set_player_team_index(&mut self, plr: PlrId, team_idx: Option<usize>) {
        if plr >= self.player_team.len() {
            self.player_team.resize(plr + 1, None);
        }
        self.player_team[plr] = team_idx;
    }

    #[inline]
    fn set_players_index(&mut self, plr: PlrId, exists: bool) {
        if plr >= self.players_set.len() {
            self.players_set.resize(plr + 1, false);
        }
        self.players_set[plr] = exists;
    }

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
        self.set_alive_index(plr, false);
        if let Some(team_idx) = self.team_index_of(plr)
            && let Some(team) = self.teams.get_mut(team_idx)
        {
            let was_alive = team.alive.contains(&plr);
            team.alive.retain(|id| *id != plr);
            if was_alive && team.alive.is_empty() {
                self.alive_group_count = self.alive_group_count.saturating_sub(1);
            }
        }
        // JS Engine.e: dj 中 C.Array.U(r, a) 会 splice 移除，保持剩余元素顺序
        self.flat_alive.retain(|id| *id != plr);
    }

    pub fn remove_player(&mut self, plr: PlrId) {
        self.remove_alive(plr);

        if let Some(idx) = self.players.iter().position(|x| *x == plr) {
            #[cfg(not(feature = "no_debug"))]
            let round_pos_before = self.round_pos;
            // 对齐 JS Engine.dj():
            //   if (s.ch <= __idx) --s.ch
            // 其中 ch 指向“刚刚被选中的 actor 下标”。
            // 因此当被移除玩家位于当前 round_pos 之后（或正好就是当前下标）时，
            // 需要把 round_pos 左移一格，保证下一次 next_round_index() 取到与 JS 相同的实体。
            if idx as i32 >= self.round_pos {
                self.round_pos -= 1;
            }
            #[cfg(not(feature = "no_debug"))]
            if std::env::var_os("TSWN_DEBUG_WORLD").is_some() {
                eprintln!(
                    "[remove_player] plr={} idx={} round_pos_before={} round_pos_after={} players_len_before={}",
                    plr,
                    idx,
                    round_pos_before,
                    self.round_pos,
                    self.players.len(),
                );
            }
            self.players.remove(idx);
            self.set_players_index(plr, false);
        }
    }

    pub fn remove_from_roster(&mut self, plr: PlrId) {
        if let Some(team_idx) = self.team_index_of(plr)
            && let Some(team) = self.teams.get_mut(team_idx)
        {
            team.roster.retain(|id| *id != plr);
            team.alive.retain(|id| *id != plr);
        }
        self.set_player_team_index(plr, None);
        self.set_alive_index(plr, false);
        self.flat_alive.retain(|id| *id != plr);
        self.remove_player(plr);
        self.sync_group_rosters();
    }

    fn ensure_player_in_round(&mut self, plr: PlrId) {
        if !self.players_set.get(plr).copied().unwrap_or(false) {
            self.set_players_index(plr, true);
            #[cfg(not(feature = "no_debug"))]
            if std::env::var_os("TSWN_DEBUG_TICK_ORDER").is_some() {
                // 这里是 round roster 追加点的调试日志，只在非 `no_debug` 构建里保留。
                eprintln!(
                    "[round_add] plr={} round_pos={} players_len_before={}",
                    plr,
                    self.round_pos,
                    self.players.len()
                );
            }
            self.players.push(plr);
        }
    }

    pub fn revive_into_team(&mut self, plr: PlrId, team_idx: usize) {
        self.ensure_player_in_round(plr);
        self.set_alive_index(plr, true);
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
        let Some(team_idx) = self.team_index_of(owner) else {
            let new_idx = self.teams.len();
            self.teams.push(TeamState {
                roster: vec![plr],
                alive: vec![plr],
            });
            self.set_player_team_index(plr, Some(new_idx));
            self.set_alive_index(plr, true);
            self.alive_group_count += 1;
            self.ensure_player_in_round(plr);
            self.sync_group_rosters();
            return;
        };
        if let Some(team) = self.teams.get_mut(team_idx)
            && !team.roster.contains(&plr)
        {
            team.roster.push(plr);
        }
        if self.team_index_of(plr).is_none() {
            self.set_player_team_index(plr, Some(team_idx));
        }
        self.revive_into_team(plr, team_idx);
        self.sync_group_rosters();
    }

    pub fn revive_player(&mut self, plr: PlrId, owner: PlrId) {
        if let Some(team_idx) = self.team_index_of(plr).or_else(|| self.team_index_of(owner)) {
            self.revive_into_team(plr, team_idx);
        } else {
            let new_idx = self.teams.len();
            self.teams.push(TeamState {
                roster: vec![plr],
                alive: vec![plr],
            });
            self.set_player_team_index(plr, Some(new_idx));
            self.set_alive_index(plr, true);
            self.alive_group_count += 1;
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

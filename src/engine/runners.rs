use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::RunUpdates;
use crate::error::runner::RunnerResult;
use crate::player::{ActionTargets, Player, PlrId};
use crate::rc4::RC4;

pub type PlayerGroup = Vec<Player>;
pub type RawPlayers = (Vec<Vec<String>>, Vec<String>);

#[derive(Debug, Clone)]
pub struct TeamState {
    /// 队伍 roster：队伍成员的静态归属顺序。
    ///
    /// 这个顺序只在“加入/永久移除”时改变，不应因为死亡/复活而重建。
    pub roster: Vec<PlrId>,
    /// 队伍 alive：当前存活成员的运行时顺序。
    ///
    /// 这是本次重构的核心状态。复活、召唤、死亡都必须直接修改它，
    /// 不能再通过“用 roster 过滤全局 alive”来间接得到。
    pub alive: Vec<PlrId>,
}

/// 对局共享世界状态。
#[derive(Debug, Clone)]
pub struct WorldState {
    /// 队伍状态，包含 roster 和当前 alive 顺序。
    pub teams: Vec<TeamState>,
    /// 所有队伍（按创建顺序）。
    /// 兼容镜像：始终与 teams.roster 同步。
    pub groups: Vec<Vec<PlrId>>,
    /// 胜方队伍（存在时表示战斗结束）。
    pub winner: Option<Vec<PlrId>>,
    /// 轮转列表（对齐 Dart 的 `Fgt.players`，包含死的，用于 round 轮转）。
    players: Vec<PlrId>,
    /// 下一次行动的轮转位置（基于 `players`）。
    round_pos: i32,
}

impl WorldState {
    pub fn new(groups: Vec<Vec<PlrId>>) -> Self {
        // 初始化时 team roster 与 team alive 一致；之后两者会分叉演化。
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
        // `groups` 仍保留给旧接口和测试使用，但语义被限制为 roster-only 镜像。
        self.groups = self.teams.iter().map(|team| team.roster.clone()).collect();
    }

    /// 按 teams 分组返回每个队伍的存活列表（保持队内 alive 顺序）。
    pub fn alives_by_group(&self, _storage: &Arc<Storage>) -> Vec<Vec<PlrId>> {
        self.teams.iter().map(|team| team.alive.clone()).collect()
    }

    /// 返回扁平化的存活列表（由 team alive 顺序拼接）。
    pub fn alives_flat(&self, _storage: &Arc<Storage>) -> Vec<PlrId> {
        self.teams.iter().flat_map(|team| team.alive.iter().copied()).collect()
    }

    fn next_round_index(&mut self, total: usize) -> usize {
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
            // 死亡只影响 team-local alive，不改变 roster 归属。
            team.alive.retain(|id| *id != plr);
        }
    }

    /// Dart `Fgt.remove`: player 死亡时从 alive 和 players 中移除，调整 round_pos。
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
            // 永久移除是比死亡更强的操作：同时从 roster 和 alive 抹掉。
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
            // 当前对齐策略：复活/重新加入的成员追加到 team alive 队尾。
            team.alive.push(plr);
        }
    }

    /// Dart `Fgt.addNew`: 新 player 加入 players 末尾；team.roster 加入队尾；alive 加入队内 alive 队尾。
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

    /// Dart `Fgt.revive`: 复活 player，加回对应队伍的 alive 队尾。
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

type ActorHook = fn(PlrId, &Arc<Storage>, &mut RC4, &mut RunUpdates);

#[derive(Default)]
pub struct HookPipeline {
    pre_action: Vec<ActorHook>,
    post_action: Vec<ActorHook>,
    pre_damage: Vec<ActorHook>,
    post_damage: Vec<ActorHook>,
}

impl HookPipeline {
    pub fn register_pre_action(&mut self, hook: ActorHook) { self.pre_action.push(hook); }

    pub fn register_post_action(&mut self, hook: ActorHook) { self.post_action.push(hook); }

    pub fn register_pre_damage(&mut self, hook: ActorHook) { self.pre_damage.push(hook); }

    pub fn register_post_damage(&mut self, hook: ActorHook) { self.post_damage.push(hook); }

    pub fn run_pre_action(&self, actor: PlrId, storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates) {
        for hook in &self.pre_action {
            hook(actor, storage, randomer, updates);
        }
    }

    pub fn run_post_action(&self, actor: PlrId, storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates) {
        for hook in &self.post_action {
            hook(actor, storage, randomer, updates);
        }
    }

    pub fn run_pre_damage(&self, actor: PlrId, storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates) {
        for hook in &self.pre_damage {
            hook(actor, storage, randomer, updates);
        }
    }

    pub fn run_post_damage(&self, actor: PlrId, storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates) {
        for hook in &self.post_damage {
            hook(actor, storage, randomer, updates);
        }
    }
}

#[derive(Default)]
pub struct RuleRegistry {
    pub skill_rules: usize,
    pub weapon_rules: usize,
    pub boss_rules: usize,
}

impl RuleRegistry {
    pub fn register_skill_rule(&mut self) { self.skill_rules += 1; }

    pub fn register_weapon_rule(&mut self) { self.weapon_rules += 1; }

    pub fn register_boss_rule(&mut self) { self.boss_rules += 1; }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionDecision {
    StepDriver,
    Skip,
}

fn next_actor(world: &mut WorldState, _storage: &Arc<Storage>) -> Option<PlrId> {
    if world.players.is_empty() {
        return None;
    }
    let idx = world.next_round_index(world.players.len());
    Some(world.players[idx])
}

fn choose_action(
    actor: PlrId,
    world: &WorldState,
    storage: &Arc<Storage>,
    _randomer: &mut RC4,
    _rules: &RuleRegistry,
) -> ActionDecision {
    if world.have_winner() {
        return ActionDecision::Skip;
    }
    if storage.get_player(&actor).map(|x| x.get_status().alive()).unwrap_or(false) {
        ActionDecision::StepDriver
    } else {
        ActionDecision::Skip
    }
}

pub(super) fn select_targets(actor: PlrId, world: &WorldState, storage: &Arc<Storage>) -> ActionTargets {
    use crate::player::skill::charm::CharmState;

    let Some(team_idx) = world.team_index_of(actor) else {
        return ActionTargets::default();
    };
    let effective_team = storage
        .get_player(&actor)
        .and_then(|player| player.get_state::<CharmState>())
        .and_then(|charm| world.team_index_of(charm.group_id))
        .unwrap_or(team_idx);
    let Some(team_roster) = world.team_roster(effective_team).map(|team| team.to_vec()) else {
        return ActionTargets::default();
    };
    // 目标域显式由 team state 生成：
    // - ally_alive 使用 team.alive 的当前顺序
    // - ally_dead 使用 roster - alive
    // - ally_all 保持 roster 语义，对齐 Dart 的 group.players
    let ally_alive = world.team_alive(effective_team).map(|team| team.to_vec()).unwrap_or_default();
    let ally_all = team_roster.clone();
    let ally_dead = team_roster.iter().copied().filter(|id| !ally_alive.contains(id)).collect::<Vec<PlrId>>();
    let all_alive = world.alives_flat(storage);
    let enemy_alive = world
        .teams
        .iter()
        .enumerate()
        .filter(|(idx, _)| *idx != effective_team)
        .flat_map(|(_, team)| team.alive.iter().copied())
        .collect::<Vec<PlrId>>();

    ActionTargets {
        enemy_alive,
        ally_alive,
        ally_all,
        ally_dead,
        all_alive,
    }
}

pub struct TickContext<'a> {
    pub storage: &'a Arc<Storage>,
    pub randomer: &'a mut RC4,
    pub updates: &'a mut RunUpdates,
}

fn resolve_combat(
    actor: PlrId,
    decision: ActionDecision,
    targets: &ActionTargets,
    ctx: &mut TickContext<'_>,
    hooks: &HookPipeline,
) {
    match decision {
        ActionDecision::StepDriver => {
            hooks.run_pre_damage(actor, ctx.storage, ctx.randomer, ctx.updates);
            if let Some(plr) = ctx.storage.just_get_player_mut(actor) {
                plr.step(ctx.randomer, ctx.updates, ctx.storage, targets);
            }
            hooks.run_post_damage(actor, ctx.storage, ctx.randomer, ctx.updates);
        }
        ActionDecision::Skip => {}
    }
}

fn check_winner(world: &mut WorldState, _storage: &Arc<Storage>) {
    let mut alive_team_indices = world
        .teams
        .iter()
        .enumerate()
        .filter_map(|(idx, team)| (!team.alive.is_empty()).then_some(idx))
        .collect::<Vec<usize>>();

    world.winner = if alive_team_indices.len() == 1 {
        world.winner_roster(alive_team_indices.remove(0))
    } else {
        None
    };
}

fn has_updates(updates: &RunUpdates) -> bool { !updates.updates.is_empty() }

fn run_update_end(storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates) {
    let mut guard = 0usize;
    while guard < 64 && !updates.on_update_end.is_empty() {
        let pending = std::mem::take(&mut updates.on_update_end);
        for actor in pending {
            if let Some(plr) = storage.just_get_player_mut(actor) {
                let _ = plr.on_update_end(randomer, updates, storage);
            }
        }
        guard += 1;
    }
}

#[derive(Default)]
pub struct EngineCore {
    hooks: HookPipeline,
    rules: RuleRegistry,
}

impl EngineCore {
    pub fn register_pre_action_hook(&mut self, hook: ActorHook) { self.hooks.register_pre_action(hook); }

    pub fn register_post_action_hook(&mut self, hook: ActorHook) { self.hooks.register_post_action(hook); }

    fn debug_world_state(tag: &str, world: &WorldState, storage: &Arc<Storage>) {
        if std::env::var_os("TSWN_DEBUG_WORLD").is_none() {
            return;
        }
        let players = world
            .players
            .iter()
            .map(|id| storage.get_player(id).map(|p| p.id_name()).unwrap_or_else(|| format!("#{id}")))
            .collect::<Vec<String>>()
            .join(" -> ");
        eprintln!("[world:{tag}] round_pos={} players=[{}]", world.round_pos, players);
    }

    fn sync_runtime_entities(&self, world: &mut WorldState, storage: &Arc<Storage>) {
        Self::debug_world_state("pre_sync", world, storage);

        // 1+2) 统一处理死亡：按 death_queue 记录的死亡发生顺序逐个移除。
        //   对齐 Dart：onDie 内部即时调用 group.die → f.remove，因此多死亡的 remove
        //   顺序必须与 combat 中死亡发生顺序一致。pending_remove 只表示“该实体已从
        //   运行态退出”，但不应从 team roster/group.players 永久抹掉；否则会改变
        //   AllyAny 抽样池，导致隐藏 RC4 漂移。pending_remove（minion group 移除）
        //   与 death_queue 合并处理，避免 pending_remove 抢先改变 round_pos。
        let pending_remove_players = storage.take_pending_remove_players();
        let death_queue = storage.take_death_queue();
        for id in &death_queue {
            if world.contains_alive(*id) && !storage.get_player(id).map(|p| p.alive()).unwrap_or(false) {
                world.remove_player(*id);
                Self::debug_world_state("after_dead_remove", world, storage);
            }
        }
        // 处理不在 death_queue 中的 pending_remove（理论上不应发生，safety net）。
        // 这里同样不改 roster，只确保它不会残留在 alive/round 视图里。
        for ptr in &pending_remove_players {
            if !death_queue.contains(ptr) {
                world.remove_player(*ptr);
                Self::debug_world_state("after_pending_remove_only", world, storage);
            }
        }
        // Fallback: 处理可能遗漏的死亡（不在 death_queue 中但已标记死亡的 player）。
        let remaining_dead: Vec<PlrId> = world
            .alives_flat(storage)
            .into_iter()
            .filter(|id| !storage.get_player(id).map(|p| p.alive()).unwrap_or(false))
            .collect();
        for id in remaining_dead {
            world.remove_player(id);
            Self::debug_world_state("after_dead_remove_fallback", world, storage);
        }

        // 2.5) 同步复活：把已复活但已从 round/alives 移除的实体加回世界。
        let mut revived_ids: Vec<PlrId> = Vec::new();
        for team in &world.teams {
            for id in &team.roster {
                if world.contains_alive(*id) {
                    continue;
                }
                let revived = storage.get_player(id).map(|p| p.alive()).unwrap_or(false);
                if revived && !revived_ids.contains(id) {
                    revived_ids.push(*id);
                }
            }
        }
        for id in revived_ids {
            world.revive_player(id, id);
            Self::debug_world_state("after_revive_sync", world, storage);
        }

        // 3) 处理 pending spawns
        let pending_spawns = storage.take_pending_spawns();
        for pending in pending_spawns {
            let owner = pending.owner;
            let plr_id = storage.just_insert_player(pending.player);
            world.add_new_player(plr_id, owner);
        }

        storage.sync_groups(&world.groups);
        storage.sync_alive_groups(&world.alives_by_group(storage));
        Self::debug_world_state("post_sync", world, storage);
    }

    pub fn tick(&mut self, world: &mut WorldState, storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates) {
        self.sync_runtime_entities(world, storage);
        if world.have_winner() {
            return;
        }

        let Some(actor) = next_actor(world, storage) else {
            check_winner(world, storage);
            return;
        };
        let debug_tick = std::env::var_os("TSWN_DEBUG_TICK").is_some();
        let rc4_before = if debug_tick { (randomer.i, randomer.j) } else { (0, 0) };
        if debug_tick && let Some(plr) = storage.get_player(&actor) {
            eprintln!(
                "[tick] actor={} mp={} hp={} rc4=({}, {})",
                plr.id_name(),
                plr.move_point(),
                plr.get_status().hp,
                randomer.i,
                randomer.j
            );
        }

        self.hooks.run_pre_action(actor, storage, randomer, updates);
        let decision = choose_action(actor, world, storage, randomer, &self.rules);
        let targets = select_targets(actor, world, storage);
        let mut ctx = TickContext {
            storage,
            randomer,
            updates,
        };
        resolve_combat(actor, decision, &targets, &mut ctx, &self.hooks);
        run_update_end(storage, ctx.randomer, ctx.updates);
        if debug_tick && (ctx.randomer.i != rc4_before.0 || ctx.randomer.j != rc4_before.1) {
            if let Some(plr) = storage.get_player(&actor) {
                let bytes = (ctx.randomer.i as i32 - rc4_before.0 as i32).rem_euclid(256);
                eprintln!(
                    "[tick_end] actor={} rc4=({},{})->({},{}) bytes={}",
                    plr.id_name(),
                    rc4_before.0,
                    rc4_before.1,
                    ctx.randomer.i,
                    ctx.randomer.j,
                    bytes
                );
            }
        }
        self.sync_runtime_entities(world, storage);
        self.hooks.run_post_action(actor, storage, ctx.randomer, ctx.updates);
        check_winner(world, storage);
    }

    pub fn main_round(&mut self, world: &mut WorldState, storage: &Arc<Storage>, randomer: &mut RC4) -> RunUpdates {
        let mut updates = RunUpdates::new();
        let max_ticks = world.all_plr_len().max(1) * 4;
        let mut ticks = 0;

        while ticks < max_ticks && !world.have_winner() && !has_updates(&updates) {
            self.tick(world, storage, randomer, &mut updates);
            ticks += 1;
        }

        updates
    }
}

pub struct Runner {
    /// 随机数发生器（保持与旧实现一致的消费顺序）。
    pub randomer: RC4,
    /// 全局存储层。
    pub storage: Arc<Storage>,
    /// 世界状态。
    pub world: WorldState,
    /// 新架构下的引擎核心流程。
    core: EngineCore,
}

impl Runner {
    /// 从名竞原始输入构建 Runner。
    pub fn new_from_namerena_raw(raw_input: String) -> RunnerResult<Runner> {
        // 根据原始输入解析队伍。
        let (players, seed) = Runner::spilt_namerena_into_groups(raw_input);

        let mut names = players
            .iter()
            .flatten()
            .filter(|str| !Player::check_is_seed(str))
            .map(|str| Player::raw_namerena_to_idname(str))
            .chain(seed.iter().cloned())
            .collect::<Vec<String>>();
        names.sort();
        names.dedup();

        // 原始逻辑：
        // 把名称排序去重后 join "\r"，再作为 RC4 key。
        let keys = names.join("\r");
        let mut randomer = RC4::new(keys.as_bytes(), 1);
        randomer.js_xor_str(&keys);

        let storage = Storage::new_arc();

        // 先完成玩家实例化与分组，sort_int 在后续按名字排序后再初始化。
        let mut inited_plrs: Vec<Vec<PlrId>> = Vec::with_capacity(players.len());
        for plrs in &players {
            let mut group = Vec::with_capacity(plrs.len());
            for plr in plrs {
                if Player::check_is_seed(plr) {
                    continue;
                }
                let player = Player::new_from_namerena_raw(plr.to_string(), storage.clone())?;
                let ptr = storage.just_insert_player(player);
                group.push(ptr);
            }
            if !group.is_empty() {
                inited_plrs.push(group);
            }
        }

        let mut local_plrs = inited_plrs
            .iter()
            .map(|x| {
                x.iter()
                    .map(|p| storage.just_get_player_mut(*p).expect("player not found when local init"))
                    .collect::<Vec<&mut Player>>()
            })
            .collect::<Vec<Vec<&mut Player>>>();

        // 同队升级：与旧实现一致，先做队内双向 upgrade。
        for plr_group in &mut local_plrs {
            if plr_group.len() < 2 {
                continue;
            }
            plr_group.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            for i in 0..plr_group.len() {
                let (left, right) = plr_group.split_at_mut(i + 1);
                let plr_p = &mut left[i];
                for plr_q in right.iter_mut() {
                    if plr_p.clan_name() == plr_q.clan_name() {
                        plr_p.upgrade(plr_q);
                        plr_q.upgrade(plr_p);
                        // println!("{} {} {}", plr_p.id_name(), plr_p.clan_name(), plr_p.display_name());
                        // println!("{} {} {}", plr_q.id_name(), plr_q.clan_name(), plr_q.display_name());
                    }
                }
            }
        }

        // 与 Dart 对齐：按 id_name 排序后逐个 build，再初始化 sort_int。
        let mut sorted_plrs = inited_plrs.iter().flatten().copied().collect::<Vec<PlrId>>();
        sorted_plrs.sort_by(|a, b| {
            let plr_a = storage.get_player(a).expect("plr not found when sorted build");
            let plr_b = storage.get_player(b).expect("plr not found when sorted build");
            plr_a.cmp_by_id_name(plr_b)
        });
        for ptr in sorted_plrs {
            let plr = storage.just_get_player_mut(ptr).expect("plr not found when build");
            plr.build();
            plr.sort_int = randomer.rFFFFFF() as i32;
        }

        for group in &mut inited_plrs {
            group.sort_by(|a, b| {
                let plr_a = storage.get_player(a).expect("plr not found when sort group member");
                let plr_b = storage.get_player(b).expect("plr not found when sort group member");
                plr_a.cmp_for_sort(plr_b)
            });
        }

        let mut sorted_groups = inited_plrs.clone();
        sorted_groups.sort_by(|a, b| {
            let Some(first_a) = a.first() else {
                return std::cmp::Ordering::Less;
            };
            let Some(first_b) = b.first() else {
                return std::cmp::Ordering::Greater;
            };
            let plr_a = storage.get_player(first_a).expect("plr not found when sort group");
            let plr_b = storage.get_player(first_b).expect("plr not found when sort group");
            plr_a.cmp_for_sort(plr_b)
        });

        // 这里顺便把 sorted hash 这块做了。
        // 保持旧版随机流消费顺序，避免战斗回放偏移。
        let sort_groups = sorted_groups.iter().collect::<Vec<&Vec<PlrId>>>();

        for group in &sort_groups {
            for plr in *group {
                let plr = storage.just_get_player_mut(*plr).expect("plr not found when encrypt");
                randomer.encrypt_bytes_no_change(&plr.id_key_name());
            }
            randomer.encrypt_bytes(&mut [0]);
        }

        // println!(
        //     "开始给 move point 之前的 rc4: {} {} {:?}",
        //     randomer.i, randomer.j, randomer.main_val
        // );
        let mut sorted_for_move_point = sorted_groups.iter().flatten().copied().collect::<Vec<PlrId>>();
        sorted_for_move_point.sort_by(|a, b| {
            let plr_a = storage.get_player(a).expect("plr not found when sort move point");
            let plr_b = storage.get_player(b).expect("plr not found when sort move point");
            plr_a.cmp_for_sort(plr_b)
        });
        for ptr in &sorted_for_move_point {
            let plr = storage.just_get_player_mut(*ptr).expect("plr not found when set move point");
            plr.set_move_point(randomer.r255() as i32);
        }

        let mut world = WorldState::new(sorted_groups);
        world.players = sorted_for_move_point;
        storage.sync_groups(&world.groups);
        storage.sync_alive_groups(&world.alives_by_group(&storage));
        if world.roster_count() == 1 {
            world.winner = world.winner_roster(0);
        }

        Ok(Runner {
            randomer,
            storage,
            world,
            core: EngineCore::default(),
        })
    }

    pub fn alives_flat(&self) -> Vec<PlrId> { self.world.alives_flat(&self.storage) }

    /// 以组为单位获取所有存活玩家。
    pub fn alives(&self) -> Vec<Vec<PlrId>> { self.world.alives_by_group(&self.storage) }

    /// 将名竞输入按队伍拆分。
    /// # 说明
    /// - 去除尾部一个或多个换行/空白
    /// - 将 `\r\n` 和 `\r` 统一成 `\n`
    /// - 将大于等于 3 个连续 `\n` 压成 2 个 `\n`
    ///
    /// # 特殊情况处理
    /// - 当前先保留旧行为：seed 行只负责提取，不做跨组修复
    ///
    /// 返回：(队伍列表, seed 行列表)
    #[allow(clippy::needless_return)]
    pub fn spilt_namerena_into_groups(raw_input: String) -> RawPlayers {
        // 去除尾部的一个/多个 `\n` 或空白。
        let raw_input = raw_input.trim_end();
        // 处理 `\r\n`。
        let raw_input = raw_input.replace("\r\n", "\n");
        // 处理 `\r`。
        let mut raw_input = raw_input.replace("\r", "\n");
        // 处理 `\n\n\n...`。
        while raw_input.contains("\n\n\n") {
            raw_input = raw_input.replace("\n\n\n", "\n\n");
        }

        let seed = raw_input
            .split("\n")
            .filter(|x| Player::check_is_seed(x))
            .map(|x| x.to_string())
            .collect::<Vec<String>>();

        // 没有空行分组：一行一个队伍（旧规则）。
        if !raw_input.contains("\n\n") {
            return (raw_input.split("\n").map(|x| vec![x.to_string()]).collect(), seed);
        }

        let raw_groups: Vec<Vec<String>> =
            raw_input.split("\n\n").map(|x| x.split("\n").map(|x| x.to_string()).collect()).collect();

        let mut groups = raw_groups;
        let is_seed_only = |group: &Vec<String>| !group.is_empty() && group.iter().all(|name| Player::check_is_seed(name));
        let mut idx = 0usize;
        while idx < groups.len() {
            if !is_seed_only(&groups[idx]) {
                idx += 1;
                continue;
            }

            // seed 独占组：优先并到前一个非 seed 组，否则并到后一个非 seed 组。
            let prev = (0..idx).rev().find(|x| !is_seed_only(&groups[*x]));
            let next = ((idx + 1)..groups.len()).find(|x| !is_seed_only(&groups[*x]));
            let Some(target_idx) = prev.or(next) else {
                idx += 1;
                continue;
            };

            let seed_group = groups.remove(idx);
            if target_idx < idx {
                groups[target_idx].extend(seed_group);
                idx = target_idx + 1;
            } else {
                let adjusted = target_idx - 1;
                let mut merged = seed_group;
                merged.extend(groups[adjusted].clone());
                groups[adjusted] = merged;
                idx = adjusted + 1;
            }
        }
        return (groups, seed);
    }

    #[inline]
    pub fn have_winner(&self) -> bool { self.world.have_winner() }

    #[inline]
    pub fn all_plrs(&self) -> Vec<PlrId> { self.world.all_plrs() }

    #[inline]
    pub fn all_plr_len(&self) -> usize { self.world.all_plr_len() }

    pub fn main_round(&mut self) -> RunUpdates { self.core.main_round(&mut self.world, &self.storage, &mut self.randomer) }

    pub fn round_tick(&mut self, updates: &mut RunUpdates) {
        self.core.tick(&mut self.world, &self.storage, &mut self.randomer, updates);
    }
}

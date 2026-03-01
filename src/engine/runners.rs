use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::RunUpdates;
use crate::error::runner::RunnerResult;
use crate::player::{ActionTargets, Player, PlrId};
use crate::rc4::RC4;

pub type PlayerGroup = Vec<Player>;
pub type RawPlayers = (Vec<Vec<String>>, Vec<String>);

/// 对局共享世界状态。
#[derive(Debug, Clone)]
pub struct WorldState {
    /// 所有队伍（按创建顺序）。
    pub groups: Vec<Vec<PlrId>>,
    /// 胜方队伍（存在时表示战斗结束）。
    pub winner: Option<Vec<PlrId>>,
    /// 轮转列表（对齐 Dart 的 `Fgt.players`，包含死的，用于 round 轮转）。
    players: Vec<PlrId>,
    /// 存活列表（对齐 Dart 的 `Fgt.alives`，只含活的，用于目标选择）。
    alives: Vec<PlrId>,
    /// 下一次行动的轮转位置（基于 `players`）。
    round_pos: i32,
}

impl WorldState {
    pub fn new(groups: Vec<Vec<PlrId>>) -> Self {
        let players = groups.iter().flatten().copied().collect::<Vec<PlrId>>();
        let alives = players.clone();
        Self {
            groups,
            winner: None,
            players,
            alives,
            round_pos: -1,
        }
    }

    #[inline]
    pub fn have_winner(&self) -> bool { self.winner.is_some() }

    #[inline]
    pub fn all_plrs(&self) -> Vec<PlrId> { self.groups.iter().flatten().copied().collect() }

    #[inline]
    pub fn all_plr_len(&self) -> usize { self.groups.iter().map(|x| x.len()).sum() }

    pub fn team_index_of(&self, actor: PlrId) -> Option<usize> { self.groups.iter().position(|group| group.contains(&actor)) }

    /// 按 groups 分组返回每个队伍的存活列表（保持 alives 中的顺序）。
    pub fn alives_by_group(&self, _storage: &Arc<Storage>) -> Vec<Vec<PlrId>> {
        self.groups
            .iter()
            .map(|group| {
                // 只保留在 self.alives 中出现的成员，顺序取 group 内顺序
                group.iter().copied().filter(|id| self.alives.contains(id)).collect::<Vec<PlrId>>()
            })
            .collect()
    }

    /// 返回扁平化的存活列表（直接用维护好的 alives）。
    pub fn alives_flat(&self, _storage: &Arc<Storage>) -> Vec<PlrId> { self.alives.clone() }

    fn next_round_index(&mut self, total: usize) -> usize {
        if total == 0 {
            return 0;
        }
        self.round_pos = (self.round_pos + 1).rem_euclid(total as i32);
        self.round_pos as usize
    }

    /// Dart `Fgt.remove`: player 死亡时从 alives 和 players 中移除，调整 round_pos。
    pub fn remove_player(&mut self, plr: PlrId) {
        self.alives.retain(|x| *x != plr);

        if let Some(idx) = self.players.iter().position(|x| *x == plr) {
            if self.round_pos <= idx as i32 {
                self.round_pos -= 1;
            }
            self.players.remove(idx);
        }
    }

    /// Dart `Fgt.addNew`: 新 player 加入 players 末尾；加入 alives 时插入到同队最后一个活人后面。
    pub fn add_new_player(&mut self, plr: PlrId, owner: PlrId) {
        if !self.players.contains(&plr) {
            self.players.push(plr);
        }
        if !self.alives.contains(&plr) {
            // 找到 owner 所在队伍
            let team_alives: Vec<PlrId> = self
                .team_index_of(owner)
                .and_then(|ti| self.groups.get(ti))
                .map(|group| group.iter().copied().filter(|id| self.alives.contains(id)).collect())
                .unwrap_or_default();
            if let Some(&last) = team_alives.last() {
                if let Some(pos) = self.alives.iter().position(|x| *x == last) {
                    self.alives.insert(pos + 1, plr);
                } else {
                    self.alives.push(plr);
                }
            } else {
                self.alives.push(plr);
            }
        }
    }

    /// Dart `Fgt.revive`: 复活 player，加回 alives（插入到同队最后一个活人后面）。
    pub fn revive_player(&mut self, plr: PlrId, owner: PlrId) {
        if !self.players.contains(&plr) {
            self.players.push(plr);
        }
        if !self.alives.contains(&plr) {
            let team_alives: Vec<PlrId> = self
                .team_index_of(owner)
                .and_then(|ti| self.groups.get(ti))
                .map(|group| group.iter().copied().filter(|id| self.alives.contains(id)).collect())
                .unwrap_or_default();
            if let Some(&last) = team_alives.last() {
                if let Some(pos) = self.alives.iter().position(|x| *x == last) {
                    self.alives.insert(pos + 1, plr);
                } else {
                    self.alives.push(plr);
                }
            } else {
                self.alives.push(plr);
            }
        }
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
    let Some(ally_all) = world.groups.get(effective_team).cloned() else {
        return ActionTargets::default();
    };
    let all_alive = world.alives_flat(storage);
    let enemy_alive = all_alive.iter().copied().filter(|id| !ally_all.contains(id)).collect::<Vec<PlrId>>();
    let ally_alive = ally_all.iter().copied().filter(|id| all_alive.contains(id)).collect::<Vec<PlrId>>();
    let ally_dead = ally_all.iter().copied().filter(|id| !all_alive.contains(id)).collect::<Vec<PlrId>>();

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

fn check_winner(world: &mut WorldState, storage: &Arc<Storage>) {
    let mut alive_groups = world
        .alives_by_group(storage)
        .into_iter()
        .filter(|group| !group.is_empty())
        .collect::<Vec<Vec<PlrId>>>();

    world.winner = if alive_groups.len() == 1 {
        Some(alive_groups.remove(0))
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
        // 1) 先处理 pending removes（minion 等需要从行动列表中移除的）。
        // 对齐 JS：由状态回调触发的 remove 会先影响 round 指针，再处理普通死亡移除。
        let pending_remove_players = storage.take_pending_remove_players();
        if !pending_remove_players.is_empty() {
            for ptr in pending_remove_players {
                world.remove_player(ptr);
                for group in &mut world.groups {
                    group.retain(|x| *x != ptr);
                }
                Self::debug_world_state("after_pending_remove", world, storage);
            }
        }

        // 2) 同步死亡：将 alives/players 中已死的 player 移除（对齐 Dart 的 group.die → f.remove）
        let dead_ids: Vec<PlrId> = world
            .alives
            .iter()
            .copied()
            .filter(|id| !storage.get_player(id).map(|p| p.alive()).unwrap_or(false))
            .collect();
        for id in dead_ids {
            world.remove_player(id);
            Self::debug_world_state("after_dead_remove", world, storage);
        }

        // 2.5) 同步复活：把已复活但已从 round/alives 移除的实体加回世界。
        let mut revived_ids: Vec<PlrId> = Vec::new();
        for group in &world.groups {
            for id in group {
                if world.alives.contains(id) {
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
            let owner_team = world.team_index_of(owner);
            if let Some(team_idx) = owner_team {
                if let Some(group) = world.groups.get_mut(team_idx) {
                    group.push(plr_id);
                }
            } else {
                world.groups.push(vec![plr_id]);
            }
            // 对齐 Dart: addNew — players 末尾, alives 插入同队最后一个活人后面
            world.add_new_player(plr_id, owner);
        }

        storage.sync_groups(&world.groups);
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
        if std::env::var_os("TSWN_DEBUG_TICK").is_some()
            && let Some(plr) = storage.get_player(&actor)
        {
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
        // Dart: alives 的初始顺序是按 groups 分组的（先 group1 的所有成员，再 group2...），
        // 而不是全局排序。对齐 Dart 的:
        //   for (Grp g in groups) { alives.addAll(g.alives); }
        world.alives = world.groups.iter().flatten().copied().collect();
        storage.sync_groups(&world.groups);
        if world.groups.len() == 1 {
            world.winner = Some(world.groups[0].clone());
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

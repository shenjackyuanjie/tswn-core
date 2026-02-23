/// namerena 评分机制里的第一个靶子。
pub const PROFILE_START: u32 = 33_554_431;

/// 由 Bevy ECS 启发的轻量存储层。
/// 目前集中托管 Skill / Group / Player 的运行期所有权。
pub mod storage {
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

        /// 获取玩家（不做 Option 检查）。
        pub fn get_player_unchecked(&self, ptr: &PlrId) -> &Player {
            self.players.get(ptr).expect("cannot get player from storage")
        }

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
}

/// 核心的游戏逻辑（runner + engine core）。
pub mod runners {
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
        /// 下一次行动的轮转位置。
        round_pos: i32,
    }

    impl WorldState {
        pub fn new(groups: Vec<Vec<PlrId>>) -> Self {
            Self {
                groups,
                winner: None,
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

        pub fn alives(&self, storage: &Arc<Storage>) -> Vec<Vec<PlrId>> {
            self.groups
                .iter()
                .map(|group| {
                    group
                        .iter()
                        .filter(|plr| storage.get_player(plr).map(|x| x.get_status().alive()).unwrap_or(false))
                        .copied()
                        .collect::<Vec<PlrId>>()
                })
                .collect::<Vec<Vec<PlrId>>>()
        }

        pub fn alives_flat(&self, storage: &Arc<Storage>) -> Vec<PlrId> {
            self.alives(storage).iter().flatten().copied().collect()
        }

        fn next_round_index(&mut self, total: usize) -> usize {
            if total == 0 {
                return 0;
            }
            self.round_pos = (self.round_pos + 1).rem_euclid(total as i32);
            self.round_pos as usize
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

    #[derive(Default)]
    pub struct TurnScheduler;

    impl TurnScheduler {
        pub fn next_actor(&self, world: &mut WorldState, storage: &Arc<Storage>) -> Option<PlrId> {
            let all = world.all_plrs();
            if all.is_empty() {
                return None;
            }

            for _ in 0..all.len() {
                let idx = world.next_round_index(all.len());
                let actor = all[idx];
                if storage.get_player(&actor).map(|x| x.get_status().alive()).unwrap_or(false) {
                    return Some(actor);
                }
            }
            None
        }
    }

    #[derive(Default)]
    pub struct ActionSystem;

    impl ActionSystem {
        pub fn choose_action(
            &self,
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
    }

    #[derive(Default)]
    pub struct TargetSystem;

    impl TargetSystem {
        pub fn select_targets(&self, actor: PlrId, world: &WorldState, storage: &Arc<Storage>) -> ActionTargets {
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

            let alive_groups = world.alives(storage);
            let all_alive = alive_groups.iter().flatten().copied().collect::<Vec<PlrId>>();
            let enemy_alive = alive_groups
                .into_iter()
                .enumerate()
                .filter_map(|(idx, group)| if idx == effective_team { None } else { Some(group) })
                .flatten()
                .collect::<Vec<PlrId>>();
            let ally_alive = ally_all
                .iter()
                .copied()
                .filter(|id| storage.get_player(id).map(|x| x.get_status().alive()).unwrap_or(false))
                .collect::<Vec<PlrId>>();
            let ally_dead = ally_all
                .iter()
                .copied()
                .filter(|id| !storage.get_player(id).map(|x| x.get_status().alive()).unwrap_or(false))
                .collect::<Vec<PlrId>>();

            ActionTargets {
                enemy_alive,
                ally_alive,
                ally_all,
                ally_dead,
                all_alive,
            }
        }
    }

    pub struct TickContext<'a> {
        pub storage: &'a Arc<Storage>,
        pub randomer: &'a mut RC4,
        pub updates: &'a mut RunUpdates,
    }

    #[derive(Default)]
    pub struct CombatResolver;

    impl CombatResolver {
        pub fn resolve(
            &self,
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
    }

    #[derive(Default)]
    pub struct WinChecker;

    impl WinChecker {
        pub fn check(&self, world: &mut WorldState, storage: &Arc<Storage>) {
            let mut alive_groups = world
                .alives(storage)
                .into_iter()
                .filter(|group| !group.is_empty())
                .collect::<Vec<Vec<PlrId>>>();

            world.winner = if alive_groups.len() == 1 {
                Some(alive_groups.remove(0))
            } else {
                None
            };
        }
    }

    #[derive(Default)]
    pub struct RunUpdateCollector;

    impl RunUpdateCollector {
        pub fn has_updates(&self, updates: &RunUpdates) -> bool { !updates.updates.is_empty() }
    }

    #[derive(Default)]
    pub struct EngineCore {
        scheduler: TurnScheduler,
        action_system: ActionSystem,
        target_system: TargetSystem,
        combat_resolver: CombatResolver,
        hooks: HookPipeline,
        rules: RuleRegistry,
        win_checker: WinChecker,
        collector: RunUpdateCollector,
    }

    impl EngineCore {
        pub fn register_pre_action_hook(&mut self, hook: ActorHook) { self.hooks.register_pre_action(hook); }

        pub fn register_post_action_hook(&mut self, hook: ActorHook) { self.hooks.register_post_action(hook); }

        fn sync_runtime_entities(&self, world: &mut WorldState, storage: &Arc<Storage>) {
            let pending_spawns = storage.take_pending_spawns();
            for pending in pending_spawns {
                let owner_team = world.team_index_of(pending.owner);
                let plr_id = storage.just_insert_player(pending.player);
                if let Some(team_idx) = owner_team {
                    if let Some(group) = world.groups.get_mut(team_idx) {
                        group.push(plr_id);
                    }
                } else {
                    world.groups.push(vec![plr_id]);
                }
            }

            let pending_remove_players = storage.take_pending_remove_players();
            if !pending_remove_players.is_empty() {
                for ptr in pending_remove_players {
                    for group in &mut world.groups {
                        group.retain(|x| *x != ptr);
                    }
                    storage.just_remove_player(ptr);
                }
            }

            storage.sync_groups(&world.groups);
        }

        pub fn tick(&mut self, world: &mut WorldState, storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates) {
            self.sync_runtime_entities(world, storage);
            if world.have_winner() {
                return;
            }

            let Some(actor) = self.scheduler.next_actor(world, storage) else {
                self.win_checker.check(world, storage);
                return;
            };

            self.hooks.run_pre_action(actor, storage, randomer, updates);
            let decision = self.action_system.choose_action(actor, world, storage, randomer, &self.rules);
            let targets = self.target_system.select_targets(actor, world, storage);
            let mut ctx = TickContext {
                storage,
                randomer,
                updates,
            };
            self.combat_resolver.resolve(actor, decision, &targets, &mut ctx, &self.hooks);
            self.sync_runtime_entities(world, storage);
            self.hooks.run_post_action(actor, storage, ctx.randomer, ctx.updates);
            self.win_checker.check(world, storage);
        }

        pub fn main_round(&mut self, world: &mut WorldState, storage: &Arc<Storage>, randomer: &mut RC4) -> RunUpdates {
            let mut updates = RunUpdates::new();
            let max_ticks = world.all_plr_len().max(1) * 4;
            let mut ticks = 0;

            while ticks < max_ticks && !world.have_winner() && !self.collector.has_updates(&updates) {
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

            // 用 randomer 初始化玩家的 sort_int。
            let mut inited_plrs: Vec<Vec<PlrId>> = Vec::with_capacity(players.len());
            for plrs in &players {
                let mut group = Vec::with_capacity(plrs.len());
                for plr in plrs {
                    let mut player = Player::new_from_namerena_raw(plr.to_string(), storage.clone())?;
                    player.sort_int = randomer.rFFFFFF() as i32;
                    let ptr = storage.just_insert_player(player);
                    group.push(ptr);
                }
                inited_plrs.push(group);
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

            // 构建所有玩家：属性/技能初始化入口。
            for group in &mut local_plrs {
                for plr in group {
                    plr.build();
                }
            }

            // 这里顺便把 sorted hash 这块做了。
            // 保持旧版随机流消费顺序，避免战斗回放偏移。
            let mut sort_groups = inited_plrs.iter().collect::<Vec<&Vec<PlrId>>>();
            sort_groups.sort_by(|a, b| {
                {
                    let plr_a = storage.get_player(&a[0]).expect("plr not found when sort");
                    let plr_b = storage.get_player(&b[0]).expect("plr not found when sort");
                    plr_a.partial_cmp(plr_b)
                }
                .unwrap_or(std::cmp::Ordering::Equal)
            });

            for group in &sort_groups {
                for plr in *group {
                    let plr = storage.just_get_player_mut(*plr).expect("plr not found when encrypt");
                    randomer.encrypt_bytes_no_change(&plr.id_name());
                }
                randomer.encrypt_bytes(&mut [0]);
            }

            for group in &inited_plrs {
                for plr in group {
                    let plr = storage.just_get_player_mut(*plr).expect("plr not found when set move point");
                    plr.set_move_point(randomer.r255() as i32);
                }
            }

            let mut world = WorldState::new(inited_plrs);
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
        pub fn alives(&self) -> Vec<Vec<PlrId>> { self.world.alives(&self.storage) }

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
}

pub mod update {
    use crate::player::PlrId;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum UpdateType {
        /// 赢！
        Win,
        /// 没动作。
        None,
        /// 下一行（用于换行分隔）。
        NextLine,
    }

    #[derive(Debug, Clone)]
    pub struct RunUpdate {
        pub score: u32,
        pub delay0: i32,
        pub delay1: i32,
        pub message: String,
        pub caster: PlrId,
        pub target: PlrId,
        pub targets: Vec<PlrId>,
        pub update_type: UpdateType,
    }

    impl RunUpdate {
        pub fn new_dummy() -> RunUpdate {
            RunUpdate {
                score: 0,
                delay0: 0,
                delay1: 0,
                message: "\n".to_string(),
                caster: 0,
                target: 0,
                targets: vec![],
                update_type: UpdateType::None,
            }
        }

        pub fn msg(&self) -> String {
            let mut msg = self.message.clone();
            // param: Object ?
            // [0] -> caster
            // [1] -> target
            // [2] -> targets
            msg = msg.replace("[0]", &self.caster.to_string());
            msg = msg.replace("[1]", &self.target.to_string());
            msg = msg.replace(
                "[2]",
                &self.targets.iter().map(|x| x.to_string()).collect::<Vec<String>>().join(","),
            );
            msg
        }

        pub fn new_newline() -> RunUpdate {
            RunUpdate {
                score: 0,
                delay0: 0,
                delay1: 0,
                message: "\n".to_string(),
                caster: 0,
                target: 0,
                targets: vec![],
                update_type: UpdateType::NextLine,
            }
        }

        pub fn new(msg: impl ToString, caster: PlrId, target: PlrId, score: u32) -> Self {
            RunUpdate {
                score,
                delay0: 0,
                delay1: 0,
                message: msg.to_string(),
                caster,
                target,
                targets: Vec::new(),
                update_type: UpdateType::None,
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct RunUpdates {
        pub updates: Vec<RunUpdate>,
    }

    impl RunUpdates {
        pub fn new() -> RunUpdates { RunUpdates { updates: vec![] } }

        pub fn add(&mut self, update: RunUpdate) { self.updates.push(update); }

        pub fn add_all(&mut self, updates: &mut [RunUpdate]) { self.updates.extend_from_slice(updates); }
    }
}

#[cfg(test)]
mod group {
    use super::*;

    /// 酒吧点炒饭列表（确信）。
    macro_rules! str_vec {
        () => {{
            let vec: Vec<String> = Vec::with_capacity(0);
            vec
        }};
    }

    macro_rules! plr {
        () => {
            str_vec!()
        };
        ($($x:expr),+ $(,)?) => (
            vec![
                $($x.to_string()),+
            ]
        );
    }

    macro_rules! plrs {
        () => {
            str_vec!(str_vec!())
        };
        ($($x:expr),+ $(,)?) => (
            vec![
                $(vec![
                    $x.to_string()
                ],)+
            ]
        );
    }

    mod spilt_namerena_groups {
        use super::*;

        #[test]
        fn basic_spilt() {
            let raw_input = "a\nb\nc".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (plrs!("a", "b", "c"), plr!()));

            let raw_input = "a\nb\nc\n".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (plrs!("a", "b", "c"), plr!()));

            let raw_input = "a\nb\nc\n\n".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (plrs!("a", "b", "c"), plr!()));
        }

        #[test]
        fn spilt_teams() {
            let raw_input = "a\nb\n\nc\nd".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (vec![plr!["a", "b"], plr!["c", "d"]], plr!()));
        }

        #[test]
        fn more_than_2_newline() {
            for x in 2..10 {
                let new_lines = "\n".repeat(x);
                let raw_input = format!("a\nb{new_lines}c\nd");
                let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
                assert_eq!(groups, (vec![plr!["a", "b"], plr!["c", "d"]], plr!()));
            }

            for x in 2..10 {
                let new_lines = "\n".repeat(x);
                let raw_input = format!("a\nb{new_lines}c\nd{new_lines}e");
                let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
                assert_eq!(groups, (vec![plr!["a", "b"], plr!["c", "d"], plr!["e"]], plr!()));
            }
        }

        #[test]
        fn lot_of_teams() {
            let raw_input = "a\nb\nc\nd\ne\nf".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (plrs!("a", "b", "c", "d", "e", "f"), plr!()));
        }

        #[test]
        fn normal_seed() {
            let raw_input = "seed: a@!\nb\nc".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (plrs!("seed: a@!", "b", "c"), plr!["seed: a@!"]));
        }

        #[test]
        fn need_fix_seed1() {
            let raw_input = "aaaa\nbbbb\n\nseed: a@!".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (vec![plr!("aaaa", "bbbb", "seed: a@!")], plr!["seed: a@!"]))
        }

        #[test]
        fn need_fix_seed2() {
            let raw_input = "seed: a@!\n\naaaa\nbbbb".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (vec![plr!("seed: a@!", "aaaa", "bbbb")], plr!["seed: a@!"]))
        }
    }

    mod runner {
        use super::*;

        #[test]
        fn sort_int_test() {
            let raw_input = "aaa\nbbb\nseed: aaaa@!";
            let runner = runners::Runner::new_from_namerena_raw(raw_input.to_string()).unwrap();

            let ints = [16_391_432, 11_292_362];
            assert!(!runner.have_winner());

            for (i, plr) in runner
                .world
                .groups
                .iter()
                .flatten()
                .filter(|plr| runner.storage.get_player(plr).expect("wtf").is_seed_plr())
                .enumerate()
            {
                let plr = runner.storage.get_player(plr).expect("plr not found");
                assert_eq!(plr.sort_int as u32, ints[i]);
            }
        }

        #[test]
        fn sort_int_test2() {
            let raw_input = "aaa\nbbb";
            let runner = runners::Runner::new_from_namerena_raw(raw_input.to_string()).unwrap();

            let ints = [7_525_315, 8_712_372];
            assert!(!runner.have_winner());

            for (i, plr) in runner.world.groups.iter().flatten().enumerate() {
                let plr = runner.storage.get_player(plr).expect("plr not found");
                assert_eq!(plr.sort_int as u32, ints[i]);
            }
        }

        #[test]
        fn charm_state_redirects_target_group() {
            let raw_input = "a\nc\n\nb";
            let runner = runners::Runner::new_from_namerena_raw(raw_input.to_string()).unwrap();
            let actor = runner.world.groups[0][0];
            let ally = runner.world.groups[0][1];
            let enemy = runner.world.groups[1][0];
            runner.storage.just_get_player_mut(actor).expect("cannot get actor").set_state(
                crate::player::skill::charm::CharmState {
                    group_id: enemy,
                    target: Some(actor),
                    on_post_action: None,
                    step: 2,
                },
            );

            let target_system = runners::TargetSystem;
            let targets = target_system.select_targets(actor, &runner.world, &runner.storage);
            assert!(targets.enemy_alive.contains(&ally));
            assert!(!targets.enemy_alive.contains(&enemy));
        }

        #[test]
        fn runtime_spawn_queue_syncs_into_world_group() {
            let raw_input = "owner\n\nenemy";
            let mut runner = runners::Runner::new_from_namerena_raw(raw_input.to_string()).unwrap();
            let owner = runner.world.groups[0][0];
            let mut minion =
                crate::player::Player::new_from_namerena_raw("owner?minion".to_string(), runner.storage.clone()).unwrap();
            minion.set_state(crate::player::skill::act::minion::MinionRuntimeState {
                owner: Some(owner),
                kind: crate::player::skill::act::minion::MinionKind::Clone,
            });
            let minion_id = minion.as_ptr();
            runner.storage.queue_spawn(owner, minion);

            let mut updates = crate::engine::update::RunUpdates::new();
            runner.round_tick(&mut updates);
            assert!(runner.world.groups[0].contains(&minion_id));
        }

        #[test]
        fn runtime_remove_queue_syncs_world_and_storage() {
            let raw_input = "owner\n\nenemy";
            let mut runner = runners::Runner::new_from_namerena_raw(raw_input.to_string()).unwrap();
            let enemy = runner.world.groups[1][0];
            runner.storage.queue_remove_player(enemy);

            let mut updates = crate::engine::update::RunUpdates::new();
            runner.round_tick(&mut updates);
            assert!(!runner.world.groups[1].contains(&enemy));
            assert!(runner.storage.get_player(&enemy).is_none());
        }
    }
}

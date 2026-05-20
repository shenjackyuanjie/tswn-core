//! # 对局 Runner (runners)
//!
//! 本模块提供 [`Runner`]，是外部使用整个战斗引擎的**唯一入口**。
//!
//! ## 使用方式
//!
//! ```rust,no_run
//! use tswn_core::Runner;
//!
//! let input = "player1\n\nplayer2".to_string();
//! let mut runner = Runner::new_from_namerena_raw(input).unwrap();
//!
//! // 逐回合推进，直到有胜负
//! while !runner.have_winner() {
//!     let updates = runner.main_round();
//!     // 处理 updates.updates 中的事件帧...
//! }
//! ```
//!
//! ## 初始化流程
//!
//! `new_from_namerena_raw` 按以下顺序初始化：
//!
//! 1. 解析原始输入，拆分队伍与种子行
//! 2. 去重名字列表并生成 RC4 随机数种子
//! 3. 按组创建玩家实例，注入 Storage
//! 4. 同组玩家双向 `upgrade`（同族名加成）
//! 5. 按 `id_name` 排序后逐个 `build`（计算八围 + 技能熟练度）
//! 6. 为 Boss 玩家初始化专属状态
//! 7. 按 `sort_int` 排序确定战斗顺序
//! 8. 为每个玩家分配初始 `move_point`
//! 9. 构建 `WorldState`

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock, RwLock};

use crate::engine::storage::Storage;
use crate::engine::update::RunUpdates;
use crate::error::runner::RunnerResult;
use crate::player::utils::trim_js_line_end;
use crate::player::{Player, PlrId};
use crate::rc4::RC4;

/// 一组玩家的集合类型，供内部初始化使用。
pub type PlayerGroup = Vec<Player>;
/// 原始输入解析结果：(队伍列表, 种子行列表)。
pub type RawPlayers = (Vec<Vec<String>>, Vec<String>);

use crate::engine::{engine_core::EngineCore, world_state::WorldState};

type PreparedGroups = Vec<Vec<Player>>;
struct PreparedRunnerTemplate {
    groups: PreparedGroups,
    base_names_sorted: Vec<String>,
    base_key: String,
    id_key_names: Vec<String>,
    sorted_by_id_name: Vec<PlrId>,
    eval_rq: f64,
}

/// 可复用的对局预构建模板。
///
/// `PreparedRunner` 本身不是一场正在运行的对局；它更像是把“与 seed 无关、但构造成本较高”的那部分初始化结果先缓存下来，
/// 以便后续在相同输入下反复按不同 seed 构造具体的 [`Runner`]。
///
/// 适用场景：
///
/// - 同一组输入需要批量跑很多局（如 win-rate / benchmark / Monte Carlo）
/// - 希望避免每次都重新解析输入、实例化玩家并完成 build
/// - 需要在保持 `raw` 路径语义一致的前提下，提高重复模拟性能
///
/// 不适合的场景：
///
/// - 只跑单局对战；此时直接使用 [`Runner::new_from_namerena_raw`] 通常更直接
///
/// 与 [`Runner`] 的区别：
///
/// - [`Runner`]：表示一场“具体可运行”的对局，可推进回合、读取 winner、读取 updates
/// - `PreparedRunner`：表示一份“可重复产出 Runner 的模板”，自身不承载对局过程
///
/// 关于 seed：
///
/// - 不传 seed 时，应传空切片 `&[]`
/// - 传 seed 时，应传与 raw 文本一致的完整 `seed:...` 行
/// - 例如：`&["seed:33554431@!".to_string()]`
///
/// 这样可以与 `raw -> split_namerena_into_groups -> new_from_groups_with_seed(...)`
/// 的行为保持一致。
#[derive(Clone)]
pub struct PreparedRunner {
    template: Arc<PreparedRunnerTemplate>,
}

fn groups_cache_key(players: &[Vec<String>], eval_rq: f64) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for group in players {
        group.hash(&mut hasher);
        0xFF_u8.hash(&mut hasher);
    }
    eval_rq.to_bits().hash(&mut hasher);
    hasher.finish()
}

fn prebuilt_groups_cache() -> &'static RwLock<HashMap<u64, Arc<PreparedRunnerTemplate>>> {
    static CACHE: OnceLock<RwLock<HashMap<u64, Arc<PreparedRunnerTemplate>>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// 核心 Runner 结构，包含随机数发生器、存储层、世界状态和引擎核心流程。
pub struct Runner {
    /// 随机数发生器（保持与旧实现一致的消费顺序）。
    pub randomer: RC4,
    /// 全局存储层。
    pub storage: Arc<Storage>,
    /// 世界状态。
    pub world: WorldState,
    /// 原始输入顺序对应的队伍 roster（不受 sort 后 world.teams 顺序影响）。
    pub input_groups: Vec<Vec<PlrId>>,
    /// 新架构下的引擎核心流程。
    core: EngineCore,
}

impl Runner {
    /// 从名竞原始输入构建 Runner。
    pub fn new_from_namerena_raw(raw_input: String) -> RunnerResult<Runner> {
        // 根据原始输入解析队伍。
        let (players, seed) = Runner::split_namerena_into_groups(raw_input);
        Runner::new_from_groups_with_seed(&players, &seed)
    }

    /// 从已解析队伍和 seed 列表构建 Runner。
    ///
    /// 该接口用于高频 benchmark 场景，可复用分组解析结果，避免重复字符串切分成本。
    pub fn new_from_groups_with_seed(players: &[Vec<String>], seed: &[String]) -> RunnerResult<Runner> {
        Self::new_from_groups_with_seed_and_eval_rq(players, seed, crate::player::eval_name::DEFAULT_EVAL_RQ)
    }

    /// 从已解析队伍和 seed 列表构建 Runner，并显式指定名字强度评估使用的 `rq`。
    pub fn new_from_groups_with_seed_and_eval_rq(players: &[Vec<String>], seed: &[String], eval_rq: f64) -> RunnerResult<Runner> {
        let prepared = Self::prepare_groups_with_eval_rq(players, eval_rq)?;
        Self::new_from_prepared_with_seed(&prepared, seed)
    }

    /// 预构建一份可复用的对局模板，后续可多次按不同 seed 构造 [`Runner`]。
    ///
    /// 这个接口会把与 seed 无关的初始化工作提前做掉，例如：
    ///
    /// - 玩家字符串解析
    /// - 玩家实例构造
    /// - 同队 upgrade
    /// - build 后得到的基础属性 / 技能模板
    ///
    /// 之后调用方可以通过 [`Runner::new_from_prepared_with_seed`] 反复构造具体对局，
    /// 适合同一输入下批量跑很多局。
    ///
    /// 注意：
    ///
    /// - `players` 是“已按组拆分后的输入”，不需要额外传 raw 文本
    /// - 这里只 prepare，不会实际开始一场对局
    /// - 若只跑单局，直接从 raw 构造 [`Runner`] 往往更简单
    pub fn prepare_groups(players: &[Vec<String>]) -> RunnerResult<PreparedRunner> {
        Self::prepare_groups_with_eval_rq(players, crate::player::eval_name::DEFAULT_EVAL_RQ)
    }

    /// 预构建一份可复用的对局模板，并显式指定名字强度评估使用的 `rq`。
    ///
    /// 与 [`Runner::prepare_groups`] 相比，这个版本允许调用方显式控制 `eval_rq`，
    /// 从而保证 prepare 路径与后续对局路径使用完全一致的名字评估语义。
    ///
    /// 返回的 [`PreparedRunner`] 可被重复复用；相同 `players + eval_rq` 组合还会命中内部缓存，
    /// 避免重复构建同一份模板。
    pub fn prepare_groups_with_eval_rq(players: &[Vec<String>], eval_rq: f64) -> RunnerResult<PreparedRunner> {
        let cache_key = groups_cache_key(players, eval_rq);
        let template = {
            if let Some(hit) = prebuilt_groups_cache().read().expect("prebuilt cache poisoned").get(&cache_key).cloned() {
                hit
            } else {
                let built = Arc::new(Self::build_prepared_groups(players, eval_rq)?);
                let mut writer = prebuilt_groups_cache().write().expect("prebuilt cache poisoned");
                writer.entry(cache_key).or_insert_with(|| Arc::clone(&built)).clone()
            }
        };
        Ok(PreparedRunner { template })
    }

    /// 通过 [`PreparedRunner`] 和 seed 列表构建一场具体的 [`Runner`]。
    ///
    /// 这是 `PreparedRunner` 的主要用途：在同一份已 prepare 的模板上，
    /// 反复按不同 seed 构造具体对局。
    ///
    /// `seed` 的约定与 raw 路径保持一致：
    ///
    /// - 不传 seed：使用空切片 `&[]`
    /// - 传 seed：传完整的 `seed:...` 行，而不是裸 seed 值
    ///
    /// 例如：
    ///
    /// - `&[]`
    /// - `&["seed:33554431@!".to_string()]`
    ///
    /// 不建议传：
    ///
    /// - `&["33554431@!".to_string()]`
    ///
    /// 因为那样与 raw 文本中的 seed 语义不一致。
    ///
    /// 典型用法：
    ///
    /// 1. 先调用 [`Runner::prepare_groups`] / [`Runner::prepare_groups_with_eval_rq`]
    /// 2. 再在循环中多次调用本函数构造不同 seed 的 [`Runner`]
    /// 3. 对每个 `Runner` 调用 `run_to_completion()` 或逐回合推进
    pub fn new_from_prepared_with_seed(prepared: &PreparedRunner, seed: &[String]) -> RunnerResult<Runner> {
        Self::new_from_prepared_groups_with_seed(prepared.template.as_ref(), seed)
    }

    #[inline]
    fn push_rc4_key_part(out: &mut String, part: &str) {
        if !out.is_empty() {
            out.push('\r');
        }
        out.push_str(part);
    }

    fn rc4_key_with_seed(prepared: &PreparedRunnerTemplate, seed: &[String]) -> String {
        if seed.is_empty() {
            return prepared.base_key.clone();
        }

        if seed.len() == 1 {
            let seed_item = &seed[0];
            let Err(insert_at) = prepared.base_names_sorted.binary_search(seed_item) else {
                return prepared.base_key.clone();
            };
            let mut key = String::with_capacity(prepared.base_key.len() + seed_item.len() + 1);
            for name in &prepared.base_names_sorted[..insert_at] {
                Self::push_rc4_key_part(&mut key, name);
            }
            Self::push_rc4_key_part(&mut key, seed_item);
            for name in &prepared.base_names_sorted[insert_at..] {
                Self::push_rc4_key_part(&mut key, name);
            }
            return key;
        }

        let mut seed_items = seed.iter().collect::<Vec<_>>();
        seed_items.sort_unstable();
        seed_items.dedup();

        let mut key =
            String::with_capacity(prepared.base_key.len() + seed_items.iter().map(|item| item.len() + 1).sum::<usize>());
        let mut base_idx = 0usize;
        let mut seed_idx = 0usize;
        while base_idx < prepared.base_names_sorted.len() || seed_idx < seed_items.len() {
            match (prepared.base_names_sorted.get(base_idx), seed_items.get(seed_idx)) {
                (Some(base), Some(seed_item)) => match base.cmp(seed_item) {
                    std::cmp::Ordering::Less => {
                        Self::push_rc4_key_part(&mut key, base);
                        base_idx += 1;
                    }
                    std::cmp::Ordering::Equal => {
                        Self::push_rc4_key_part(&mut key, base);
                        base_idx += 1;
                        seed_idx += 1;
                    }
                    std::cmp::Ordering::Greater => {
                        Self::push_rc4_key_part(&mut key, seed_item);
                        seed_idx += 1;
                    }
                },
                (Some(base), None) => {
                    Self::push_rc4_key_part(&mut key, base);
                    base_idx += 1;
                }
                (None, Some(seed_item)) => {
                    Self::push_rc4_key_part(&mut key, seed_item);
                    seed_idx += 1;
                }
                (None, None) => break,
            }
        }
        key
    }

    fn cmp_prepared_player_for_sort(storage: &Arc<Storage>, id_key_names: &[String], a: PlrId, b: PlrId) -> std::cmp::Ordering {
        let plr_a = storage.get_player(&a).expect("plr not found when sort prepared player");
        let plr_b = storage.get_player(&b).expect("plr not found when sort prepared player");
        match plr_a.sort_int.cmp(&plr_b.sort_int) {
            std::cmp::Ordering::Equal => match id_key_names[a].cmp(&id_key_names[b]) {
                std::cmp::Ordering::Equal => plr_a.id().cmp(&plr_b.id()),
                ord => ord,
            },
            ord => ord,
        }
    }

    fn build_prepared_groups(players: &[Vec<String>], eval_rq: f64) -> RunnerResult<PreparedRunnerTemplate> {
        let mut base_names_sorted = players
            .iter()
            .flatten()
            .filter(|str| !Player::check_is_seed(str))
            .map(|str| Player::raw_namerena_to_idname(str))
            .collect::<Vec<String>>();
        base_names_sorted.sort();
        base_names_sorted.dedup();

        let storage = Storage::new_arc_with_eval_rq(eval_rq);

        // 先完成玩家实例化与分组（跳过 seed 行），与正常初始化路径保持一致。
        let mut inited_plrs: Vec<Vec<PlrId>> = Vec::with_capacity(players.len());
        for plrs in players {
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
                    }
                }
            }
        }

        // 与 Dart 对齐：按 id_name 排序后逐个 build。
        let mut sorted_plrs = inited_plrs.iter().flatten().copied().collect::<Vec<PlrId>>();
        sorted_plrs.sort_by(|a, b| {
            let plr_a = storage.get_player(a).expect("plr not found when sorted build");
            let plr_b = storage.get_player(b).expect("plr not found when sorted build");
            plr_a.cmp_by_id_name(plr_b)
        });
        for ptr in sorted_plrs {
            let plr = storage.just_get_player_mut(ptr).expect("plr not found when build");
            plr.build();
            if plr.player_type() == crate::player::PlayerType::Boss {
                crate::player::boss::init_boss_state(plr);
            }
        }

        // 固化成可复用模板（深拷贝 Player）。
        let mut prepared_groups = Vec::with_capacity(inited_plrs.len());
        let total_players = inited_plrs.iter().map(Vec::len).sum::<usize>();
        let mut id_key_names = vec![String::new(); total_players];
        for group in inited_plrs {
            let mut prepared_group = Vec::with_capacity(group.len());
            for ptr in group {
                let plr = storage.get_player(&ptr).expect("prepared player not found");
                id_key_names[ptr] = plr.id_key_name();
                prepared_group.push(plr.clone());
            }
            prepared_groups.push(prepared_group);
        }
        let mut sorted_by_id_name = id_key_names
            .iter()
            .enumerate()
            .filter_map(|(id, name)| (!name.is_empty()).then_some(id))
            .collect::<Vec<PlrId>>();
        sorted_by_id_name.sort_by(|a, b| id_key_names[*a].cmp(&id_key_names[*b]));
        let base_key = base_names_sorted.join("\r");
        Ok(PreparedRunnerTemplate {
            groups: prepared_groups,
            base_names_sorted,
            base_key,
            id_key_names,
            sorted_by_id_name,
            eval_rq,
        })
    }

    fn new_from_prepared_groups_with_seed(prepared: &PreparedRunnerTemplate, seed: &[String]) -> RunnerResult<Runner> {
        // 原始逻辑：
        // 把名称排序去重后 join "\r"，再作为 RC4 key。
        let keys = Self::rc4_key_with_seed(prepared, seed);
        let mut randomer = RC4::new(keys.as_bytes(), 1);
        randomer.js_xor_str(&keys);

        let storage = Storage::new_arc_with_eval_rq(prepared.eval_rq);
        let total_players = prepared.groups.iter().map(|group| group.len()).sum::<usize>();
        for _ in 0..total_players {
            let _ = storage.new_plr_id();
        }

        let mut inited_plrs: Vec<Vec<PlrId>> = Vec::with_capacity(prepared.groups.len());
        for group in &prepared.groups {
            let mut copied_group = Vec::with_capacity(group.len());
            for player in group {
                let ptr = storage.just_insert_player(player.clone());
                copied_group.push(ptr);
            }
            if !copied_group.is_empty() {
                inited_plrs.push(copied_group);
            }
        }

        // 与 Dart 对齐：按 id_name 排序后初始化 sort_int（依赖 seed）。
        for &ptr in &prepared.sorted_by_id_name {
            let plr = storage.just_get_player_mut(ptr).expect("plr not found when set sort_int");
            plr.sort_int = randomer.rFFFFFF() as i32;
        }

        for group in &mut inited_plrs {
            group.sort_by(|a, b| Self::cmp_prepared_player_for_sort(&storage, &prepared.id_key_names, *a, *b));
        }

        let input_groups = inited_plrs.clone();

        inited_plrs.sort_by(|a, b| {
            let Some(first_a) = a.first() else {
                return std::cmp::Ordering::Less;
            };
            let Some(first_b) = b.first() else {
                return std::cmp::Ordering::Greater;
            };
            Self::cmp_prepared_player_for_sort(&storage, &prepared.id_key_names, *first_a, *first_b)
        });

        // 保持旧版随机流消费顺序，避免战斗回放偏移。
        for group in &inited_plrs {
            for plr in group {
                randomer.encrypt_bytes_no_change(&prepared.id_key_names[*plr]);
            }
            randomer.encrypt_bytes(&mut [0]);
        }

        let mut sorted_for_move_point = inited_plrs.iter().flatten().copied().collect::<Vec<PlrId>>();
        sorted_for_move_point.sort_by(|a, b| Self::cmp_prepared_player_for_sort(&storage, &prepared.id_key_names, *a, *b));
        for ptr in &sorted_for_move_point {
            let plr = storage.just_get_player_mut(*ptr).expect("plr not found when set move point");
            plr.set_move_point(randomer.r255() as i32);
        }

        let mut world = WorldState::new(inited_plrs);
        world.players = sorted_for_move_point;
        storage.sync_groups(&world.groups);
        storage.sync_alive_groups_owned_with_count(world.alives_by_group(&storage), world.alive_group_count());

        // 对初始即为死亡状态的玩家（如 Seed 类型）补充 record_death，
        // 保证 sync_runtime_entities 快速路径不会遗漏它们，第一次 tick 就能正常清除。
        for id in world.all_plrs() {
            if !storage.get_player(&id).map(|p| p.alive()).unwrap_or(true) {
                storage.record_death(id);
            }
        }

        if world.roster_count() == 1 {
            world.winner = world.winner_roster(0);
        }

        Ok(Runner {
            randomer,
            storage,
            world,
            input_groups,
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
    pub fn split_namerena_into_groups(raw_input: String) -> RawPlayers {
        // 处理 `\r\n`。
        let raw_input = raw_input.replace("\r\n", "\n");
        // 处理 `\r`。
        let raw_input = raw_input.replace("\r", "\n");

        let mut lines = raw_input.split('\n').map(trim_js_line_end).map(str::to_string).collect::<Vec<String>>();

        while lines.last().is_some_and(|line| line.is_empty()) {
            lines.pop();
        }

        let seed = lines.iter().filter(|x| Player::check_is_seed(x)).cloned().collect::<Vec<String>>();

        // 没有空行分组：一行一个队伍（旧规则）。
        if !lines.iter().any(|line| line.is_empty()) {
            return (lines.into_iter().map(|x| vec![x]).collect(), seed);
        }

        let mut raw_groups: Vec<Vec<String>> = Vec::new();
        let mut current_group: Vec<String> = Vec::new();
        for line in lines {
            if line.is_empty() {
                if !current_group.is_empty() {
                    raw_groups.push(std::mem::take(&mut current_group));
                }
                continue;
            }
            current_group.push(line);
        }
        if !current_group.is_empty() {
            raw_groups.push(current_group);
        }

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
        (groups, seed)
    }

    #[inline]
    pub fn have_winner(&self) -> bool { self.world.have_winner() }

    #[inline]
    pub fn all_plrs(&self) -> Vec<PlrId> { self.world.all_plrs() }

    #[inline]
    pub fn all_plr_len(&self) -> usize { self.world.all_plr_len() }

    pub fn main_round(&mut self) -> RunUpdates { self.core.main_round(&mut self.world, &self.storage, &mut self.randomer) }

    /// 直接跑完整场战斗。
    ///
    /// 这里复用 `main_round()`，保持与普通 fight CLI 一致的空回合判定语义，
    /// 避免高速路径与正常对局出现不同胜者。
    pub fn run_to_completion(&mut self) -> bool {
        let mut idle = 0usize;
        let mut rounds = 0usize;
        let mut updates = RunUpdates::new_no_capture();
        while !self.world.have_winner() && idle <= 16 && rounds < 100_000 {
            updates.reset();
            self.core
                .main_round_into(&mut self.world, &self.storage, &mut self.randomer, &mut updates);
            if !updates.had_updates() {
                idle += 1;
            } else {
                idle = 0;
            }
            rounds += 1;
        }
        self.world.have_winner()
    }

    pub fn round_tick(&mut self, updates: &mut RunUpdates) {
        self.core.tick(&mut self.world, &self.storage, &mut self.randomer, updates);
    }
}

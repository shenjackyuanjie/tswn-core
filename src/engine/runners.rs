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

use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::RunUpdates;
use crate::error::runner::RunnerResult;
use crate::player::{Player, PlrId};
use crate::rc4::RC4;

/// 一组玩家的集合类型，供内部初始化使用。
pub type PlayerGroup = Vec<Player>;
/// 原始输入解析结果：(队伍列表, 种子行列表)。
pub type RawPlayers = (Vec<Vec<String>>, Vec<String>);

use crate::engine::{engine_core::EngineCore, world_state::WorldState};

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
        let (players, seed) = Runner::split_namerena_into_groups(raw_input);

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
            if plr.player_type() == crate::player::PlayerType::Boss {
                crate::player::boss::init_boss_state(plr);
            }
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
    pub fn split_namerena_into_groups(raw_input: String) -> RawPlayers {
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

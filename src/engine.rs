/// namerena 评分机制里的第一个靶子
pub const PROFILE_START: u32 = 33554431;

/// 由 Bevy Ecs 启发的一个简单的 ECS 系统
/// 主要用来存储 Skill (玩家技能), State (玩家状态)
/// 有可能后面会一块干脆把 Player 也放进来
pub mod storage {

    use crate::player::Player;

    use slotmap::{new_key_type, SlotMap};

    new_key_type! {
        /// 稳定的玩家句柄（Arena Key）
        pub struct PlrId;
    }

    /// 存数据的地方
    ///
    /// 单线程 Arena：使用 slotmap 提供稳定 key（不会随容器移动而失效）
    #[derive(Debug)]
    pub struct Storage {
        /// 玩家
        players: SlotMap<PlrId, Player>,
    }

    impl Storage {
        /// 创建一个新的 Storages
        pub fn new() -> Storage {
            Storage {
                players: SlotMap::with_key(),
            }
        }

        pub fn new_arc() -> std::sync::Arc<Self> { std::sync::Arc::new(Self::new()) }

        pub fn clear(&mut self) {
            self.players.clear();
        }

        /// 获取玩家
        pub fn get_player(&self, id: PlrId) -> Option<&Player> { self.players.get(id) }
        pub fn get_player_mut(&mut self, id: PlrId) -> Option<&mut Player> { self.players.get_mut(id) }

        /// 同时可变借用两个不同玩家（用于升级/对战等）
        ///
        /// 只有当两个 id 都存在且不相等时才返回 Some。
        pub fn get_two_players_mut(&mut self, a: PlrId, b: PlrId) -> Option<(&mut Player, &mut Player)> {
            self.players.get_disjoint_mut([a, b]).map(|[pa, pb]| (pa, pb))
        }

        pub fn insert_player(&mut self, player: Player) -> PlrId { self.players.insert(player) }
        pub fn remove_player(&mut self, id: PlrId) -> Option<Player> { self.players.remove(id) }
    }

    impl std::default::Default for Storage {
        fn default() -> Self { Self::new() }
    }
}

/// 事件队列：跨实体影响通过事件表达，Runner 统一 drain 处理，避免借用冲突。
pub mod event {
    use std::collections::VecDeque;

    use crate::engine::storage::PlrId;

    #[derive(Debug, Clone)]
    pub enum Event {
        /// 玩家想要攻击（由 Runner 决定目标/执行）
        TryAttack { caster: PlrId },
        /// 具体攻击一次
        Attack { caster: PlrId, target: PlrId, is_mag: bool },
        /// 直接造成（或回复）伤害
        DealDamage { caster: PlrId, target: PlrId, dmg: i32 },
    }

    /// 事件队列：跨实体影响通过事件表达，Runner 统一 drain 处理，避免借用冲突。
    #[derive(Debug, Default)]
    pub struct EventQueue {
        q: VecDeque<Event>,
    }

    impl EventQueue {
        pub fn new() -> Self { Self { q: VecDeque::new() } }

        #[inline]
        pub fn push(&mut self, ev: Event) { self.q.push_back(ev) }

        #[inline]
        pub fn pop(&mut self) -> Option<Event> { self.q.pop_front() }

        #[inline]
        pub fn is_empty(&self) -> bool { self.q.is_empty() }

        #[inline]
        pub fn len(&self) -> usize { self.q.len() }

        pub fn clear(&mut self) { self.q.clear() }
    }
}

/// 核心的游戏逻辑
pub mod runners {

    use crate::engine::update::RunUpdates;
    use crate::error::runner::RunnerResult;
    use crate::engine::event::{Event, EventQueue};
    use crate::engine::storage::PlrId;
    use crate::player::Player;
    use crate::player::on_damage_default;
    use crate::rc4::RC4;

    use super::storage::Storage;

    pub type PlayerGroup = Vec<Player>;

    pub struct Runner {
        /// 应该是一个 Rc4 实例类似物
        pub randomer: RC4,
        /// 所有玩家 (包括 boss)
        pub players: Vec<Vec<PlrId>>,
        /// 赢家
        ///
        /// 也应该是一个队伍
        pub winner: Option<Vec<PlrId>>,
        /// 该哪个玩家动了
        round_pos: i32,
        /// 存储所有 state 和 skill 的地方
        pub storage: Storage,

        /// 跨实体事件队列（技能/武器/状态改动等通过事件表达）
        pub events: EventQueue,
    }

    pub type RawPlayers = (Vec<Vec<String>>, Vec<String>);

    impl Runner {
        /// 从一个 名竞的原始输入 中创建一个 Runner
        ///
        /// 其实就是解析名竞的输入格式
        pub fn new_from_namerena_raw(raw_input: String) -> RunnerResult<Runner> {
            // 根据原始输入解析队伍

            // 原始逻辑:
            // 把所有\n去掉
            // 然后 join "\r"
            // 然后 utf8 encode
            // 然后用于生成这个 Randomer
            let (players, seed) = Runner::spilt_namerena_into_groups(raw_input);

            // 检查 seed 数量，一个游戏最多只能有 1 个 seed
            if seed.len() > 1 {
                return Err(crate::error::runner::RunnerError::TooManySeeds(seed.len()));
            }

            let mut names = players
                .iter()
                .flatten()
                .filter(|str| !Player::check_is_seed(str))
                .map(|str| Player::raw_namerena_to_idname(str))
                .chain(seed.iter().cloned())
                .collect::<Vec<String>>();
            // 这里顺便把 sorted hash 这块做了
            names.sort();
            names.dedup();
            // println!("{:?}", names);
            let keys = names.join("\r");
            // println!("{:?}", keys.as_bytes().to_vec());
            let mut randomer = RC4::new(keys.as_bytes(), 1);
            randomer.js_xor_str(&keys);
            // 准备好了
            // 用 randmoer 初始化玩家的 sort_int

            let mut storage = Storage::new();

            let mut inited_plrs = Vec::with_capacity(players.len());
            for plrs in players.iter() {
                let mut group = Vec::with_capacity(plrs.len());
                for plr in plrs.iter() {
                    let mut player = Player::new_from_namerena_raw(plr.to_string())?;
                    #[cfg(not(test))]
                    {
                        player.core_build.sort_int = randomer.rFFFFFF() as i32;
                    }
                    #[cfg(test)]
                    {
                        println!("randomer: {:?}", randomer);
                        let int_a = randomer.next_u8();
                        let int_b = randomer.next_u8();
                        let int_c = randomer.next_u8();
                        println!(
                            "{} {} {} {}, {}",
                            int_a,
                            int_b,
                            int_c,
                            ((int_a as u32) << 16) | ((int_b as u32) << 8) | (int_c as u32),
                            player.display_name() // 你好，shenjack
                        );
                        let int = ((int_a as u32) << 16) | ((int_b as u32) << 8) | (int_c as u32);
                        player.core_build.sort_int = int as i32;
                    }
                    let ptr = storage.insert_player(player);
                    // 如果有问题，就直接返回错误
                    // 不过大概率不会有问题就是了
                    group.push(ptr);
                }
                inited_plrs.push(group);
            }

            // 同队升级（ID 驱动，避免同时借用整个 storage）
            for group in inited_plrs.iter() {
                if group.len() < 2 {
                    continue;
                }
                // 先按 sort 比较稳定排序（读借用）
                let mut sorted = group.clone();
                sorted.sort_by(|a, b| {
                    let plr_a = storage.get_player(*a).expect("plr not found when sort");
                    let plr_b = storage.get_player(*b).expect("plr not found when sort");
                    plr_a.partial_cmp(plr_b).unwrap_or(std::cmp::Ordering::Equal)
                });
                for i in 0..sorted.len() {
                    for j in (i + 1)..sorted.len() {
                        let a = sorted[i];
                        let b = sorted[j];
                        let (plr_a_clan, plr_b_clan) = {
                            let pa = storage.get_player(a).unwrap();
                            let pb = storage.get_player(b).unwrap();
                            (pa.clan_name(), pb.clan_name())
                        };
                        if plr_a_clan == plr_b_clan {
                            let (pa, pb) = storage.get_two_players_mut(a, b).expect("plr not found");
                            pa.upgrade(pb);
                            pb.upgrade(pa);
                        }
                    }
                }
            }

            for group in inited_plrs.iter() {
                for plr_id in group.iter() {
                    let plr = storage.get_player_mut(*plr_id).expect("plr not found when build");
                    plr.build();
                }
            }

            let mut sort_groups = inited_plrs.iter().collect::<Vec<&Vec<PlrId>>>();
            sort_groups.sort_by(|a, b| {
                {
                    let plr_a = storage.get_player(a[0]).expect("plr not found when sort");
                    let plr_b = storage.get_player(b[0]).expect("plr not found when sort");
                    plr_a.partial_cmp(plr_b)
                }
                .unwrap_or(std::cmp::Ordering::Equal)
            });

            for group in sort_groups.iter() {
                for plr in group.iter() {
                    let plr_ref = storage.get_player(*plr).expect("plr not found when enc");
                    randomer.encrypt_bytes_no_change(&plr_ref.id_name());
                }
                randomer.encrypt_bytes(&mut [0]);
            }

            for group in inited_plrs.iter_mut() {
                for plr in group.iter_mut() {
                    let plr_ref = storage.get_player_mut(*plr).expect("plr not found when enc");
                    plr_ref.set_move_point(randomer.r255() as i32);
                }
            }

            let winner = if inited_plrs.len() == 1 {
                Some(inited_plrs[0].clone())
            } else {
                None
            };
            Ok(Runner {
                randomer,
                players: inited_plrs,
                winner,
                round_pos: -1,
                storage,
                events: EventQueue::new(),
            })
        }

        /// 获取所有存活的玩家
        pub fn alives_flat(&self) -> Vec<PlrId> { self.alives().iter().flatten().cloned().collect() }

        /// 以组为单位获取所有存活的玩家
        pub fn alives(&self) -> Vec<Vec<PlrId>> {
            self.players
                .iter()
                .map(|x| {
                    x.iter()
                        .filter(|x| {
                            let x = self.storage.get_player(**x).expect("plr not found when getting alive");
                            x.get_status().alive()
                        })
                        .cloned()
                        .collect()
                })
                .collect()
        }

        /// 将原始输入分拆成队伍
        /// # 说明
        ///
        /// ## 特殊情况处理
        /// - 去除尾部的一个/多个 \n/带有几个空格
        /// - 将 \r\n 替换成 \n
        /// - 将 \r 替换成 \n
        /// - 将 大于等于3个 \n 替换成 2个 \n
        ///
        /// 返回: (队伍, seed)
        #[allow(clippy::needless_return)]
        pub fn spilt_namerena_into_groups(raw_input: String) -> RawPlayers {
            // 去除尾部的一个/多个 \n/带有几个空格的情况
            let raw_input = raw_input.trim_end();
            // 处理一下有 \r\n 的情况
            let raw_input = raw_input.replace("\r\n", "\n");
            // 处理一下 \r 的情况
            let mut raw_input = raw_input.replace("\r", "\n");
            // 处理一下 \n\n\n
            while raw_input.contains("\n\n\n") {
                raw_input = raw_input.replace("\n\n\n", "\n\n");
            }
            // 先把 SEED_PREFIX 取出来
            let seed = raw_input
                .split("\n")
                .filter(|x| Player::check_is_seed(x))
                .map(|x| x.to_string())
                .collect::<Vec<String>>();
            // 首先，如果没有\n\n, 那么一行就是一个队伍
            if !raw_input.contains("\n\n") {
                return (raw_input.split("\n").map(|x| vec![x.to_string()]).collect(), seed);
            }
            let mut raw_groups: Vec<Vec<String>> = raw_input
                .split("\n\n")
                .map(|x| x.split('\n').map(|x| x.to_string()).collect())
                .filter(|g: &Vec<String>| !g.is_empty())
                .collect();

            // 修复: 将单独的 seed 队伍合并到相邻的非 seed 队伍
            // 场景: "aaaa\nbbbb\n\nseed: a@!" 或 "seed: a@!\n\naaaa\nbbbb"
            let mut i = 0;
            while i < raw_groups.len() {
                // 检查当前队伍是否是纯 seed 队伍
                let is_seed_only = raw_groups[i].len() == 1
                    && !raw_groups[i].is_empty()
                    && Player::check_is_seed(&raw_groups[i][0]);

                if is_seed_only {
                    let seed_player = raw_groups[i][0].clone();

                    if i > 0 {
                        // 有前一个队伍，合并到前一个队伍
                        raw_groups[i - 1].push(seed_player);
                        raw_groups.remove(i);
                        // 不递增 i，继续检查下一个
                    } else if i + 1 < raw_groups.len() {
                        // 是第一个队伍，合并到后一个队伍的开头
                        raw_groups[i + 1].insert(0, seed_player);
                        raw_groups.remove(i);
                        // 不递增 i，继续检查下一个
                    } else {
                        // 只有一个队伍且是 seed，保留
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }

            return (raw_groups, seed);
        }

        #[inline]
        pub fn have_winner(&self) -> bool { self.winner.is_some() }

        #[inline]
        pub fn all_plrs(&self) -> Vec<PlrId> { self.players.iter().flatten().cloned().collect() }

        #[inline]
        pub fn all_plr_len(&self) -> usize { self.players.iter().map(|x| x.len()).sum() }

        /// TODO: 实现 main_round 方法
        pub fn main_round(&mut self) { let _updates = RunUpdates::new(); }

        pub fn round_tick(&mut self, updates: &mut RunUpdates) {
            self.round_pos += 1;
            self.round_pos %= self.all_plr_len() as i32;

            let tick_plr_index = self.round_pos as usize;

            let mut all_plrs = self.players.iter().flatten().cloned().collect::<Vec<PlrId>>();

            // 获取当前 tick 的玩家
            if let Some(tick_plr) = all_plrs.get_mut(tick_plr_index) {
                // 调用 step 方法
                let tick_plr_ref = self.storage.get_player_mut(*tick_plr).expect("plr not found when tick");
                tick_plr_ref.step(*tick_plr, &mut self.randomer, updates, &mut self.events);

                self.process_events(updates);
            } else {
                unreachable!("tick_plr_index out of range");
            }
        }

        fn process_events(&mut self, updates: &mut RunUpdates) {
            // 防止错误技能导致无限事件循环
            const MAX_EVENTS_PER_TICK: usize = 1024;
            for _ in 0..MAX_EVENTS_PER_TICK {
                let Some(ev) = self.events.pop() else {
                    break;
                };

                match ev {
                    Event::TryAttack { caster } => {
                        let candidates: Vec<PlrId> = self
                            .all_plrs()
                            .into_iter()
                            .filter(|id| *id != caster)
                            .filter(|id| self.storage.get_player(*id).map(|p| p.alive()).unwrap_or(false))
                            .collect();

                        if candidates.is_empty() {
                            continue;
                        }

                        let idx = (self.randomer.rFFFFFF() as usize) % candidates.len();
                        let target = candidates[idx];
                        self.events.push(Event::Attack {
                            caster,
                            target,
                            is_mag: false,
                        });
                    }
                    Event::Attack { caster, target, is_mag } => {
                        let Some((caster_plr, target_plr)) = self.storage.get_two_players_mut(caster, target) else {
                            continue;
                        };
                        if !caster_plr.active() || !target_plr.alive() {
                            continue;
                        }

                        let caster_stats = caster_plr.caster_snapshot();
                        let atp = caster_plr.get_at(is_mag, &mut self.randomer);
                        target_plr.attacked(
                            target,
                            atp,
                            is_mag,
                            caster,
                            caster_stats,
                            on_damage_default,
                            &mut self.randomer,
                            updates,
                            &mut self.events,
                        );
                    }
                    Event::DealDamage { caster, target, dmg } => {
                        let Some(target_plr) = self.storage.get_player_mut(target) else {
                            continue;
                        };
                        target_plr.damage(target, dmg, caster, on_damage_default, &mut self.randomer, updates, &mut self.events);
                    }
                }
            }
        }
    }
}

pub mod update {

    use crate::engine::storage::PlrId;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum UpdateType {
        /// 赢!
        Win,
        /// 没动作
        None,
        /// 下一行
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
        // param: Object ?
    }

    impl RunUpdate {
        pub fn new_dummy() -> RunUpdate {
            RunUpdate {
                score: 0,
                delay0: 0,
                delay1: 0,
                message: "\n".to_string(),
                caster: PlrId::default(),
                target: PlrId::default(),
                targets: vec![],
                update_type: UpdateType::None,
                // param: Object ?
            }
        }

        pub fn msg(&self) -> String {
            // [0] -> caster
            // [1] -> target
            // [2] -> targets
            let mut msg = self.message.clone();
            msg = msg.replace("[0]", &format!("{:?}", self.caster));
            msg = msg.replace("[1]", &format!("{:?}", self.target));
            msg = msg.replace(
                "[2]",
                &self
                    .targets
                    .iter()
                    .map(|x| format!("{:?}", x))
                    .collect::<Vec<String>>()
                    .join(","),
            );
            msg
        }

        pub fn new_newline() -> RunUpdate {
            RunUpdate {
                score: 0,
                delay0: 0,
                delay1: 0,
                message: "\n".to_string(),
                caster: PlrId::default(),
                target: PlrId::default(),
                targets: vec![],
                update_type: UpdateType::NextLine,
                // param: Object ?
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
        // post_updates: Vec<RunUpdate>,
    }

    impl RunUpdates {
        pub fn new() -> RunUpdates {
            RunUpdates {
                updates: vec![],
                // post_updates: vec![],
            }
        }

        pub fn add(&mut self, update: RunUpdate) { self.updates.push(update); }

        pub fn add_all(&mut self, updates: &mut [RunUpdate]) { self.updates.extend_from_slice(updates); }
    }
}

#[cfg(test)]
/// 酒吧点炒饭列表(确信)
mod group {
    use super::*;

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
                // 填充 x, 每一个都调用一遍 to_string
                $($x.to_string()),+
            ]
        );
    }

    // 自动把每一个分开填
    macro_rules! plrs {
        () => {
            str_vec!(str_vec!())
        };
        ($($x:expr),+ $(,)?) => (
            vec![
                $(vec![
                    // 填充 x, 每一个都调用一遍 to_string
                    $x.to_string()
                ],)+
            ]
        );
    }

    mod spilt_namerena_groups {
        use super::*;
        #[test]
        fn basic_spilt() {
            // 没有 \n\n 的最基本情况
            let raw_input = "a\nb\nc".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (plrs!("a", "b", "c"), plr!()));

            // 跟随着一个或者多个尾部 \n 的情况
            // 自动忽略
            let raw_input = "a\nb\nc\n".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (plrs!("a", "b", "c"), plr!()));
            let raw_input = "a\nb\nc\n\n".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (plrs!("a", "b", "c"), plr!()));
        }

        #[test]
        fn spilt_teams() {
            // 有 \n\n 的情况
            let raw_input = "a\nb\n\nc\nd".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (vec![plr!["a", "b"], plr!["c", "d"]], plr!()));
        }

        #[test]
        fn more_than_2_newline() {
            // 有多个 \n 的情况
            // 2个 \n 以上的情况，都会被替换成2个 \n
            for x in 2..10 {
                let new_lines = "\n".repeat(x);
                let raw_input = format!("a\nb{new_lines}c\nd");
                let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
                assert_eq!(groups, (vec![plr!["a", "b"], plr!["c", "d"]], plr!()));
            }
            // 以及有多个队伍的情况
            for x in 2..10 {
                let new_lines = "\n".repeat(x);
                let raw_input = format!("a\nb{new_lines}c\nd{new_lines}e");
                let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
                assert_eq!(groups, (vec![plr!["a", "b"], plr!["c", "d"], plr!["e"]], plr!()));
            }
        }

        #[test]
        fn lot_of_teams() {
            // 多个队伍
            let raw_input = "a\nb\nc\nd\ne\nf".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (plrs!("a", "b", "c", "d", "e", "f"), plr!()));
        }

        #[test]
        fn normal_seed() {
            // 一个seed
            let raw_input = "seed: a@!\nb\nc".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (plrs!("seed: a@!", "b", "c"), plr!["seed: a@!"]));
        }

        #[test]
        fn need_fix_seed1() {
            // seed 被错误地单独分成一个队伍（手误多打了空行）
            // 输入: "aaaa\nbbbb\n\nseed: a@!"
            // 修复后: seed 应该合并到前一个队伍
            let raw_input = "aaaa\nbbbb\n\nseed: a@!".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            // 期望: [["aaaa", "bbbb", "seed: a@!"]] (一个队伍)
            assert_eq!(groups, (vec![plr!("aaaa", "bbbb", "seed: a@!")], plr!["seed: a@!"]));
        }

        #[test]
        fn need_fix_seed2() {
            // 跟 test 1 顺序相反
            // 输入: "seed: a@!\n\naaaa\nbbbb"
            // 修复后: seed 应该合并到后一个队伍
            let raw_input = "seed: a@!\n\naaaa\nbbbb".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            // 期望: [["seed: a@!", "aaaa", "bbbb"]] (一个队伍)
            assert_eq!(groups, (vec![plr!("seed: a@!", "aaaa", "bbbb")], plr!["seed: a@!"]));
        }

        #[test]
        fn seed_in_middle_of_teams() {
            // seed 在多个队伍中间
            // 输入: "team1\n\nseed: a@!\n\nteam2"
            // 修复后: seed 合并到前一个队伍（优先合并到左边）
            let raw_input = "team1\n\nseed: a@!\n\nteam2".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            // 期望: [["team1", "seed: a@!"], ["team2"]]
            assert_eq!(groups, (vec![plr!("team1", "seed: a@!"), plr!("team2")], plr!["seed: a@!"]));
        }

        #[test]
        fn multiple_seeds_in_one_team() {
            // 多个 seed 在一个队伍里 → 应该返回 TooManySeeds 错误
            let raw_input = "seed: a@!\nseed: b@!\n\nnormal".to_string();
            let result = runners::Runner::new_from_namerena_raw(raw_input);
            assert!(matches!(result, Err(crate::error::runner::RunnerError::TooManySeeds(2))));
        }

        #[test]
        fn consecutive_seed_only_teams() {
            // 连续的纯 seed 队伍（2个 seed）→ 应该返回 TooManySeeds 错误
            let raw_input = "normal\n\nseed: a@!\n\nseed: b@!\n\nteam2".to_string();
            let result = runners::Runner::new_from_namerena_raw(raw_input);
            assert!(matches!(result, Err(crate::error::runner::RunnerError::TooManySeeds(2))));
        }

        #[test]
        fn only_seed_player() {
            // 只有 seed 玩家（特殊情况）
            let raw_input = "seed: a@!".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (plrs!("seed: a@!"), plr!["seed: a@!"]));
        }

        #[test]
        fn seed_mixed_with_normal_in_same_team() {
            // seed 和普通玩家在一个队伍（正常输入，无需修复）
            // 输入: "normal1\nnormal2\nseed: a@!"
            // 注意：这里只有一个 \n，所以是一个队伍
            let raw_input = "normal1\nnormal2\nseed: a@!".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (plrs!("normal1", "normal2", "seed: a@!"), plr!["seed: a@!"]));
        }

        #[test]
        fn multi_teams_with_seed_at_end() {
            // 多队伍场景，seed 在最后一个队伍（被错误地单独分组）
            // 输入: "a\nb\n\nc\nd\n\nseed: x@!"
            let raw_input = "a\nb\n\nc\nd\n\nseed: x@!".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            // seed 合并到最后一个普通队伍
            assert_eq!(groups, (vec![plr!("a", "b"), plr!("c", "d", "seed: x@!")], plr!["seed: x@!"]));
        }

        #[test]
        fn empty_lines_between_normal_players() {
            // 普通玩家之间有多个空行（不应该被合并）
            // 输入: "a\n\n\nb\n\n\nc"
            // 应该变成两个队伍：[a], [b], [c]
            let raw_input = "a\n\n\nb\n\n\nc".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (vec![plr!("a"), plr!("b"), plr!("c")], plr!()));
        }

        #[test]
        fn seed_with_team_name() {
            // seed 玩家带有队伍名
            // 注意："seed: a@!red" 是以 "seed:" 开头的，所以被认为是 seed 玩家
            // 修复逻辑会将其合并到前一个队伍
            let raw_input = "aaa\n\nseed: a@!red".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            // 虽然是 seed: a@!red，但仍然以 seed: 开头，所以合并到 aaa
            assert_eq!(groups, (vec![plr!("aaa", "seed: a@!red")], plr!["seed: a@!red"]));
        }

        #[test]
        fn only_seeds_multiple_teams() {
            // 全是 seed 的多个队伍（3个 seed）→ 应该返回 TooManySeeds 错误
            let raw_input = "seed: a@!\n\nseed: b@!\n\nseed: c@!".to_string();
            let result = runners::Runner::new_from_namerena_raw(raw_input);
            assert!(matches!(result, Err(crate::error::runner::RunnerError::TooManySeeds(3))));
        }

        #[test]
        fn too_many_seeds_error() {
            // 显式测试多 seed 返回错误
            let raw_input = "seed: a@!\nseed: b@!\nseed: c@!\nseed: d@!".to_string();
            let result = runners::Runner::new_from_namerena_raw(raw_input);
            assert!(matches!(result, Err(crate::error::runner::RunnerError::TooManySeeds(4))));
        }
    }

    mod runner {
        use super::*;

        #[test]
        fn sort_int_test() {
            let raw_input = "aaa\nbbb\nseed: aaaa@!";
            let runner = runners::Runner::new_from_namerena_raw(raw_input.to_string()).unwrap();

            let ints = [16391432, 11292362];
            assert!(!runner.have_winner());

            for (i, plr) in runner
                .players
                .iter()
                .flatten()
                .filter(|plr| runner.storage.get_player(**plr).expect("wtf").is_seed_plr())
                .enumerate()
            {
                let plr = runner.storage.get_player(*plr).expect("plr not found");
                assert_eq!(plr.core_build.sort_int as u32, { ints[i] });
            }
        }

        #[test]
        fn sort_int_test2() {
            let raw_input = "aaa\nbbb";
            let runner = runners::Runner::new_from_namerena_raw(raw_input.to_string()).unwrap();

            let ints = [7525315, 8712372];
            assert!(!runner.have_winner());

            for (i, plr) in runner.players.iter().flatten().enumerate() {
                let plr = runner.storage.get_player(*plr).expect("plr not found");
                println!("plr: {}", plr.display_name());
                assert_eq!(plr.core_build.sort_int as u32, { ints[i] });
            }
        }
    }
}

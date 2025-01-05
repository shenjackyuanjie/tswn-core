pub const PROFILE_START: u32 = 33554431;

/// 由 Bevy Ecs 启发的一个简单的 ECS 系统
/// 主要用来存储 Skill (玩家技能), State (玩家状态)
/// 有可能后面会一块干脆把 Player 也放进来
pub mod storage {

    use std::sync::Arc;

    use crate::player::skill_state::Skill;
    use crate::player::{Player, PlrPtr};

    use foldhash::HashMap as FastHashMap;

    /// 技能的 ID (ECS内的)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SkillId(usize);

    impl SkillId {
        pub fn new(id: usize) -> SkillId { SkillId(id) }

        /// 根据一个 Skill 实例创建一个 SkillId
        /// (其实就是取个内存地址)
        pub fn new_from_skill(skill: &Skill) -> SkillId {
            SkillId({
                let ptr = skill as *const Skill;
                ptr as usize
            })
        }
    }

    /// 存数据的地方
    ///
    /// 使用了 foldhash 的 HashMap, 快一些
    #[derive(Debug)]
    pub struct Storage {
        /// 存技能
        /// usize: memory address
        skills: FastHashMap<usize, Skill>,
        /// 存玩家组?
        groups: FastHashMap<usize, Vec<PlrPtr>>,
        /// 玩家
        players: FastHashMap<PlrPtr, Player>,
    }

    impl Storage {
        /// 创建一个新的 Storages
        pub fn new() -> Storage {
            Storage {
                skills: FastHashMap::default(),
                groups: FastHashMap::default(),
                players: FastHashMap::default(),
            }
        }

        pub fn new_arc() -> Arc<Self> { Arc::new(Self::new()) }

        pub fn clear(&mut self) {
            self.skills.clear();
            self.groups.clear();
            self.players.clear();
        }

        /// 获取技能
        pub fn get_skill(&self, id: SkillId) -> Option<&Skill> { self.skills.get(&id.0) }
        pub fn get_player(&self, ptr: PlrPtr) -> Option<&Player> { self.players.get(&ptr) }

        /// 获取技能的可变引用
        pub fn get_skill_mut(&mut self, id: SkillId) -> Option<&mut Skill> { self.skills.get_mut(&id.0) }
        /// 我就硬获取了
        /// 不过这个方法不安全
        pub fn just_get_skill_mut(&self, id: SkillId) -> Option<&mut Skill> {
            unsafe {
                let mut_slf = self as *const Storage as *mut Storage;
                (*mut_slf).skills.get_mut(&id.0)
            }
        }

        pub fn just_get_player_mut(&self, ptr: PlrPtr) -> Option<&mut Player> {
            unsafe {
                let mut_slf = self as *const Storage as *mut Storage;
                (*mut_slf).players.get_mut(&ptr)
            }
        }

        /// 插入技能 (返回技能的 ID)
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

        pub fn insert_player(&mut self, player: Player) -> PlrPtr {
            let ptr = player.as_ptr();
            self.players.insert(ptr, player);
            ptr
        }
        pub fn just_insert_player(&self, player: Player) -> PlrPtr {
            unsafe {
                let mut_slf = self as *const Storage as *mut Storage;
                let ptr = player.as_ptr();
                (*mut_slf).players.insert(ptr, player);
                ptr
            }
        }

        /// 删除技能
        pub fn remove_skill(&mut self, id: SkillId) -> Option<Skill> { self.skills.remove(&id.0) }
        pub fn just_remove_skill(&self, id: SkillId) -> Option<Skill> {
            unsafe {
                let mut_slf = self as *const Storage as *mut Storage;
                (*mut_slf).skills.remove(&id.0)
            }
        }

        pub fn just_remove_player(&self, ptr: PlrPtr) -> Option<Player> {
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

/// 核心的游戏逻辑
pub mod runners {

    use std::sync::Arc;

    use crate::engine::update::RunUpdates;
    use crate::error::runner::RunnerResult;
    use crate::player::{Player, PlrPtr};
    use crate::rc4::RC4;

    use super::storage::Storage;

    pub type PlayerGroup = Vec<Player>;

    pub struct Runner {
        /// 应该是一个 Rc4 实例类似物
        pub randomer: RC4,
        /// 所有玩家 (包括 boss)
        pub players: Vec<Vec<PlrPtr>>,
        /// 赢家
        ///
        /// 也应该是一个队伍
        pub winner: Option<Vec<PlrPtr>>,
        /// 该哪个玩家动了
        round_pos: i32,
        /// 存储所有 state 和 skill 的地方
        pub storage: Arc<Storage>,
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
            println!("{:?}", names);
            let keys = names.join("\r");
            println!("{:?}", keys.as_bytes().to_vec());
            let mut randomer = RC4::new(keys.as_bytes(), 1);
            randomer.encrypt_bytes_no_change(&keys);
            // 准备好了
            // 用 randmoer 初始化玩家的 sort_int

            let mut storage = Storage::new_arc();

            let mut inited_plrs = Vec::with_capacity(players.len());
            for plrs in players.iter() {
                let mut group = Vec::with_capacity(plrs.len());
                for plr in plrs.iter() {
                    let mut player = Player::new_from_namerena_raw(plr.to_string(), storage.clone())?;
                    player.sort_int = randomer.rFFFFFF() as i32;
                    let ptr = storage.just_insert_player(player);
                    // 如果有问题，就直接返回错误
                    // 不过大概率不会有问题就是了
                    group.push(ptr);
                }
                inited_plrs.push(group);
            }

            // 同队升级
            for plr_group in inited_plrs.iter_mut() {
                if plr_group.len() < 2 {
                    continue;
                }
                plr_group.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                for i in 0..plr_group.len() {
                    let (left, right) = plr_group.split_at_mut(i + 1);
                    let plr_p = &mut left[i];
                    for plr_q in right.iter_mut() {
                        let plr_p = storage.just_get_player_mut(*plr_p).expect("plr not found when upgrade");
                        let plr_q = storage.just_get_player_mut(*plr_q).expect("plr not found when upgrade");
                        if plr_p.clan_name() == plr_q.clan_name() {
                            plr_p.upgrade(plr_q);
                            plr_q.upgrade(plr_p);
                        }
                    }
                }
            }

            for group in inited_plrs.iter_mut() {
                for plr in group.iter_mut() {
                    let plr = storage.just_get_player_mut(*plr).expect("plr not found when building");
                    plr.build();
                }
            }

            let mut sort_groups = inited_plrs.iter().collect::<Vec<&Vec<PlrPtr>>>();
            sort_groups.sort_by(|a, b| {{
                let plr_a = storage.get_player(a[0]).expect("plr not found when sort");
                let plr_b = storage.get_player(b[0]).expect("plr not found when sort");
                plr_a.partial_cmp(plr_b)
            }.unwrap_or(std::cmp::Ordering::Equal)});

            for group in sort_groups.iter() {
                for plr in group.iter() {
                    let plr = storage.just_get_player_mut(*plr).expect("plr not found when enc");
                    randomer.encrypt_bytes_no_change(&plr.id_name());
                }
                randomer.encrypt_bytes(&mut [0]);
            }

            for group in inited_plrs.iter_mut() {
                for plr in group.iter_mut() {
                    let plr = storage.just_get_player_mut(*plr).expect("plr not found when enc");
                    plr.set_move_point(randomer.r255() as i32);
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
            })
        }

        /// 获取所有存活的玩家
        pub fn alives_flat(&self) -> Vec<PlrPtr> { self.alives().iter().flatten().cloned().collect() }

        /// 以组为单位获取所有存活的玩家
        pub fn alives(&self) -> Vec<Vec<PlrPtr>> {
            self.players
                .iter()
                .map(|x| x.iter().filter(|x| {
                    let x = self.storage.get_player(**x).expect("plr not found when getting alive");
                    x.get_status().alive()}).cloned().collect())
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
            let raw_groups: Vec<Vec<String>> =
                raw_input.split("\n\n").map(|x| x.split("\n").map(|x| x.to_string()).collect()).collect();

            // 修复是 TODO 项
            return (raw_groups, seed);
            // let raw_input = raw_input
            //     .split("\n")
            //     // .filter(|x| !Player::check_is_seed(x))
            //     .collect::<Vec<&str>>()
            //     .join("\n");
            // 如果有\n\n, 那么就是一个队伍

            // 下面是为了修复一些容易手误的情况
            // 比如
            // aaaaa
            // bbbb
            //
            // seed: xxx@!
            // 上面的情况中，按照规范, 应该把 seed: xxx@! 那一行和上面并起来
            // 但是很容易手误，多打一个回车
            // 导致这个seed: xxx@!成为一个队伍
            // aaaaa 和 bbbb 成为另一个队伍
            // 这里修复一下这个问题

            // 先检查有没有单独的seed玩家

            // let groups = raw_input
            //     .split("\n\n")
            //     .map(|x| x.split("\n").map(|x| x.to_string()).collect())
            //     .collect::<Vec<Vec<String>>>();

            // // 然后就是一些特判
            // // 比如双队伍, 同时其中一个是纯 seed
            // if raw_groups.len() == 2 {
            //     println!("need fix {:?}", raw_groups);
            //     // 双队伍特判
            //     // 队伍1是纯seed
            //     // 队伍2不是纯seed
            //     if raw_groups[0].len() == 1
            //         && Player::check_is_seed(raw_groups[0][0].as_str())
            //         && raw_groups[1].iter().all(|x| !Player::check_is_seed(x))
            //     {
            //         // 进行一个 fix
            //         // 也就是把那个非纯seed队伍分散成多个队伍
            //     }
            // }

            // (
            //     raw_input.split("\n\n").map(|x| x.split("\n").map(|x| x.to_string()).collect()).collect(),
            //     seed,
            // )
        }

        #[inline]
        pub fn have_winner(&self) -> bool { self.winner.is_some() }

        #[inline]
        pub fn all_plrs(&self) -> Vec<PlrPtr> { self.players.iter().flatten().cloned().collect() }

        #[inline]
        pub fn all_plr_len(&self) -> usize { self.players.iter().map(|x| x.len()).sum() }

        pub unsafe fn get_plr_by_ptr_unchecked(&self, ptr: PlrPtr) -> &Player {
            // 直接 unsafe 强转
            let ptr = ptr as *const Player;
            &*ptr
        }

        pub fn main_round(&mut self) { let updates = RunUpdates::new(); }

        pub fn round_tick(&mut self, updates: &mut RunUpdates) {
            self.round_pos += 1;
            self.round_pos %= self.all_plr_len() as i32;

            let tick_plr_index = self.round_pos as usize;

            let mut all_plrs = self.players.iter().flatten().cloned().collect::<Vec<PlrPtr>>();

            // 获取当前 tick 的玩家
            if let Some(tick_plr) = all_plrs.get_mut(tick_plr_index) {
                // 调用 step 方法
                let tick_plr = self.storage.just_get_player_mut(*tick_plr).expect("plr not found when tick");
                tick_plr.step(&mut self.randomer, updates);
            } else {
                unreachable!("tick_plr_index out of range");
            }
        }
    }
}

pub mod update {

    use crate::player::PlrPtr;

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
        score: i32,
        delay0: i32,
        delay1: i32,
        message: String,
        caster: PlrPtr,
        target: PlrPtr,
        targets: Vec<PlrPtr>,
        update_type: UpdateType,
        // param: Object ?
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
                // param: Object ?
            }
        }

        pub fn msg(&self) -> String {
            // [0] -> caster
            // [1] -> target
            // [2] -> targets
            let mut msg = self.message.clone();
            msg = msg.replace("{0}", &self.caster.to_string());
            msg = msg.replace("{1}", &self.target.to_string());
            msg = msg.replace(
                "{2}",
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
                // param: Object ?
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
                let raw_input = format!("a\nb{}c\nd", new_lines);
                let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
                assert_eq!(groups, (vec![plr!["a", "b"], plr!["c", "d"]], plr!()));
            }
            // 以及有多个队伍的情况
            for x in 2..10 {
                let new_lines = "\n".repeat(x);
                let raw_input = format!("a\nb{}c\nd{}e", new_lines, new_lines);
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
            // 需要修复的seed
            let raw_input = "aaaa\nbbbb\n\nseed: a@!".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            // assert_eq!(groups, vec![vec!["aaaa", "bbbb"], vec!["seed: a@!"]]);
            // 这个情况下，应该是修复成三个队伍
            // TODO
            assert_ne!(groups, (plrs!("aaaa", "bbbb", "seed: a@!"), plr!["seed: a@!"]))
        }

        #[test]
        /// 应该faild
        /// TODO
        // #[should_panic]
        fn need_fix_seed2() {
            // 跟 test 1 顺序相反
            let raw_input = "seed: a@!\n\naaaa\nbbbb".to_string();
            // 合法输入: seed: a@!\naaaa\nbbbb
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_ne!(groups, (vec![plr!("seed: a@!", "aaaa", "bbbb")], plr!["seed: a@!"]));
            // 这个情况下，应该是修复成三个队伍
            assert_eq!(groups, (plrs!("seed: a@!", "aaaa", "bbbb"), plr!["seed: a@!"]))
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

            for (i, plr) in runner.players.iter().flatten().enumerate() {
                let plr = runner.storage.get_player(*plr).expect("plr not found");
                assert_eq!(plr.sort_int as u32, { ints[i] });
            }
        }
    }
}

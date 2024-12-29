pub const PROFILE_START: u32 = 33554431;

pub mod runners {

    use crate::engine::update::{RunUpdate, RunUpdates};
    use crate::error::runner::RunnerResult;
    use crate::player::{Player, PlrPtr};
    use crate::rc4::RC4;

    pub type PlayerGroup = Vec<Player>;

    pub struct Runner {
        /// 应该是一个 Rc4 实例类似物
        pub randomer: RC4,
        /// 所有玩家 (包括 boss)
        pub players: Vec<PlayerGroup>,
        /// 赢家
        ///
        /// 也应该是一个队伍
        pub winner: Option<PlayerGroup>,
        /// 该哪个玩家动了
        round_pos: i32,
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
                .chain(seed.iter())
                .map(|str| Player::raw_namerena_to_idname(str))
                .collect::<Vec<String>>();
            // 这里顺便把 sorted hash 这块做了
            names.sort();
            names.dedup();
            let keys = names.join("\n");
            let mut randomer = RC4::new(keys.as_bytes(), 1);
            randomer.encrypt_bytes_no_change(&keys);
            // 准备好了
            // 用 randmoer 初始化玩家的 sort_int

            let mut inited_plrs = Vec::with_capacity(players.len());
            for plrs in players.iter() {
                let mut group = Vec::with_capacity(plrs.len());
                for plr in plrs.iter() {
                    let mut player = Player::new_from_namerena_raw(plr.to_string())?;
                    player.sort_int = randomer.rFFFFFF() as i32;
                    // 如果有问题，就直接返回错误
                    // 不过大概率不会有问题就是了
                    group.push(player);
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
                        if plr_p.clan_name() == plr_q.clan_name() {
                            plr_p.upgrade(&plr_q.rand);
                            plr_q.upgrade(&plr_p.rand);
                        }
                    }
                }
            }

            for group in inited_plrs.iter_mut() {
                for plr in group.iter_mut() {
                    plr.build();
                }
            }

            let mut sort_groups = inited_plrs.iter().collect::<Vec<&PlayerGroup>>();
            sort_groups.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap_or(std::cmp::Ordering::Equal));

            for group in sort_groups.iter() {
                for plr in group.iter() {
                    randomer.encrypt_bytes_no_change(&plr.id_name());
                }
                randomer.encrypt_bytes(&mut [0]);
            }

            for group in inited_plrs.iter_mut() {
                for plr in group.iter_mut() {
                    plr.set_move_point(randomer.r255());
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
            })
        }

        /// 获取所有存活的玩家
        pub fn alives_flat(&self) -> Vec<&Player> { self.players.iter().flatten().filter(|x| x.status().alive()).collect() }

        /// 以组为单位获取所有存活的玩家
        pub fn alives(&self) -> Vec<Vec<&Player>> {
            self.players.iter().map(|x| x.iter().filter(|x| x.status().alive()).collect()).collect()
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
        pub fn all_plrs(&self) -> Vec<&Player> { self.players.iter().flatten().collect() }

        /// 你甚至可以通过他们的指针来直接访问对应的玩家
        pub fn all_plr_ptrs(&self) -> Vec<PlrPtr> { self.players.iter().flatten().map(|x| x.uid()).collect() }

        #[inline]
        pub fn all_plr_len(&self) -> usize { self.players.iter().map(|x| x.len()).sum() }

        pub fn get_plr_by_ptr(&self, ptr: PlrPtr) -> Option<&Player> {
            for group in self.players.iter() {
                for plr in group.iter() {
                    if plr.uid() == ptr {
                        return Some(plr);
                    }
                }
            }
            None
        }

        pub unsafe fn get_plr_by_ptr_unchecked(&self, ptr: PlrPtr) -> &Player {
            // 直接 unsafe 强转
            let ptr = ptr as *const Player;
            &*ptr
        }

        pub fn main_round(&mut self) { let mut updates = RunUpdates::new(); }

        pub fn round_tick(&mut self, updates: &mut RunUpdates) {
            self.round_pos += 1;
            self.round_pos %= self.all_plr_len() as i32;

            let tick_plr_index = self.round_pos as usize;

            let tick_plr_ptr = self.all_plr_ptrs()[tick_plr_index];

            // WARN: 我直接用 ptr 来获取玩家了
            // TODO: 换成直接获取玩家的方法
            let tick_plr = unsafe {
                // 直接将 usize 转换成 &Player
                // 这里是安全的，因为我们知道这个指针是有效的
                &mut *(tick_plr_ptr as *mut Player)
            };
            // 调用 step 方法
            tick_plr.step(&mut self.randomer, updates);
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
            assert_eq!(groups, (plrs!("b", "c"), plr!["seed: a@!"]));
        }

        #[test]
        fn need_fix_seed1() {
            // 需要修复的seed
            let raw_input = "aaaa\nbbbb\n\nseed: a@!".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            // assert_eq!(groups, vec![vec!["aaaa", "bbbb"], vec!["seed: a@!"]]);
            // 这个情况下，应该是修复成三个队伍
            // TODO
            assert_ne!(groups, (plrs!("aaaa", "bbbb"), plr!["seed: a@!"]))
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
            assert_ne!(groups, (vec![plr!("aaaa", "bbbb")], plr!["seed: a@!"]));
            // 这个情况下，应该是修复成三个队伍
            assert_eq!(groups, (plrs!("aaaa", "bbbb"), plr!["seed: a@!"]))
        }
    }

    mod runner {
        use super::*;

        #[test]
        fn sort_int_test() {
            let raw_input = "aaa\nbbb\nseed: aaaa@!";
            let runner = runners::Runner::new_from_namerena_raw(raw_input.to_string()).unwrap();

            let ints = [2415636, 7852640, 14598063];
            assert!(!runner.have_winner());

            for (i, plr) in runner.players.iter().flatten().enumerate() {
                assert_eq!(plr.sort_int as u32, { ints[i] });
            }
        }
    }
}

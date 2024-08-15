pub const PROFILE_START: u32 = 33554431;

pub mod runners {
    use thiserror::Error;

    use std::collections::HashMap;

    use crate::player::{Player, PlayerError};
    use crate::rc4::RC4;

    #[derive(Error, Debug)]
    pub enum PlayerGroupError {
        /// 某个玩家解析失败
        /// 通常是因为名竞的输入格式不对
        ///
        /// 0: 玩家名
        /// 1: 错误原因
        #[error("Player parse error: {0}")]
        PlayerParseError(#[from] PlayerError),
    }

    #[derive(Error, Debug)]
    pub enum RunnerError {
        /// 某个队伍解析失败
        /// 通常是因为名竞的输入格式不对
        ///
        /// 0: 队伍名
        /// 1: 错误原因
        #[error("PlayerGroup parse error: {0}")]
        PlayerGroupParseError(#[from] PlayerGroupError),
        /// 某个人在创建过程中报错
        #[error("Player parse error: {0}")]
        PlayerError(#[from] PlayerError),
        /// 只有一个队伍
        #[error("Only one group")]
        OnlyOneGroup,
    }

    pub type PlayerGroupResult<T> = Result<T, PlayerGroupError>;
    pub type RunnerResult<T> = Result<T, RunnerError>;

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
            names.sort();
            let mut keys = names.join("\n").as_bytes().to_vec();
            let mut randomer = RC4::new(&keys, 1);
            randomer.encrypt_bytes(&mut keys);
            // 准备好了
            // 用 randmoer 初始化玩家的 sort_int
            let sort_ints: HashMap<&str, u32> = HashMap::from_iter(names.iter().map(|name| (name.as_str(), randomer.rFFFFFF())));
            let mut plrs = Vec::with_capacity(players.len());
            for group in players.iter() {
                let mut plr = Vec::with_capacity(group.len());
                for name in group {
                    let mut player = Player::new_from_namerena_raw(name.to_owned())?;
                    // 如果有问题，就直接返回错误
                    // 不过大概率不会有问题就是了
                    player.sort_int = sort_ints.get(player.name.as_str()).unwrap().to_owned() as i32;
                    plr.push(player);
                }
                plrs.push(plr);
            }
            let winner = if plrs.len() == 1 { Some(plrs.pop().unwrap()) } else { None };
            Ok(Runner {
                randomer,
                players: plrs,
                winner,
            })
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
                return (
                    raw_input
                        .split("\n")
                        .filter(|x| !Player::check_is_seed(x))
                        .map(|x| vec![x.to_string()])
                        .collect(),
                    seed,
                );
            }
            let raw_input = raw_input
                .split("\n")
                .filter(|x| !Player::check_is_seed(x))
                .collect::<Vec<&str>>()
                .join("\n");
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
            // let mut need_fix = false;
            let groups = raw_input
                .split("\n\n")
                .map(|x| x.split("\n").map(|x| x.to_string()).collect())
                .collect::<Vec<Vec<String>>>();
            // groups.iter().for_each(|group| {
            //     if group.len() == 1 && Player::check_is_seed(group[0]) {
            //         need_fix = true;
            //     }
            // });
            // 然后就是一些特判
            // 比如双队伍, 同时其中一个是纯 seed
            if groups.len() == 2 {
                // 双队伍特判
                // 队伍1是纯seed
                // 队伍2不是纯seed
                if groups[0].len() == 1
                    && Player::check_is_seed(groups[0][0].as_str())
                    && groups[1].iter().all(|x| !Player::check_is_seed(x))
                {
                    // 进行一个 fix
                }
            }

            (
                raw_input.split("\n\n").map(|x| x.split("\n").map(|x| x.to_string()).collect()).collect(),
                seed,
            )
        }

        #[inline]
        pub fn have_winner(&self) -> bool { self.winner.is_some() }
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
        fn need_fix_seed() {
            // 需要修复的seed
            let raw_input = "aaaa\nbbbb\n\nseed: a@!".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            // assert_eq!(groups, vec![vec!["aaaa", "bbbb"], vec!["seed: a@!"]]);
            // 这个情况下，应该是修复成三个队伍
            // TODO
            assert_ne!(groups, (plrs!("aaaa", "bbbb"), plr!["seed: a@!"]))
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

pub const PROFILE_START: u32 = 33554431;

pub mod runners {
    use thiserror::Error;

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

    pub struct PlayerGroup {
        players: Vec<Player>,
    }

    impl PlayerGroup {
        pub fn new(players: Vec<Player>) -> PlayerGroup { PlayerGroup { players } }
    }

    pub struct Runner {
        /// 应该是一个 Rc4 实例类似物
        randomer: RC4,
        /// 所有玩家 (包括 boss)
        players: Vec<PlayerGroup>,
        /// 赢家
        ///
        /// 也应该是一个队伍
        winner: Option<PlayerGroup>,
    }

    impl Runner {
        /// 从一个 名竞的原始输入 中创建一个 Runner
        ///
        /// 其实就是解析名竞的输入格式
        pub fn new_from_namerena_raw(raw_input: String) -> RunnerResult<Runner> {
            let spilted_input = raw_input.split("\n").collect::<Vec<&str>>();
            let mut players = Vec::new();
            for player in spilted_input.iter().filter(|name| !name.is_empty()) {
                let player = Player::new_from_namerena_raw(player.to_string())?;
                players.push(player);
            }
            // 根据原始输入解析队伍
            todo!()
        }

        pub fn spilt_namerena_into_groups(raw_input: String) -> Vec<Vec<String>> {
            // 去除尾部的一个/多个 \n/带有几个空格的情况
            let raw_input = raw_input.trim_end();
            // 首先，如果没有\n\n, 那么一行就是一个队伍
            if !raw_input.contains("\n\n") {
                return raw_input.split("\n").map(|x| vec![x.to_string()]).collect();
            }
            todo!()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod spilt_namerena_into_groups {
        use super::*;
        #[test]
        fn basic_spilt() {
            // 没有 \n\n 的最基本情况
            let raw_input = "a\nb\nc".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, vec![vec!["a"], vec!["b"], vec!["c"]]);

            // 跟随着一个或者多个尾部 \n 的情况
            // 自动忽略
            let raw_input = "a\nb\nc\n".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, vec![vec!["a"], vec!["b"], vec!["c"]]);
            let raw_input = "a\nb\nc\n\n".to_string();
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, vec![vec!["a"], vec!["b"], vec!["c"]]);
        }
    }
}

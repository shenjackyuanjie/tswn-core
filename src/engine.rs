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
            let spilted_input = raw_input.split("\n");
            let mut players = Vec::new();
            for raw_player in spilted_input {
                let player = Player::new_from_namerena_raw(raw_player.to_string())?;
                players.push(player);
            }

            // 首先以 \n\n 分割
            // let mut raw_input = raw_input.split("\n\n");
            // let mut groups = Vec::new();
            // // 如果只有一组
            // if raw_input.clone().count() == 1 {
            //     // 每个玩家一组
            //     let input = raw_input.next().unwrap();
            //     // 用 \n 分割
            //     let raw_input = input.split("\n");
            //     // 如果只有一个人
            //     // 直接丢个错误
            //     if raw_input.clone().count() == 1 {
            //         return Err(RunnerError::OnlyOneGroup);
            //     }
            //     for player in raw_input {
            //         let group = PlayerGroup::new_from_namerena_raw(player.to_string())?;
            //         groups.push(group);
            //     }
            // } else {
            //     // 每个组一组
            //     for raw_group in raw_input {
            //         let group = PlayerGroup::new_from_namerena_raw(raw_group.to_string())?;
            //         groups.push(group);
            //     }
            // }
            // // 尝试生成 PlayerGroup
            // let mut players = Vec::new();
            // for raw_group in raw_input {
            //     let group = PlayerGroup::new_from_namerena_raw(raw_group.to_string())?;
            //     players.push(group);
            // }
            // // 新建一个 Rc4 实例
            // // let randomer = RC4::new(PROFILE_START);
            todo!()
        }
    }
}

#[cfg(test)]
mod tests {}

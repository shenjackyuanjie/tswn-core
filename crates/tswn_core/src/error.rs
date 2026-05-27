//! 引擎错误类型定义。
//!
//! 包含两类错误：
//! - `player` 子模块：[`PlayerError`]，玩家属性校验错误；
//! - `runner` 子模块：[`RunnerError`]，战斗执行器解析/运行错误。

pub mod player {
    use std::fmt::Display;

    use crate::player::{NAME_MAX_LEN, TEAM_MAX_LEN};

    pub type PlayerResult<T> = Result<T, PlayerError>;

    #[derive(Debug)]
    pub enum PlayerError {
        /// 名字中包含了非法字符
        ///
        /// - String: 是啥
        /// - usize: 在名字中的位置
        InvalidTextInName(String, usize),
        /// 名字太长了!!
        ///
        /// - usize: bytes 实际长度
        /// - usize: 字符串长度
        NameTooLong(usize, usize),
        /// 队伍名中包含了非法字符
        ///
        /// - String: 是啥
        /// - usize: 在队伍名中的位置
        InvalidTextInTeam(String, usize),
        /// 队伍名太长了!!
        ///
        /// - usize: bytes 实际长度
        /// - usize: 字符串长度
        TeamNameTooLong(usize, usize),
        /// 武器里怎么也包含非法字符呢
        /// 输入中包含换行符
        ///
        /// 单独把你拿出来
        NewlineInInput(Vec<usize>),
    }

    impl Display for PlayerError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                PlayerError::InvalidTextInName(s, i) => {
                    write!(f, "Invalid Text(s): {s} in name[{i}]")
                }
                PlayerError::NameTooLong(b, s) => {
                    write!(f, "Name too long, max length is {} < {b} strlen={s}", NAME_MAX_LEN)
                }
                PlayerError::InvalidTextInTeam(s, i) => {
                    write!(f, "Invalid Text(s): {s} in team[{i}]")
                }
                PlayerError::TeamNameTooLong(b, s) => {
                    write!(f, "Team name too long, max length is {} < {b} strlen={s}", TEAM_MAX_LEN)
                }
                PlayerError::NewlineInInput(v) => {
                    write!(f, "Input contains newline character in {v:?}")
                }
            }
        }
    }

    impl std::error::Error for PlayerError {}
}

pub mod runner {
    use std::fmt::Display;

    use super::player::PlayerError;

    pub type PlayerGroupResult<T> = Result<T, PlayerGroupError>;
    pub type RunnerResult<T> = Result<T, RunnerError>;

    #[derive(Debug)]
    pub enum PlayerGroupError {
        /// 某个玩家解析失败
        /// 通常是因为名竞的输入格式不对
        ///
        /// 0: 玩家名
        /// 1: 错误原因
        PlayerParseError(PlayerError),
    }

    impl Display for PlayerGroupError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                PlayerGroupError::PlayerParseError(e) => {
                    write!(f, "Player parse error: {}", e)
                }
            }
        }
    }

    impl std::error::Error for PlayerGroupError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                PlayerGroupError::PlayerParseError(e) => Some(e),
            }
        }
    }

    #[derive(Debug)]
    pub enum RunnerError {
        /// 某个队伍解析失败
        /// 通常是因为名竞的输入格式不对
        ///
        /// 0: 队伍名
        /// 1: 错误原因
        PlayerGroupParseError(PlayerGroupError),
        /// 某个人在创建过程中报错
        PlayerError(PlayerError),
        /// 只有一个队伍
        OnlyOneGroup,
    }

    impl From<PlayerError> for RunnerError {
        fn from(e: PlayerError) -> Self { RunnerError::PlayerError(e) }
    }

    impl Display for RunnerError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                RunnerError::PlayerGroupParseError(e) => {
                    write!(f, "PlayerGroup parse error: {}", e)
                }
                RunnerError::PlayerError(e) => {
                    write!(f, "Player parse error: {}", e)
                }
                RunnerError::OnlyOneGroup => {
                    write!(f, "Only one group")
                }
            }
        }
    }

    impl std::error::Error for RunnerError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                RunnerError::PlayerGroupParseError(e) => Some(e),
                RunnerError::PlayerError(e) => Some(e),
                RunnerError::OnlyOneGroup => None,
            }
        }
    }
}

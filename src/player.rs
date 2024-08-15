pub mod skills;
pub mod utils;

use std::{cmp::Ordering, fmt::Display};

use thiserror::Error;

use crate::rc4::RC4;

/// 名字本体最大长度
pub const NAME_MAX_LEN: usize = 256;
/// 队伍名最大长度
pub const TEAM_MAX_LEN: usize = 256;

#[derive(Error, Debug)]
pub enum PlayerError {
    /// 名字中包含了非法字符
    ///
    /// - String: 是啥
    /// - usize: 在名字中的位置
    #[error("Invalid Text(s): {0} in name[{1}]")]
    InvalidTextInName(String, usize),
    /// 名字太长了!!
    ///
    /// - usize: bytes 实际长度
    /// - usize: 字符串长度
    #[error(
        "Name too long, max length is {} < {0} strlen={1}",
        NAME_MAX_LEN
    )]
    NameTooLong(usize, usize),
    /// 队伍名太长了!!
    ///
    /// - usize: bytes 实际长度
    /// - usize: 字符串长度
    #[error(
        "Team name too long, max length is {} < {0} strlen={1}",
        TEAM_MAX_LEN
    )]
    TeamNameTooLong(usize, usize),
    /// 队伍名中包含了非法字符
    ///
    /// - String: 是啥
    /// - usize: 在队伍名中的位置
    #[error("Invalid Text(s): {0} in team[{1}]")]
    InvalidTextInTeam(String, usize),
    /// 武器里怎么也包含非法字符呢
    /// 输入中包含换行符
    ///
    /// 单独把你拿出来
    #[error("Input contains newline character in {0:?}")]
    NewlineInInput(Vec<usize>),
}

pub type PlayerResult<T> = Result<T, PlayerError>;

pub struct PlayerStatus {
    /// 是否被冻结
    frozen: bool,
    /// 是否存活
    alive: bool,
    /// 血量
    hp: u32,
    /// 分数
    point: u32,
}

impl Default for PlayerStatus {
    fn default() -> Self {
        PlayerStatus {
            frozen: false,
            alive: true,
            hp: 0,
            point: 0,
        }
    }
}

impl Display for PlayerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PlayerStatus{{{},{} hp: {}, point: {} }}",
            // 冻结/正常
            // 存活/死亡
            if self.frozen { "冻结" } else { "正常" },
            if self.alive { "存活" } else { "死亡" },
            self.hp,
            self.point
        )
    }
}

pub struct Player {
    /// 队伍
    team: Option<String>,
    /// 玩家名
    pub name: String,
    /// 武器
    weapon: Option<String>,
    /// 玩家类型
    player_type: PlayerType,
    /// skl id
    skil_id: Vec<u32>,
    /// skl prop
    skil_prop: Vec<u32>,
    /// 玩家的 sort int
    /// 用于在排序中比较两个玩家
    pub sort_int: i32,
    /// RC4
    pub rand: RC4,
    /// 玩家状态
    ///
    /// 主要是我懒得加一大堆字段
    status: PlayerStatus,
}

/// boss 玩家的名字
pub const BOSS_NAMES: [&str; 11] = [
    "mario", "sonic", "mosquito", "yuri", "slime", "ikaruga", "conan", "aokiji", "lazy", "covid", "saitama",
];

/// ["田一人", 18, "云剑狄卡敢", 25, "云剑穸跄祇", 35]
pub const BOOST_NAMES: [&str; 3] = ["云剑狄卡敢", "云剑穸跄祇", "田一人"];

/// 种子玩家的前缀
pub const SEED_PREFIX: &str = "seed:";

/// 匹配字符的 Unicode 码点
///
/// 其实就是过滤一下不可见字符
///
/// NOTE: 原始函数的实现方式是在内部有一个 match, 然后外面手动排除了 `13(\r)` 和 `32(空格)`
pub fn filter_char(s: char) -> bool {
    matches!(s as u32 , 9..12 | 133 | 160 | 5760 | 8192..8202 | 8232..8233 | 8239 | 8287 | 12288 | 65279)
}

#[derive(Default, PartialEq, Eq, Debug)]
pub enum PlayerType {
    #[default]
    Normal,
    /// 种子玩家
    ///
    /// # marker: `seed:`
    Seed,
    /// 被克隆的玩家
    ///
    /// 似乎有个三种?
    Clone,
    /// Boss 玩家
    /// 其实应该是一大堆
    Boss,
    /// 被特殊增强的玩家
    ///
    /// 有一堆玩家都被增强了
    Boost,
    /// 标准测号用靶子
    ///
    /// # marker: `\u0002`
    Test1,
    /// 没用到的测号用玩家
    ///
    /// # marker: `\u0003`
    Test2,
    /// 比标准测号再强一点的测号用靶子
    ///
    /// # marker: `!`
    TestEx,
}

impl Player {
    /// 按照 namerena 的原始 new
    pub fn namer_new(base_name: String, team_name: String, sgl_name: String, weapon: String) -> Self { todo!() }

    pub fn new(team: Option<String>, name: String, weapon: Option<String>) -> PlayerResult<Self> {
        // 先校验长度
        if team.is_some() && team.as_ref().unwrap().as_bytes().len() > TEAM_MAX_LEN {
            let t = team.unwrap();
            return Err(PlayerError::TeamNameTooLong(t.as_bytes().len(), t.len()));
        }
        if name.as_bytes().len() > NAME_MAX_LEN {
            return Err(PlayerError::NameTooLong(name.as_bytes().len(), name.len()));
        }
        // 再校验字符
        if let Some(t) = team.as_ref() {
            if t.chars().any(filter_char) {
                return Err(PlayerError::InvalidTextInTeam(
                    t.chars().find(|&char| filter_char(char)).unwrap().to_string(),
                    t.chars().position(filter_char).unwrap(),
                ));
            }
        }
        if name.chars().any(filter_char) {
            return Err(PlayerError::InvalidTextInName(
                name.chars().find(|&char| filter_char(char)).unwrap().to_string(),
                name.chars().position(filter_char).unwrap(),
            ));
        }
        let player_type = {
            if let Some(t) = team.as_ref() {
                match t.as_str() {
                    "!" => {
                        if BOSS_NAMES.contains(&name.as_str()) {
                            PlayerType::Boss
                        } else if BOOST_NAMES.contains(&name.as_str()) {
                            PlayerType::Boost
                        } else if name.starts_with(SEED_PREFIX) {
                            PlayerType::Seed
                        } else {
                            // 高强度测号用靶子
                            PlayerType::TestEx
                        }
                    }
                    "\u{0002}" => PlayerType::Test1,
                    "\u{0003}" => PlayerType::Test2,
                    _ => {
                        if name.starts_with(SEED_PREFIX) {
                            PlayerType::Seed
                        } else {
                            PlayerType::Normal
                        }
                    }
                }
            } else {
                PlayerType::Normal
            }
        };
        Ok(Player {
            team,
            name,
            weapon,
            player_type,
            sort_int: 0,
            rand: RC4::default(),
            skil_id: vec![],
            skil_prop: vec![],
            status: PlayerStatus::default(),
        })
    }

    /// 设置 sort int
    pub fn set_sort_int(&mut self, val: i32) { self.sort_int = val }
    /// 获取 sort int
    pub fn get_sort_int(&self) -> i32 { self.sort_int }

    /// 检查输入的名字是否是种子玩家
    pub fn check_is_seed(name: &str) -> bool { name.starts_with(SEED_PREFIX) }

    /// 直接从一个名竞的原始输入创建一个 Player
    ///
    /// # 要求
    /// 不许有 \n
    ///
    /// 可能的输入格式:
    /// - <name>
    /// - <name>@<team>
    /// - <name>+<weapon>
    /// - <name>+<weapon>+diy{xxxxx}
    /// - <name>@<team>+<weapon>
    /// - <name>@<team>+<weapon>+diy{xxxxx}
    pub fn new_from_namerena_raw(raw_name: String) -> PlayerResult<Self> {
        // 先判断是否有 + 和 @
        if !raw_name.contains("@") && !raw_name.contains("+") {
            return Player::new(None, raw_name.clone(), None);
        }
        // 区分队伍名
        let name: &str;
        let mut team: &str;
        let weapon: Option<&str>;
        if raw_name.contains("@") {
            (name, team) = raw_name.split_once("@").unwrap();
            // 判定武器
            if team.contains("+") {
                let tmp;
                (team, tmp) = team.split_once("+").unwrap();
                weapon = Some(tmp);
            } else {
                weapon = None;
            }
            Player::new(Some(team.to_string()), name.to_string(), weapon.map(|s| s.to_string()))
        } else {
            // 没有队伍名, 直接是武器
            if raw_name.contains("+") {
                let (name, weapon) = raw_name.split_once("+").unwrap();
                Player::new(None, name.to_string(), Some(weapon.to_string()))
            } else {
                Player::new(None, raw_name, None)
            }
        }
    }

    #[inline]
    pub fn raw_namerena_to_idname(raw_name: &str) -> String {
        // @/+ 后面的部分不要
        if let Some(idx) = raw_name.find("@") {
            raw_name[..idx].to_string()
        } else if let Some(idx) = raw_name.find("+") {
            raw_name[..idx].to_string()
        } else {
            raw_name.to_string()
        }
    }

    pub fn update_player(&mut self) {}

    /// 每回合中的玩家行动
    ///
    /// 包括 pre, main, post
    pub fn step(&mut self, _randomer: &mut RC4) {}

    #[inline]
    pub fn id_name(&self) -> String { self.name.clone() }
    #[inline]
    pub fn display_name(&self) -> String { self.name.split(" ").next().unwrap_or_default().to_string() }

    fn p_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.sort_int - other.sort_int != 0 {
            self.sort_int.partial_cmp(&other.sort_int)
        } else {
            self.id_name().partial_cmp(&other.id_name())
        }
    }
}

impl PartialOrd for Player {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { self.p_cmp(other) }
}

impl PartialEq for Player {
    fn eq(&self, other: &Self) -> bool { self.p_cmp(other).map(|cmp| matches!(cmp, Ordering::Equal)).unwrap_or(false) }
}

impl Display for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Player{{{}{}, status: {}}}",
            if self.team.is_some() {
                format!("{}@{}", self.name, self.team.as_ref().unwrap())
            } else {
                self.name.to_string()
            },
            if let Some(weapon) = &self.weapon {
                format!("+{}", weapon)
            } else {
                "".to_string()
            },
            self.status
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    /// 测试根据原始输入创建 Player
    fn player_raw_new() {
        let player = Player::new_from_namerena_raw("mario".to_string());
        let player = player.unwrap();
        assert_eq!(player.name, "mario");
        assert_eq!(player.team, None);
        assert_eq!(player.weapon, None);
        assert_eq!(player.player_type, PlayerType::Normal);

        let player = Player::new_from_namerena_raw("mario@red".to_string());
        let player = player.unwrap();
        println!("{}", player);
        assert_eq!(player.name, "mario");
        assert_eq!(player.team, Some("red".to_string()));
        assert_eq!(player.weapon, None);
        assert_eq!(player.player_type, PlayerType::Normal);

        let player = Player::new_from_namerena_raw("mario+fire".to_string());
        let player = player.unwrap();
        assert_eq!(player.name, "mario");
        assert_eq!(player.team, None);
        assert_eq!(player.weapon, Some("fire".to_string()));
        assert_eq!(player.player_type, PlayerType::Normal);

        let player = Player::new_from_namerena_raw("mario+fire+diy{xxxx}".to_string());
        let player = player.unwrap();
        assert_eq!(player.name, "mario");
        assert_eq!(player.team, None);
        assert_eq!(player.weapon, Some("fire+diy{xxxx}".to_string()));
        assert_eq!(player.player_type, PlayerType::Normal);

        let player = Player::new_from_namerena_raw("mario@red+fire".to_string());
        let player = player.unwrap();
        assert_eq!(player.name, "mario");
        assert_eq!(player.team, Some("red".to_string()));
        assert_eq!(player.weapon, Some("fire".to_string()));
        assert_eq!(player.player_type, PlayerType::Normal);

        let player = Player::new_from_namerena_raw("mario@red+fire+diy{xxxx}".to_string());
        let player = player.unwrap();
        assert_eq!(player.name, "mario");
        assert_eq!(player.team, Some("red".to_string()));
        assert_eq!(player.weapon, Some("fire+diy{xxxx}".to_string()));
        assert_eq!(player.player_type, PlayerType::Normal);
    }

    #[test]
    fn player_name() {
        let player = Player::new_from_namerena_raw("aaa".to_string()).unwrap();
        assert_eq!(player.id_name(), "aaa");
        assert_eq!(player.display_name(), "aaa");

        // 包含了 @
        let player = Player::new_from_namerena_raw("aaa@bbb".to_string()).unwrap();
        assert_eq!(player.id_name(), "aaa");
        assert_eq!(player.display_name(), "aaa");

        // 空格分开的名字
        let player = Player::new_from_namerena_raw("aaa bbb".to_string()).unwrap();
        assert_eq!(player.id_name(), "aaa bbb");
        assert_eq!(player.display_name(), "aaa");

        // 包含了 + 的名字
        let player = Player::new_from_namerena_raw("aaa+bbb".to_string()).unwrap();
        assert_eq!(player.id_name(), "aaa");
        assert_eq!(player.display_name(), "aaa");
    }

    #[test]
    fn player_raw_types() {
        let player = Player::new_from_namerena_raw("normal@normal".to_string());
        let player = player.unwrap();
        assert_eq!(player.player_type, PlayerType::Normal);

        // seed
        let player = Player::new_from_namerena_raw("seed:just seed@!".to_string());
        let player = player.unwrap();
        assert_eq!(player.name, "seed:just seed");
        assert_eq!(player.player_type, PlayerType::Seed);

        // testEx
        let player = Player::new_from_namerena_raw("testEx@!".to_string());
        let player = player.unwrap();
        assert_eq!(player.player_type, PlayerType::TestEx);

        // test1
        let player = Player::new_from_namerena_raw("test1@\u{0002}".to_string());
        let player = player.unwrap();
        assert_eq!(player.team, Some("\u{0002}".to_string()));
        assert_eq!(player.player_type, PlayerType::Test1);

        // test2
        let player = Player::new_from_namerena_raw("test2@\u{0003}".to_string());
        let player = player.unwrap();
        assert_eq!(player.team, Some("\u{0003}".to_string()));
        assert_eq!(player.player_type, PlayerType::Test2);

        // boss
        let player = Player::new_from_namerena_raw("mario@!".to_string());
        let player = player.unwrap();
        assert_eq!(player.player_type, PlayerType::Boss);

        // boosted
        let player = Player::new_from_namerena_raw("云剑狄卡敢@!".to_string());
        let player = player.unwrap();
        assert_eq!(player.player_type, PlayerType::Boost);
    }
}

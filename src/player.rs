pub mod skills;
pub mod utils;

use std::fmt::Display;

use crate::rc4::RC4;

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
            if self.alive { "存货" } else { "死亡" },
            self.hp,
            self.point
        )
    }
}

pub struct Player {
    /// 队伍
    team: String,
    /// 玩家名
    name: String,
    /// 武器
    weapon: Option<String>,
    /// 玩家类型
    player_type: PlayerType,
    /// skl id
    skil_id: Vec<u32>,
    /// skl prop
    skil_prop: Vec<u32>,
    /// 玩家状态
    ///
    /// 主要是我懒得加一大堆字段
    status: PlayerStatus,
}

pub const BOSS_NAMES: [&str; 11] = [
    "mario", "sonic", "mosquito", "yuri", "slime", "ikaruga", "conan", "aokiji", "lazy", "covid", "saitama",
];

// ["田一人", 18, "云剑狄卡敢", 25, "云剑穸跄祇", 35]
pub const BOOST_NAMES: [&str; 3] = ["云剑狄卡敢", "云剑穸跄祇", "田一人"];

/// 匹配字符的 Unicode 码点
/// 
/// 其实就是过滤一下不可见字符
pub fn filter_char(s: char) -> bool {
    matches!(s as u32 , 9..13 | 32 | 133 | 160 | 5760 | 8192..8202 | 8232..8233 | 8239 | 8287 | 12288 | 65279)
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
    pub fn new(team: String, name: String, weapon: Option<String>) -> Self {
        let player_type = {
            match team.as_str() {
                "!" => {
                    if BOSS_NAMES.contains(&name.as_str()) {
                        PlayerType::Boss
                    } else if BOOST_NAMES.contains(&name.as_str()) {
                        PlayerType::Boost
                    } else if name.starts_with("seed:") {
                        PlayerType::Seed
                    } else {
                        // 高强度测号用靶子
                        PlayerType::TestEx
                    }
                }
                "\u{0002}" => PlayerType::Test1,
                "\u{0003}" => PlayerType::Test2,
                _ => {
                    if name.starts_with("seed:") {
                        PlayerType::Seed
                    } else {
                        PlayerType::Normal
                    }
                }
            }
        };
        Player {
            team,
            name,
            weapon,
            player_type,
            skil_id: vec![],
            skil_prop: vec![],
            status: PlayerStatus::default(),
        }
    }

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
    pub fn new_from_namerena_raw(raw_name: String) -> Self {
        // 先判断是否有 + 和 @
        if !raw_name.contains("@") && !raw_name.contains("+") {
            return Player::new(raw_name.clone(), raw_name.clone(), None);
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
        } else {
            team = raw_name.as_str();
            // 没有队伍名, 直接是武器
            if team.contains("+") {
                let tmp;
                (name, tmp) = raw_name.split_once("+").unwrap();
                weapon = Some(tmp);
                team = name;
            } else {
                name = team;
                weapon = None;
            }
        }
        Player::new(team.to_string(), name.to_string(), weapon.map(|s| s.to_string()))
    }

    pub fn update_player(&mut self) {}

    pub fn step(&mut self, randomer: &mut RC4) {}
}

impl Display for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Player{{{}@{}{}, status: {}}}",
            self.name,
            self.team,
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
        assert_eq!(player.name, "mario");
        assert_eq!(player.team, "mario");
        assert_eq!(player.weapon, None);
        assert_eq!(player.player_type, PlayerType::Normal);

        let player = Player::new_from_namerena_raw("mario@red".to_string());
        println!("{}", player);
        assert_eq!(player.name, "mario");
        assert_eq!(player.team, "red");
        assert_eq!(player.weapon, None);
        assert_eq!(player.player_type, PlayerType::Normal);

        let player = Player::new_from_namerena_raw("mario+fire".to_string());
        assert_eq!(player.name, "mario");
        assert_eq!(player.team, "mario");
        assert_eq!(player.weapon, Some("fire".to_string()));
        assert_eq!(player.player_type, PlayerType::Normal);

        let player = Player::new_from_namerena_raw("mario+fire+diy{xxxx}".to_string());
        assert_eq!(player.name, "mario");
        assert_eq!(player.team, "mario");
        assert_eq!(player.weapon, Some("fire+diy{xxxx}".to_string()));
        assert_eq!(player.player_type, PlayerType::Normal);

        let player = Player::new_from_namerena_raw("mario@red+fire".to_string());
        assert_eq!(player.name, "mario");
        assert_eq!(player.team, "red");
        assert_eq!(player.weapon, Some("fire".to_string()));
        assert_eq!(player.player_type, PlayerType::Normal);

        let player = Player::new_from_namerena_raw("mario@red+fire+diy{xxxx}".to_string());
        assert_eq!(player.name, "mario");
        assert_eq!(player.team, "red");
        assert_eq!(player.weapon, Some("fire+diy{xxxx}".to_string()));
        assert_eq!(player.player_type, PlayerType::Normal);
    }

    #[test]
    pub fn player_raw_types() {
        let player = Player::new_from_namerena_raw("normal@normal".to_string());
        assert_eq!(player.player_type, PlayerType::Normal);

        // seed
        let player = Player::new_from_namerena_raw("seed:just seed@!".to_string());
        assert_eq!(player.name, "seed:just seed");
        assert_eq!(player.player_type, PlayerType::Seed);

        // testEx
        let player = Player::new_from_namerena_raw("testEx@!".to_string());
        assert_eq!(player.player_type, PlayerType::TestEx);

        // test1
        let player = Player::new_from_namerena_raw("test1@\u{0002}".to_string());
        assert_eq!(player.team, "\u{0002}".to_string());
        assert_eq!(player.player_type, PlayerType::Test1);

        // test2
        let player = Player::new_from_namerena_raw("test2@\u{0003}".to_string());
        assert_eq!(player.team, "\u{0003}".to_string());
        assert_eq!(player.player_type, PlayerType::Test2);

        // boss
        let player = Player::new_from_namerena_raw("mario@!".to_string());
        assert_eq!(player.player_type, PlayerType::Boss);

        // boosted
        let player = Player::new_from_namerena_raw("云剑狄卡敢@!".to_string());
        assert_eq!(player.player_type, PlayerType::Boost);
    }
}

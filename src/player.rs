pub mod eval_name;
pub mod skills;
pub mod utils;

use std::cmp::Ordering;

use crate::error::player::{PlayerError, PlayerResult};
use crate::rc4::RC4;

/// 名字本体最大长度
pub const NAME_MAX_LEN: usize = 256;
/// 队伍名最大长度
pub const TEAM_MAX_LEN: usize = 256;

pub struct PlayerStatus {
    /// 是否被冻结
    frozen: bool,
    /// 是否存活
    alive: bool,
    /// 分数
    point: u32,
    /// 血量
    pub hp: u32,
    /// 攻击力
    pub attack: u32,
    /// 防御
    pub defense: u32,
    /// 速度
    pub speed: u32,
    /// 敏捷
    pub agility: u32,
    /// 魔法
    pub magic: u32,
    /// 抗性
    pub resistance: u32,
    /// 智力
    pub wisdom: u32,
}

impl PlayerStatus {
    #[inline]
    pub fn frozed(&self) -> bool { self.frozen }
    #[inline]
    pub fn alive(&self) -> bool { self.alive }
}

impl Default for PlayerStatus {
    fn default() -> Self {
        PlayerStatus {
            frozen: false,
            alive: true,
            point: 0,
            hp: 0,
            attack: 0,
            defense: 0,
            speed: 0,
            agility: 0,
            magic: 0,
            resistance: 0,
            wisdom: 0,
        }
    }
}

impl std::fmt::Display for PlayerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PlayerStatus{{{},{} point: {}, hp: {} 攻|{} 防|{} 速|{} 敏|{} 魔|{} 抗|{} 智|{} }}",
            // 冻结/正常
            // 存活/死亡
            if self.frozen { "冻结" } else { "正常" },
            if self.alive { "存活" } else { "死亡" },
            self.point,
            self.hp,
            self.attack,
            self.defense,
            self.speed,
            self.agility,
            self.magic,
            self.resistance,
            self.wisdom
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
    /// name base?
    /// [u8; 128]
    pub name_base: Vec<u8>,
    /// 玩家状态
    ///
    /// 主要是我懒得加一大堆字段
    status: PlayerStatus,
    /// 名字长度系数
    name_factor: f64,
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
    // /// 按照 namerena 的原始 new
    // pub fn namer_new(base_name: String, team_name: String, sgl_name: String, weapon: String) -> Self { todo!() }

    /// 创建一个新的玩家
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
        let name_bytes = [0_u8].iter().chain(name.as_bytes()).copied().collect::<Vec<u8>>();
        let team_bytes = [0_u8]
            .iter()
            .chain(team.as_ref().unwrap_or(&name).as_bytes())
            .copied()
            .collect::<Vec<u8>>();

        let mut rand = RC4::new(&team_bytes, 1);
        rand.update(&name_bytes, 2);

        let mut name_base = vec![];

        for i in 0..255 {
            let j = (rand.get_val(i) as u32 * 181 + 160) as u8;
            if 88 < j && j < 217 {
                name_base.push(j);
            }
        }
        let mut skills = (0..39).collect::<Vec<u32>>();
        rand.sort_list(&mut skills);

        let name_factor = {
            let factor_name = eval_name::eval_str_common(name.as_str());
            let factor_team = match team.as_ref() {
                Some(team) => eval_name::eval_str_common(team.as_str()),
                None => factor_name,
            };
            factor_team.max(factor_name - 6.0)
        };

        Ok(Player {
            team,
            name,
            weapon,
            player_type,
            sort_int: 0,
            rand,
            name_base,
            skil_id: skills.clone(),
            skil_prop: skills,
            status: PlayerStatus::default(),
            name_factor,
        })
    }

    /// 根据名字系数调整数值
    ///
    /// ```javascript
    /// const result = Math.round(a * (1 - this.x / b))
    /// ```
    fn scale_by_name_factor(&self, val: u32, factor2: u32) -> u32 {
        (val as f64 * (1.0 - self.name_factor / factor2 as f64)).round() as u32
    }

    pub fn build(&mut self) {
        // TODO: weapon
        if let Some(_weapon) = &self.weapon {
            // weapon
        }
        // init raw attr
        let mut rand_vals = [0_u8; 32];
        rand_vals.copy_from_slice(&self.rand.main_val[0..32]);
        rand_vals.get_mut(0..10).unwrap().sort_unstable();
        self.status.hp = self.scale_by_name_factor(
            rand_vals[3] as u32 + rand_vals[4] as u32 + rand_vals[5] as u32 + rand_vals[6] as u32,
            128,
        );
    }

    pub fn upgrade(&mut self, rand: &RC4) {
        // 升级!
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
    /// 不许有 `\n`
    ///
    /// 可能的输入格式:
    /// - \<name>
    /// - \<name>@\<team>
    /// - \<name>+\<weapon>
    /// - \<name>+\<weapon>+diy{xxxxx}
    /// - \<name>@<team>+\<weapon>
    /// - \<name>@<team>+\<weapon>+diy{xxxxx}
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

    /// 把原始的 namerena 名字转换为 id name
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

    /// 更新玩家
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

impl std::fmt::Display for Player {
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

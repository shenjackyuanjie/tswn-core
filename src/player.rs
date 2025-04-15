pub mod eval_name;
pub mod skill_state;
pub mod utils;
pub mod weapons;

use std::cmp::{Ordering, min};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use crate::engine::storage::{SkillId, Storage};
use crate::engine::update::RunUpdates;
use crate::error::player::{PlayerError, PlayerResult};
use crate::player::skill_state::{Skill, SkillStore};
use crate::rc4::RC4;

/// 名字本体最大长度
pub const NAME_MAX_LEN: usize = 256;
/// 队伍名最大长度
pub const TEAM_MAX_LEN: usize = 256;

/// 2048 以上才行动
pub const MOVE_POINT_THRESHOLD: i32 = 2048;

/// 假装是一个指针
/// (其实就是 usize)
pub type PlrPtr = usize;

/// 将 PlrPtr 转换为 &mut Player
/// 其实就是一个包装
pub fn player_ptr_as_mut_plr<'a>(ptr: &PlrPtr) -> &'a mut Player { unsafe { &mut *(*ptr as *mut Player) } }

/// Player 的自增 ID
pub static PLAYER_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug)]
pub struct PlayerStatus {
    /// 是否被冻结
    frozen: bool,
    /// 是否存活
    alive: bool,
    /// 分数
    point: u32,
    /// 原文: spsum
    /// >= 2048 时才行动
    ///
    /// 单调递增, >= 2048 时 -= 2048
    /// 然后接着单增
    pub move_point: i32,
    /// 血量
    pub hp: i32,
    /// 最大血量
    pub max_hp: i32,
    /// 攻击力 (atk)
    pub attack: i32,
    /// 防御 (def)
    pub defense: i32,
    /// 速度 (spd)
    pub speed: i32,
    /// 敏捷 (agl)
    pub agility: i32,
    /// 魔法 (mag)
    pub magic: i32,
    /// 蓝条
    pub mp: i32,
    /// 抗性 (mdf)
    pub resistance: i32,
    /// 智力 (itl)
    pub wisdom: i32,
    /// 蓄力速度?
    pub at_boost: f64,
    /// attract ?
    pub attract: f64,
    /// 总属性和
    pub attr_sum: u32,
    /// 攻击和?
    pub atk_sum: i32,
    /// 总和?
    pub all_sum: u32,
}

impl PlayerStatus {
    #[inline]
    pub fn frozed(&self) -> bool { self.frozen }
    #[inline]
    pub fn alive(&self) -> bool { self.alive }
    #[deprecated(note = "请使用 move_point()")]
    #[inline]
    pub fn spsum(&self) -> i32 { self.move_point }
    #[inline]
    pub fn check_move(&self) -> bool { self.move_point >= MOVE_POINT_THRESHOLD }

    pub fn set_frozen(&mut self, val: bool) { self.frozen = val }

    pub fn set_alive(&mut self, val: bool) { self.alive = val }

    pub fn set_point(&mut self, val: u32) { self.point = val }
}

impl Default for PlayerStatus {
    fn default() -> Self {
        PlayerStatus {
            frozen: false,
            alive: true,
            point: 0,
            move_point: 0,
            hp: 0,
            max_hp: 0,
            attack: 0,
            defense: 0,
            speed: 0,
            agility: 0,
            magic: 0,
            mp: 0,
            resistance: 0,
            wisdom: 0,
            at_boost: 1.0,
            attract: 32768.0,
            attr_sum: 0,
            atk_sum: 0,
            all_sum: 0,
        }
    }
}

impl std::fmt::Display for PlayerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PlayerStatus{{{},{} 分数: {}, hp: {} 移动点数: {} sums:{},{},{} 攻|{} 防|{} 速|{} 敏|{} 魔|{} mp|{} 抗|{} 智|{} }}",
            // 冻结/正常
            // 存活/死亡
            if self.frozen { "冻结" } else { "正常" },
            if self.alive { "存活" } else { "死亡" },
            self.point,
            self.move_point,
            self.attr_sum,
            self.atk_sum,
            self.all_sum,
            self.hp,
            self.attack,
            self.defense,
            self.speed,
            self.agility,
            self.magic,
            self.mp,
            self.resistance,
            self.wisdom
        )
    }
}

/// boss 玩家的名字
pub const BOSS_NAMES: [&str; 11] = [
    "mario", "sonic", "mosquito", "yuri", "slime", "ikaruga", "conan", "aokiji", "lazy", "covid", "saitama",
];

/// ["田一人", 18, "云剑狄卡敢", 25, "云剑穸跄祇", 35]
pub const BOOST_NAMES: [&str; 3] = ["云剑狄卡敢", "云剑穸跄祇", "田一人"];

pub fn boost_value(name: &str) -> u32 {
    match name {
        "云剑狄卡敢" => 25,
        "云剑穸跄祇" => 35,
        "田一人" => 18,
        _ => 0,
    }
}

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

pub fn median<T>(x: T, y: T, z: T) -> T
where
    T: std::cmp::Ord + std::marker::Copy,
{
    if x < y {
        if y < z {
            y
        } else if x < z {
            z
        } else {
            x
        }
    } else if x < z {
        x
    } else if y < z {
        z
    } else {
        y
    }
}
#[derive(Default, PartialEq, Eq, Debug, Clone, Copy)]
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

#[derive(Clone, Debug)]
pub struct Player {
    /// 队伍
    team: Option<String>,
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
    /// 玩家的 sort int
    /// 用于在排序中比较两个玩家
    pub sort_int: i32,
    /// RC4
    pub rand: RC4,
    /// name base
    /// ```python
    /// len(list(i for i in range(256) if (i * 181 + 160) % 256 > 88 and (i * 181 + 160) % 256 < 217 )) == 128
    /// ```
    pub name_base: Vec<u8>,
    /// 没 upgrade 过的 name base
    raw_name_base: [u8; 128],
    /// 原始的属性数据
    attr: [u32; 8],
    /// 玩家状态
    ///
    /// 主要是我懒得加一大堆字段
    status: PlayerStatus,
    /// 技能相关
    skill_store: skill_state::SkillStore,
    /// 名字长度系数
    name_factor: f64,
    /// store
    pub storage: Arc<Storage>,
    /// plr id
    id: usize,
}

impl Player {
    // /// 按照 namerena 的原始 new
    // pub fn namer_new(base_name: String, team_name: String, sgl_name: String, weapon: String) -> Self { todo!() }

    /// 创建一个新的玩家
    pub fn new_and_init(team: Option<String>, name: String, weapon: Option<String>, storage: Arc<Storage>) -> PlayerResult<Self> {
        // 先校验长度
        if team.is_some() && team.as_ref().unwrap().len() > TEAM_MAX_LEN {
            let t = team.unwrap();
            return Err(PlayerError::TeamNameTooLong(t.len(), t.len()));
        }
        if name.len() > NAME_MAX_LEN {
            return Err(PlayerError::NameTooLong(name.len(), name.len()));
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
        // 开始处理 rc4 部分
        let name_bytes = [0_u8].iter().chain(name.as_bytes()).copied().collect::<Vec<u8>>();
        let team_bytes = [0_u8]
            .iter()
            .chain(team.as_ref().unwrap_or(&name).as_bytes())
            .copied()
            .collect::<Vec<u8>>();

        let mut rand = RC4::new(&team_bytes, 1);
        rand.update(&name_bytes, 2);

        // 生成 name_base
        let mut name_base: Vec<u8> = Vec::with_capacity(128);

        for i in 0..=255 {
            let j = (unsafe { rand.get_val_unchecked(i) } as u32 * 181 + 87) as u8;
            if j & 0x80 == 0 {
                name_base.push(j + 89);
            }
        }
        // UNWRAP SAFE: name_base.len() == 128
        let raw_name_base: [u8; 128] = name_base
            .as_slice()
            .try_into()
            .expect("unreachable(如果真到这里了就tm得好好怀疑一下自己的代码是怎么写的了)");

        // 技能顺序
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

        let mut status = PlayerStatus::default();
        if player_type == PlayerType::Seed {
            status.set_alive(false);
        }

        let id = PLAYER_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        Ok(Player {
            team,
            name,
            weapon,
            player_type,
            sort_int: 0,
            rand,
            name_base,
            raw_name_base,
            attr: [0; 8],
            skil_id: skills.clone(),
            skil_prop: skills,
            status,
            skill_store: SkillStore::new(storage.clone()),
            name_factor,
            storage,
            id,
        })
    }

    /// 获取当前的 spsum(步数)
    #[inline]
    #[deprecated(note = "请使用 move_point()")]
    pub fn sp_sum(&self) -> i32 { self.status.move_point }

    /// 获取当前的 move point (spsum)
    #[inline]
    pub fn move_point(&self) -> i32 { self.status.move_point }

    /// 设置 move point (spsum)
    #[inline]
    pub fn set_move_point(&mut self, val: i32) { self.status.move_point = val }

    /// 检查是否可以行动
    pub fn check_move(&self) -> bool { self.status.check_move() }

    pub fn check_immune(&self, _skill: SkillId, randomer: &mut RC4) -> bool {
        match self.player_type {
            PlayerType::Boost => randomer.r127() < boost_value(&self.name),
            PlayerType::Boss => {
                // TODO
                randomer.c33()
            }
            _ => false,
        }
    }

    /// 获取当前的玩家状态
    pub fn get_status(&self) -> &PlayerStatus { &self.status }

    pub fn as_ptr(&self) -> PlrPtr { self as *const Player as usize }

    pub fn id(&self) -> usize { self.id }

    /// 根据名字系数调整数值
    ///
    /// ```javascript
    /// const result = Math.round(a * (1 - this.x / b))
    /// ```
    fn scale_by_name_factor_u(&self, val: u32, factor2: u32) -> u32 {
        (val as f64 * (1.0 - self.name_factor / factor2 as f64)).round() as u32
    }

    fn scale_by_name_factor_i(&self, val: i32, factor2: i32) -> i32 {
        (val as f64 * (1.0 - self.name_factor / factor2 as f64)).round() as i32
    }

    /// upgrade 之后
    /// 计算:
    /// - 具体属性 ( 8围 )
    /// - 技能熟练度
    pub fn build(&mut self) {
        // TODO: 武器 pre upgrade
        if let Some(_weapon) = &self.weapon {
            // weapon
        }

        // init raw attr
        let mut rand_vals = [0_u8; 32];
        rand_vals.copy_from_slice(&self.rand.main_val[0..32]);
        rand_vals.get_mut(0..10).unwrap().sort_unstable();

        let mut attr = [0, 0, 0, 0, 0, 0, 0, 0];
        // 10 - 31
        // rand_vals 10~12 midle value
        // DIY TODO
        attr[0] = median(rand_vals[10], rand_vals[11], rand_vals[12]) as u32;
        attr[1] = median(rand_vals[13], rand_vals[14], rand_vals[15]) as u32;
        attr[2] = median(rand_vals[16], rand_vals[17], rand_vals[18]) as u32;
        attr[3] = median(rand_vals[19], rand_vals[20], rand_vals[21]) as u32;
        attr[4] = median(rand_vals[22], rand_vals[23], rand_vals[24]) as u32;
        attr[5] = median(rand_vals[25], rand_vals[26], rand_vals[27]) as u32;
        attr[6] = median(rand_vals[28], rand_vals[29], rand_vals[30]) as u32;
        // 7 -> rand 3 + 4 + 5 + 6
        attr[7] = rand_vals[3] as u32 + rand_vals[4] as u32 + rand_vals[5] as u32 + rand_vals[6] as u32;
        self.attr = attr;

        // init skills
        // 技能熟练度计算
        // 计算 skl_id 的已经在初始化做完了
        // DIY TODO
        for (j, i) in (64..128).step_by(4).enumerate() {
            // 取 val index ~ val index + 3 的最小值
            let small = min(
                min(self.name_base[i], self.name_base[i + 1]),
                min(self.name_base[i + 2], self.name_base[i + 3]),
            );
            if small > 10 && self.skil_id[j] < 35 {
                let mut skill = Skill::new_from_type_id((small - 10) as u32, self.skil_id[j] as u8);
                let raw_small = min(
                    min(self.raw_name_base[i], self.raw_name_base[i + 1]),
                    min(self.raw_name_base[i + 2], self.raw_name_base[i + 3]),
                );
                // 其实是懒得读取原始的last skill, 就直接按照原始代码来了
                if raw_small < 10 {
                    skill.boosted = true;
                }
                let skill_id = self.storage.as_ref().just_insert_skill(skill);
                self.skill_store.add_skill(skill_id);
            }
        }

        // TODO: 武器 post upgrade
        if let Some(_weapon) = &self.weapon {
            // weapon
        }

        // boost skills(addSkillsToProc)
        // boost最后一个
        self.skill_store.boost_last();
        // 然后是 boost passive
        if self.skill_store.skill_store.len() >= 16 {
            // 14
            let skill_14 = self
                .storage
                .just_get_skill_mut(self.skill_store.skill_store[14])
                .expect("skill 14 not found??");
            if skill_14.level() > 0 && !skill_14.boosted {
                let boost_level = min(min(self.name_base[60], self.name_base[61]) as u32, skill_14.level());
                skill_14.boost_level(boost_level);
            }
            // 15
            let skill_15 = self
                .storage
                .just_get_skill_mut(self.skill_store.skill_store[15])
                .expect("skill 15 not found??");
            if skill_15.level() > 0 && !skill_15.boosted {
                let boost_level = min(min(self.name_base[62], self.name_base[63]) as u32, skill_15.level());
                skill_15.boost_level(boost_level);
            }
        }
        // 更新 proc(其实就是缓存)
        self.skill_store.update_proc();

        self.update_states();

        // DIY TODO
    }

    /// 更新状态
    pub fn update_states(&mut self) {
        // init values
        self.status.attack = self.scale_by_name_factor_i(self.attr[0] as i32, 128);
        self.status.defense = self.scale_by_name_factor_i(self.attr[1] as i32, 128);
        self.status.speed = self.scale_by_name_factor_i(self.attr[2] as i32, 128) + 160;
        self.status.agility = self.scale_by_name_factor_i(self.attr[3] as i32, 128);
        self.status.magic = self.scale_by_name_factor_i(self.attr[4] as i32, 128);
        // 蓝条是魔法的一半
        self.status.mp = self.status.magic >> 1;
        self.status.resistance = self.scale_by_name_factor_i(self.attr[5] as i32, 128);
        self.status.wisdom = self.scale_by_name_factor_i(self.attr[6] as i32, 80);
        self.status.max_hp = self.attr[7] as i32;
        self.status.hp = self.status.max_hp;

        self.calc_attr_sum();

        self.status.at_boost = 1.0;
        self.status.set_frozen(false);
        // update state entry
        // 先设置为 mut了,以防万一
        let status = &mut self.status;
        for skill_id in self.skill_store.update_states.iter() {
            // 通过一个华丽的 unsafe 来绕过借用检查
            // rinick 我谢谢你啊
            // let slf = unsafe { &mut *(self as *const Player as *mut Player) };
            // 好家伙, 看来不需要了呢, 所有的非 status 修改都是 state 的, 不是 skill得到
            // skill.update_state(status);
            let skill = self.storage.as_ref().just_get_skill_mut(*skill_id).expect("skill not found");
            skill.update_state(status);
        }
    }

    /// 我真是谢谢您呢……
    pub fn calc_attr_sum(&mut self) {
        self.status.attr_sum = self.attr[0..7].iter().sum();
        self.status.atk_sum =
            (self.attr[0] as i32 - self.attr[1] as i32 + self.attr[2] as i32 + self.attr[4] as i32 - self.attr[5] as i32) * 2
                + self.attr[3] as i32
                + self.attr[6] as i32;
        self.status.all_sum = (self.status.attr_sum * 3) + self.attr[7];
        self.status.attract = 32768.0;
    }

    fn init_skills(&mut self) {}

    /// 同队升级
    pub fn upgrade(&mut self, other: &Self) {
        for i in 7..128 {
            if self.name_base[i] == other.name_base[i] && other.name_base[i] > self.name_base[i] {
                self.name_base[i] = other.name_base[i];
            }
        }
        if self.base_name() == self.clan_name() {
            for i in 5..128 {
                if self.name_base[i - 2] == other.name_base[i] && other.name_base[i] > self.name_base[i] {
                    self.name_base[i] = other.name_base[i];
                }
            }
        }
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
    pub fn new_from_namerena_raw(raw_name: String, storage: Arc<Storage>) -> PlayerResult<Self> {
        // 先判断是否有 + 和 @
        if !raw_name.contains("@") && !raw_name.contains("+") {
            return Player::new_and_init(None, raw_name.clone(), None, storage);
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
            Player::new_and_init(Some(team.to_string()), name.to_string(), weapon.map(|s| s.to_string()), storage)
        } else {
            // 没有队伍名, 直接是武器
            if raw_name.contains("+") {
                let (name, weapon) = raw_name.split_once("+").unwrap();
                Player::new_and_init(None, name.to_string(), Some(weapon.to_string()), storage)
            } else {
                Player::new_and_init(None, raw_name, None, storage)
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
    pub fn step(&mut self, randomer: &mut RC4, updates: &mut RunUpdates) {
        if !self.status.alive() {
            return;
        }
        let stp = self.status.speed * randomer.r3() as i32;

        // 预动作
        // todo

        self.status.move_point += stp;
        if self.check_move() {
            self.status.move_point -= MOVE_POINT_THRESHOLD;
            // 主动作
            self.action(randomer, updates);
        }
        // 结束
    }

    pub fn action(&mut self, randomer: &mut RC4, updates: &mut RunUpdates) {
        // let mut targets: Vec<_> = vec![];

        let smart = self.status.wisdom > randomer.r63() as i32;
        let req_mp = 0;

        // todo: pre action

        if self.status.frozed() {}
    }

    /// 当前玩家是否可行动
    #[inline]
    pub fn active(&self) -> bool { self.status.hp > 0 && !self.status.frozed() }
    /// 活着呢吧?
    #[inline]
    pub fn alive(&self) -> bool { self.status.alive() }

    /// 蓝条是不是够用
    pub fn mp_ready(&mut self, randomer: &mut RC4) -> bool {
        if !self.active() {
            return false;
        }
        let require_mp = randomer.r3x3() as i32;
        if self.status.mp >= require_mp {
            self.status.mp -= require_mp;
            return true;
        }
        false
    }

    // 用于兼容 namerena 的各种名字调用
    #[inline]
    pub fn id_name(&self) -> String { self.name.clone() }
    #[inline]
    pub fn display_name(&self) -> String { self.name.split(" ").next().unwrap_or_default().to_string() }
    #[inline]
    pub fn clan_name(&self) -> String { self.team.clone().unwrap_or(self.name.clone()) }
    #[inline]
    pub fn base_name(&self) -> String { self.name.clone() }

    #[inline]
    pub fn is_seed_plr(&self) -> bool { matches!(self.player_type, PlayerType::Boost) }

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
        let storage = Storage::new_arc();

        let player = Player::new_from_namerena_raw("mario".to_string(), storage.clone());
        let player = player.unwrap();
        assert_eq!(player.name, "mario");
        assert_eq!(player.team, None);
        assert_eq!(player.weapon, None);
        assert_eq!(player.player_type, PlayerType::Normal);

        let player = Player::new_from_namerena_raw("mario@red".to_string(), storage.clone());
        let player = player.unwrap();
        println!("{}", player);
        assert_eq!(player.name, "mario");
        assert_eq!(player.team, Some("red".to_string()));
        assert_eq!(player.weapon, None);
        assert_eq!(player.player_type, PlayerType::Normal);

        let player = Player::new_from_namerena_raw("mario+fire".to_string(), storage.clone());
        let player = player.unwrap();
        assert_eq!(player.name, "mario");
        assert_eq!(player.team, None);
        assert_eq!(player.weapon, Some("fire".to_string()));
        assert_eq!(player.player_type, PlayerType::Normal);

        let player = Player::new_from_namerena_raw("mario+fire+diy{xxxx}".to_string(), storage.clone());
        let player = player.unwrap();
        assert_eq!(player.name, "mario");
        assert_eq!(player.team, None);
        assert_eq!(player.weapon, Some("fire+diy{xxxx}".to_string()));
        assert_eq!(player.player_type, PlayerType::Normal);

        let player = Player::new_from_namerena_raw("mario@red+fire".to_string(), storage.clone());
        let player = player.unwrap();
        assert_eq!(player.name, "mario");
        assert_eq!(player.team, Some("red".to_string()));
        assert_eq!(player.weapon, Some("fire".to_string()));
        assert_eq!(player.player_type, PlayerType::Normal);

        let player = Player::new_from_namerena_raw("mario@red+fire+diy{xxxx}".to_string(), storage.clone());
        let player = player.unwrap();
        assert_eq!(player.name, "mario");
        assert_eq!(player.team, Some("red".to_string()));
        assert_eq!(player.weapon, Some("fire+diy{xxxx}".to_string()));
        assert_eq!(player.player_type, PlayerType::Normal);
    }

    #[test]
    fn player_name() {
        let storage = Storage::new_arc();

        let player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
        assert_eq!(player.id_name(), "aaa");
        assert_eq!(player.display_name(), "aaa");

        // 包含了 @
        let player = Player::new_from_namerena_raw("aaa@bbb".to_string(), storage.clone()).unwrap();
        assert_eq!(player.id_name(), "aaa");
        assert_eq!(player.display_name(), "aaa");

        // 空格分开的名字
        let player = Player::new_from_namerena_raw("aaa bbb".to_string(), storage.clone()).unwrap();
        assert_eq!(player.id_name(), "aaa bbb");
        assert_eq!(player.display_name(), "aaa");

        // 包含了 + 的名字
        let player = Player::new_from_namerena_raw("aaa+bbb".to_string(), storage.clone()).unwrap();
        assert_eq!(player.id_name(), "aaa");
        assert_eq!(player.display_name(), "aaa");
    }

    #[test]
    fn player_raw_types() {
        let storage = Storage::new_arc();

        let player = Player::new_from_namerena_raw("normal@normal".to_string(), storage.clone());
        let player = player.unwrap();
        assert_eq!(player.player_type, PlayerType::Normal);

        // seed
        let player = Player::new_from_namerena_raw("seed:just seed@!".to_string(), storage.clone());
        let player = player.unwrap();
        assert_eq!(player.name, "seed:just seed");
        assert_eq!(player.player_type, PlayerType::Seed);

        // testEx
        let player = Player::new_from_namerena_raw("testEx@!".to_string(), storage.clone());
        let player = player.unwrap();
        assert_eq!(player.player_type, PlayerType::TestEx);

        // test1
        let player = Player::new_from_namerena_raw("test1@\u{0002}".to_string(), storage.clone());
        let player = player.unwrap();
        assert_eq!(player.team, Some("\u{0002}".to_string()));
        assert_eq!(player.player_type, PlayerType::Test1);

        // test2
        let player = Player::new_from_namerena_raw("test2@\u{0003}".to_string(), storage.clone());
        let player = player.unwrap();
        assert_eq!(player.team, Some("\u{0003}".to_string()));
        assert_eq!(player.player_type, PlayerType::Test2);

        // boss
        let player = Player::new_from_namerena_raw("mario@!".to_string(), storage.clone());
        let player = player.unwrap();
        assert_eq!(player.player_type, PlayerType::Boss);

        // boosted
        let player = Player::new_from_namerena_raw("云剑狄卡敢@!".to_string(), storage.clone());
        let player = player.unwrap();
        assert_eq!(player.player_type, PlayerType::Boost);
    }
}

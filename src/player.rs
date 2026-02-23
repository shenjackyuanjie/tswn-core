pub mod eval_name;
pub mod skill;
pub mod utils;
pub mod weapons;

use std::any::{Any, TypeId};
use std::cmp::{Ordering, min};
use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::error::player::{PlayerError, PlayerResult};
use crate::player::skill::{Skill, SkillTargetDomain, store::SkillStorage};
use crate::rc4::RC4;
use foldhash::HashMap as FastHashMap;

/// 名字本体最大长度
pub const NAME_MAX_LEN: usize = 256;
/// 队伍名最大长度
pub const TEAM_MAX_LEN: usize = 256;

/// 大于 2048 才行动
pub const MOVE_POINT_THRESHOLD: i32 = 2048;

/// 玩家句柄（运行期唯一 ID）。
/// 为兼容旧命名仍叫 `PlrId`，但语义已从“裸指针”切到“稳定 ID”。
pub type PlrId = usize;

#[derive(Clone, Debug, Default)]
pub struct ActionTargets {
    pub enemy_alive: Vec<PlrId>,
    pub ally_alive: Vec<PlrId>,
    pub ally_all: Vec<PlrId>,
    pub ally_dead: Vec<PlrId>,
    pub all_alive: Vec<PlrId>,
}

impl ActionTargets {
    pub fn from_enemy_alive(enemy_alive: &[PlrId]) -> Self {
        Self {
            enemy_alive: enemy_alive.to_vec(),
            all_alive: enemy_alive.to_vec(),
            ..Self::default()
        }
    }
}

pub type StateTag = TypeId;

#[inline]
pub fn state_tag<T: StateTrait + 'static>() -> StateTag { TypeId::of::<T>() }

pub trait StateTrait: std::fmt::Debug + Any {
    fn meta_type(&self) -> i32 { 0 }

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn clone_box(&self) -> Box<dyn StateTrait>;
}

impl Clone for Box<dyn StateTrait> {
    fn clone(&self) -> Self { self.clone_box() }
}

/// 玩家状态容器（用于承载各种技能运行时状态）。
#[derive(Clone, Debug, Default)]
pub struct PlayerStateStore {
    states: FastHashMap<StateTag, Box<dyn StateTrait>>,
}

impl PlayerStateStore {
    #[inline]
    pub fn set<T: StateTrait + 'static>(&mut self, state: T) { self.states.insert(state_tag::<T>(), Box::new(state)); }

    #[inline]
    pub fn get<T: StateTrait + 'static>(&self) -> Option<&T> { self.states.get(&state_tag::<T>())?.as_any().downcast_ref::<T>() }

    #[inline]
    pub fn get_mut<T: StateTrait + 'static>(&mut self) -> Option<&mut T> {
        self.states.get_mut(&state_tag::<T>())?.as_any_mut().downcast_mut::<T>()
    }

    #[inline]
    pub fn has<T: StateTrait + 'static>(&self) -> bool { self.states.contains_key(&state_tag::<T>()) }

    #[inline]
    pub fn clear<T: StateTrait + 'static>(&mut self) { self.states.remove(&state_tag::<T>()); }

    #[inline]
    pub fn meta_type(&self, tag: StateTag) -> Option<i32> { self.states.get(&tag).map(|state| state.meta_type()) }

    pub fn clear_negative_states(&mut self) {
        let mut to_remove = Vec::new();
        for (tag, state) in self.states.iter() {
            if state.meta_type() < 0 {
                to_remove.push(*tag);
            }
        }
        for tag in to_remove {
            self.states.remove(&tag);
        }
    }

    pub fn clear_positive_states(&mut self) {
        let mut to_remove = Vec::new();
        for (tag, state) in self.states.iter() {
            if state.meta_type() > 0 {
                to_remove.push(*tag);
            }
        }
        for tag in to_remove {
            self.states.remove(&tag);
        }
    }

    #[inline]
    pub fn negative_state_count(&self) -> usize { self.states.values().filter(|state| state.meta_type() < 0).count() }
}

/// OnDamage 函数
///
/// 为什么 dart 里函数类型这么写的啊(恼)
///
/// ```dart
/// typedef OnDamage(Plr caster, Plr target, int dmg, R r, RunUpdates updates);
/// ```
pub type OnDamageFunc = fn(PlrId, PlrId, i32, &mut RC4, &mut RunUpdates);

fn noop_on_damage(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates) {}

/// 通过玩家句柄从存储层取可变玩家引用。
#[inline]
pub fn player_id_as_mut_plr<'a>(ptr: PlrId, storage: &'a Arc<Storage>) -> &'a mut Player {
    storage.just_get_player_mut(ptr).expect("cannot get mutable player by player handle")
}

// /// Player 的自增 ID
// pub static PLAYER_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug)]
pub struct PlayerStatus {
    /// 是否被冻结
    frozen: bool,
    /// 是否存活
    alive: bool,
    /// 分数
    point: u32,
    /// 原文: spsum
    /// > 2048 时才行动
    ///
    /// 单调递增, > 2048 时 -= 2048
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
    pub fn check_move(&self) -> bool { self.move_point > MOVE_POINT_THRESHOLD }

    pub fn set_frozen(&mut self, val: bool) { self.frozen = val }

    pub fn set_alive(&mut self, val: bool) { self.alive = val }

    pub fn set_point(&mut self, val: u32) { self.point = val }

    #[inline]
    #[deprecated(note = "self.resistance")]
    pub fn mdf(&self) -> i32 { self.resistance }

    #[inline]
    #[deprecated(note = "self.wisdom")]
    pub fn itl(&self) -> i32 { self.wisdom }
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
    /// 运行时状态（meta）
    state: PlayerStateStore,
    /// 技能相关
    skills: SkillStorage,
    /// 名字长度系数
    name_factor: f64,
    // /// store
    // pub storage: Arc<Storage>,
    /// plr id
    id: u64,
}

impl Player {
    // /// 按照 namerena 的原始 new
    // pub fn namer_new(base_name: String, team_name: String, sgl_name: String, weapon: String) -> Self { todo!() }

    /// 创建一个新的玩家
    pub fn new_and_init(team: Option<String>, name: String, weapon: Option<String>, storage: Arc<Storage>) -> PlayerResult<Self> {
        // 先校验长度
        if let Some(t) = team.as_ref()
            && t.len() > TEAM_MAX_LEN
        {
            return Err(PlayerError::TeamNameTooLong(t.len(), t.len()));
        }
        if name.len() > NAME_MAX_LEN {
            return Err(PlayerError::NameTooLong(name.len(), name.len()));
        }
        // 再校验字符
        if let Some(t) = team.as_ref()
            && t.chars().any(filter_char)
        {
            return Err(PlayerError::InvalidTextInTeam(
                t.chars().find(|&char| filter_char(char)).unwrap().to_string(),
                t.chars().position(filter_char).unwrap(),
            ));
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
            .unwrap_or_else(|_| unreachable!("unreachable(如果真到这里了就tm得好好怀疑一下自己的代码是怎么写的了)"));

        // 技能顺序
        let mut skills = (0..39).collect::<Vec<u32>>();
        rand.sort_list(&mut skills);

        let name_factor = {
            let factor_name = eval_name::eval_str_common(name.as_str(), false);
            let factor_team = match team.as_ref() {
                Some(team) => eval_name::eval_str_common(team.as_str(), false),
                None => factor_name,
            };
            factor_team.max(factor_name - 6.0)
        };

        let mut status = PlayerStatus::default();
        if player_type == PlayerType::Seed {
            status.set_alive(false);
        }

        let id = storage.new_plr_id();

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
            state: PlayerStateStore::default(),
            skills: SkillStorage::new(),
            name_factor,
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

    #[inline]
    pub fn mp(&self) -> i32 { self.status.mp }

    #[inline]
    pub fn set_mp(&mut self, val: i32) { self.status.mp = val.max(0); }

    #[inline]
    pub fn mul_at_boost(&mut self, scale: f64) { self.status.at_boost *= scale; }

    #[inline]
    pub fn mul_attract(&mut self, scale: f64) { self.status.attract *= scale; }

    #[inline]
    pub fn add_agility(&mut self, val: i32) { self.status.agility += val; }

    #[inline]
    pub fn add_defense(&mut self, val: i32) { self.status.defense += val; }

    #[inline]
    pub fn add_resistance(&mut self, val: i32) { self.status.resistance += val; }

    #[inline]
    pub fn add_attack(&mut self, val: i32) { self.status.attack += val; }

    #[inline]
    pub fn add_magic(&mut self, val: i32) { self.status.magic += val; }

    #[inline]
    pub fn add_speed(&mut self, val: i32) { self.status.speed += val; }

    #[inline]
    pub fn add_wisdom(&mut self, val: i32) { self.status.wisdom += val; }

    #[inline]
    pub fn add_max_hp(&mut self, val: i32) { self.status.max_hp += val; }

    /// 检查是否可以行动
    pub fn check_move(&self) -> bool { self.status.check_move() }

    pub fn check_immune(&self, _state: StateTag, randomer: &mut RC4) -> bool {
        match self.player_type {
            PlayerType::Boost => randomer.r127() < boost_value(&self.name),
            PlayerType::Boss => {
                let threshold: u32 = match self.name.as_str() {
                    // 高抗性 boss
                    "saitama" => 112,
                    "covid" => 104,
                    "aokiji" => 96,
                    // 默认 boss 抗性（对齐原始 c33）
                    _ => 84,
                };
                randomer.r127() < threshold
            }
            _ => false,
        }
    }

    /// 获取当前的玩家状态
    pub fn get_status(&self) -> &PlayerStatus { &self.status }

    #[inline]
    pub fn attr_sum(&self) -> i32 { self.attr.iter().map(|x| *x as i32).sum() }

    #[inline]
    pub fn negative_state_count(&self) -> usize { self.state.negative_state_count() }

    /// 获取玩家句柄（兼容旧接口名）。
    #[inline]
    pub fn as_ptr(&self) -> PlrId { self.ptr() }

    /// 获取玩家句柄（推荐新接口名）。
    #[inline]
    pub fn ptr(&self) -> PlrId { self.id.try_into().expect("player id overflow usize") }

    pub fn id(&self) -> u64 { self.id }

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
        let equipped_weapon = self.weapon.clone();
        if let Some(weapon_name) = equipped_weapon.as_deref() {
            let weapon = weapons::Weapon::from_name(weapon_name);
            weapon.pre_upgrade(self);
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
        println!("attr: {:?} {:?}", self.attr, self.rand.main_val);

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
                let mut skill = Skill::new_with_id((small - 10) as u32, self.skil_id[j] as u8);
                let raw_small = min(
                    min(self.raw_name_base[i], self.raw_name_base[i + 1]),
                    min(self.raw_name_base[i + 2], self.raw_name_base[i + 3]),
                );
                // 其实是懒得读取原始的last skill, 就直接按照原始代码来了
                if raw_small < 10 {
                    skill.boosted = true;
                }
                self.skills.add_skill(skill);
            }
        }

        if let Some(weapon_name) = equipped_weapon.as_deref() {
            let weapon = weapons::Weapon::from_name(weapon_name);
            weapon.post_upgrade(self);
        }

        // boost skills(addSkillsToProc)
        // boost最后一个
        self.skills.boost_last();
        // 然后是 boost passive
        if self.skills.skill.len() >= 16 {
            // 14
            let skill_14 = self.skills.skill_by_idx_mut(14);
            if skill_14.level() > 0 && !skill_14.boosted {
                let boost_level = min(min(self.name_base[60], self.name_base[61]) as u32, skill_14.level());
                skill_14.boost_level(boost_level);
            }
            // 15
            let skill_15 = self.skills.skill_by_idx_mut(15);
            if skill_15.level() > 0 && !skill_15.boosted {
                let boost_level = min(min(self.name_base[62], self.name_base[63]) as u32, skill_15.level());
                skill_15.boost_level(boost_level);
            }
        }
        // 更新 proc(其实就是缓存)
        self.skills.update_proc();

        self.init_values();

        // DIY TODO
    }

    /// 初始化生命/蓝条（只在 build 阶段调用一次）
    pub fn init_values(&mut self) {
        self.update_states();
        self.status.hp = self.status.max_hp;
        // Dart: mp = itl ~/ 2
        self.status.mp = self.status.wisdom >> 1;
    }

    /// 更新状态
    pub fn update_states(&mut self) {
        // init values
        self.status.attack = self.scale_by_name_factor_i(self.attr[0] as i32, 128);
        self.status.defense = self.scale_by_name_factor_i(self.attr[1] as i32, 128);
        self.status.speed = self.scale_by_name_factor_i(self.attr[2] as i32, 128) + 160;
        self.status.agility = self.scale_by_name_factor_i(self.attr[3] as i32, 128);
        self.status.magic = self.scale_by_name_factor_i(self.attr[4] as i32, 128);
        self.status.resistance = self.scale_by_name_factor_i(self.attr[5] as i32, 128);
        self.status.wisdom = self.scale_by_name_factor_i(self.attr[6] as i32, 80);
        self.status.max_hp = self.attr[7] as i32;

        self.calc_attr_sum();

        self.status.at_boost = 1.0;
        self.status.set_frozen(false);
        self.apply_update_state_effects();

        // 先设置为 mut了,以防万一
        // let status = &mut self.status;
        // for skill_idx in self.skill_store.update_states.iter() {
        // 通过一个华丽的 unsafe 来绕过借用检查
        // rinick 我谢谢你啊
        // let slf = unsafe { &mut *(self as *const Player as *mut Player) };
        // 好家伙, 看来不需要了呢, 所有的非 status 修改都是 state 的, 不是 skill得到
        // skill.update_state(status);
        // let skill = self.storage.as_ref().just_get_skill_mut(*skill_idx).expect("skill not found");
        // let skill = self.skill_store.skill_store.get(skill_idx).expect("faild to get skill from storage");
        // skill.update_state(status);
        // TODO: 我觉得这玩意不应该放在这
        // }
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

    fn init_skills(&mut self) { self.skills.update_proc(); }

    /// 同队升级
    pub fn upgrade(&mut self, other: &Self) {
        for i in 7..128 {
            if other.raw_name_base[i - 1] == self.raw_name_base[i] && other.raw_name_base[i] > self.name_base[i] {
                self.name_base[i] = other.raw_name_base[i];
            }
        }
        if self.base_name() == self.clan_name() {
            for i in 5..128 {
                if other.raw_name_base[i - 2] == self.raw_name_base[i] && other.raw_name_base[i] > self.name_base[i] {
                    self.name_base[i] = other.raw_name_base[i];
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
    pub fn update_player(&mut self) {
        self.init_skills();
        self.update_states();
    }

    /// 每回合中的玩家行动
    ///
    /// 包括 pre, main, post
    pub fn step(&mut self, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>, targets: &ActionTargets) {
        if !self.status.alive() {
            return;
        }
        self.update_states();
        let ptr = self.as_ptr();
        self.skills.update_state((ptr, randomer, updates, storage));
        let mut stp = self.status.speed * randomer.r3() as i32;
        stp = self.apply_pre_step_states(stp, updates);
        stp = self.skills.pre_step(stp, (ptr, randomer, updates, storage));
        self.status.move_point += stp;
        if self.check_move() {
            self.status.move_point -= MOVE_POINT_THRESHOLD;
            // 主动作
            self.action(randomer, updates, storage, targets);
        }
        // 结束
    }

    pub fn action(&mut self, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>, targets: &ActionTargets) {
        use crate::player::skill::berserk::BerserkState;

        let smart = self.status.wisdom > randomer.r63() as i32;
        let req_mp = randomer.r15() as i32 + 8;
        let ptr = self.as_ptr();
        let forced_skill = self.skills.pre_action(smart, (ptr, randomer, updates, storage));
        if self.status.frozed() {
            return;
        }

        let mut acted = false;
        let mut selected_skill_key: Option<usize> = forced_skill;
        if self.has_state::<BerserkState>() {
            self.default_attack(false, randomer, updates, storage, &targets.enemy_alive);
            acted = true;
        } else if selected_skill_key.is_none() && self.status.mp >= req_mp {
            self.status.mp -= req_mp;
            let skill_keys = self.skills.skill.clone();
            for key in skill_keys {
                let should_cast = {
                    let skill = self.skills.skill_by_id(key);
                    skill.level() > 0 && skill.has_action_impl() && skill.prob(smart, (ptr, randomer, updates, storage))
                };
                if should_cast {
                    selected_skill_key = Some(key);
                    break;
                }
            }
        }

        if let Some(skill_key) = selected_skill_key {
            let self_candidates = [ptr];
            let selected_targets = {
                let skill = self.skills.skill_by_id(skill_key);
                let candidates: &[PlrId] = match skill.target_domain() {
                    SkillTargetDomain::EnemyAlive => targets.enemy_alive.as_slice(),
                    SkillTargetDomain::AllyAlive => targets.ally_alive.as_slice(),
                    SkillTargetDomain::AllyAny => targets.ally_all.as_slice(),
                    SkillTargetDomain::AllyDead => targets.ally_dead.as_slice(),
                    SkillTargetDomain::SelfOnly => &self_candidates,
                    SkillTargetDomain::AllAlive => targets.all_alive.as_slice(),
                };
                skill.select_targets(candidates, smart, (ptr, randomer, updates, storage))
            };
            if !selected_targets.is_empty() {
                let skill = self.skills.skill_by_id_mut(skill_key);
                skill.act(selected_targets, smart, (ptr, randomer, updates, storage));
                acted = true;
            }
        }

        if !acted {
            self.default_attack(smart, randomer, updates, storage, &targets.enemy_alive);
        }

        let recover_threshold = (self.status.wisdom + 64).clamp(0, 127) as u32;
        if randomer.r127() < recover_threshold {
            self.status.mp += 16;
        }
        updates.add(RunUpdate::new_newline());
        self.skills.post_action((ptr, randomer, updates, storage));
        self.apply_post_action_states(randomer, updates, storage);
    }

    fn pick_target(targets: &[PlrId], randomer: &mut RC4) -> Option<PlrId> { randomer.pick(targets).map(|idx| targets[idx]) }

    fn default_attack(
        &mut self,
        smart: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
        targets: &[PlrId],
    ) {
        let Some(target_id) = Self::pick_target(targets, randomer) else {
            return;
        };

        if smart && self.status.magic > self.status.attack {
            let req_mp = (self.status.magic - self.status.attack) >> 2;
            if self.status.mp >= req_mp {
                self.status.mp -= req_mp;
                let atp = self.get_at(true, randomer);
                updates.add(RunUpdate::new("[0]发起攻击", self.as_ptr(), target_id, 0));
                storage
                    .just_get_player_mut(target_id)
                    .expect("cannot get default-attack target from storage")
                    .attacked(atp, true, self.as_ptr(), noop_on_damage, randomer, updates, storage);
                return;
            }
        }

        let atp = self.get_at(false, randomer);
        updates.add(RunUpdate::new("[0]发起攻击", self.as_ptr(), target_id, 0));
        storage
            .just_get_player_mut(target_id)
            .expect("cannot get default-attack target from storage")
            .attacked(atp, false, self.as_ptr(), noop_on_damage, randomer, updates, storage);
    }

    /// 当前玩家是否可行动
    #[inline]
    pub fn active(&self) -> bool { self.status.hp > 0 && !self.status.frozed() }
    /// 活着呢吧?
    #[inline]
    pub fn alive(&self) -> bool { self.status.alive() }

    #[inline]
    pub fn revive_with_hp(&mut self, hp: i32) {
        self.status.hp = hp.clamp(1, self.status.max_hp.max(1));
        self.status.set_alive(true);
        self.status.set_frozen(false);
    }

    #[inline]
    pub fn set_state<T: StateTrait + 'static>(&mut self, state: T) { self.state.set(state); }

    #[inline]
    pub fn get_state<T: StateTrait + 'static>(&self) -> Option<&T> { self.state.get::<T>() }

    #[inline]
    pub fn get_state_mut<T: StateTrait + 'static>(&mut self) -> Option<&mut T> { self.state.get_mut::<T>() }

    #[inline]
    pub fn has_state<T: StateTrait + 'static>(&self) -> bool { self.state.has::<T>() }

    #[inline]
    pub fn clear_state<T: StateTrait + 'static>(&mut self) { self.state.clear::<T>(); }

    #[inline]
    pub fn clear_negative_states(&mut self) { self.state.clear_negative_states(); }

    #[inline]
    pub fn clear_positive_states(&mut self) { self.state.clear_positive_states(); }

    fn apply_update_state_effects(&mut self) {
        use crate::player::skill::{curse::CurseState, haste::HasteState, ice::IceState, slow::SlowState};

        if let Some(haste) = self.get_state::<HasteState>() {
            self.status.speed *= haste.faster;
        }
        if self.has_state::<SlowState>() {
            self.status.speed /= 2;
        }
        if self.has_state::<CurseState>() {
            self.status.atk_sum *= 4;
        }
        if self.has_state::<IceState>() {
            self.status.set_frozen(true);
        }
    }

    fn apply_pre_step_states(&mut self, mut step: i32, updates: &mut RunUpdates) -> i32 {
        use crate::player::skill::ice::IceState;

        let mut clear_ice = false;
        let move_point = self.status.move_point;
        if let Some(ice) = self.get_state_mut::<IceState>()
            && step > 0
        {
            if ice.frozen_step > 0 {
                ice.frozen_step -= step;
                step = 0;
            } else if step + move_point >= MOVE_POINT_THRESHOLD {
                clear_ice = true;
                step = 0;
            }
        }
        if clear_ice {
            self.clear_state::<IceState>();
            if self.alive() {
                updates.add(RunUpdate::new_newline());
                updates.add(RunUpdate::new("[1]从[冰冻]中解除", self.as_ptr(), self.as_ptr(), 0));
            }
        }
        step
    }

    fn apply_post_action_states(&mut self, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
        use crate::player::skill::{
            berserk::BerserkState, charm::CharmState, haste::HasteState, poison::PoisonState, slow::SlowState,
        };

        let mut clear_poison = false;
        let mut clear_haste = false;
        let mut clear_slow = false;
        let mut clear_berserk = false;
        let mut clear_charm = false;
        let mut poison_tick: Option<(PlrId, i32)> = None;
        let magic = self.status.magic;

        if self.alive()
            && let Some(poison) = self.get_state_mut::<PoisonState>()
        {
            let atpp = poison.atp * (1.0 + (poison.count - 1) as f64 * 0.1) / poison.count as f64;
            poison.atp -= atpp;
            let dmg = (atpp / (magic + 64) as f64).ceil() as i32;
            poison.count -= 1;
            clear_poison = poison.count <= 0;
            poison_tick = Some((poison.caster.unwrap_or(self.as_ptr()), dmg));
        }
        if let Some((caster, dmg)) = poison_tick {
            updates.add(RunUpdate::new("[1][毒性发作]", caster, self.as_ptr(), 0));
            self.damage(dmg, caster, noop_on_damage, randomer, updates, storage);
        }
        if clear_poison {
            self.clear_state::<PoisonState>();
            if self.alive() {
                updates.add(RunUpdate::new_newline());
                updates.add(RunUpdate::new("[1]从[中毒]中解除", self.as_ptr(), self.as_ptr(), 0));
            }
        }

        if let Some(haste) = self.get_state_mut::<HasteState>() {
            haste.step -= 1;
            clear_haste = haste.step <= 0;
        }
        if clear_haste {
            self.clear_state::<HasteState>();
            if self.alive() {
                updates.add(RunUpdate::new_newline());
                updates.add(RunUpdate::new("[1]从[疾走]中解除", self.as_ptr(), self.as_ptr(), 0));
            }
        }

        if let Some(slow) = self.get_state_mut::<SlowState>() {
            slow.step -= 1;
            clear_slow = slow.step <= 0;
        }
        if clear_slow {
            self.clear_state::<SlowState>();
            if self.alive() {
                updates.add(RunUpdate::new_newline());
                updates.add(RunUpdate::new("[1]从[迟缓]中解除", self.as_ptr(), self.as_ptr(), 0));
            }
        }

        if let Some(berserk) = self.get_state_mut::<BerserkState>() {
            berserk.step -= 1;
            clear_berserk = berserk.step <= 0;
        }
        if clear_berserk {
            self.clear_state::<BerserkState>();
            if self.alive() {
                updates.add(RunUpdate::new_newline());
                updates.add(RunUpdate::new("[1]从[狂暴]中解除", self.as_ptr(), self.as_ptr(), 0));
            }
        }

        if let Some(charm) = self.get_state_mut::<CharmState>() {
            charm.step -= 1;
            clear_charm = charm.step <= 0;
        }
        if clear_charm {
            self.clear_state::<CharmState>();
            if self.alive() {
                updates.add(RunUpdate::new_newline());
                updates.add(RunUpdate::new("[1]从[魅惑]中解除", self.as_ptr(), self.as_ptr(), 0));
            }
        }
    }

    fn apply_pre_defend_states(
        &mut self,
        atp: f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> f64 {
        use crate::player::skill::protect::ProtectState;

        let target_id = self.as_ptr();
        let links = self
            .get_state::<ProtectState>()
            .map(|state| state.protect_from.clone())
            .unwrap_or_default();
        if links.is_empty() {
            return atp;
        }

        let mut stale_owners = Vec::new();
        for link in links {
            let protector_alive = storage.get_player(&link.owner).map(|p| p.alive()).unwrap_or(false);
            if !protector_alive {
                stale_owners.push(link.owner);
                continue;
            }
            if randomer.r127() >= link.level {
                continue;
            }
            let protector_ready = {
                let protector = storage.just_get_player_mut(link.owner).expect("cannot get protect owner from storage");
                protector.mp_ready(randomer)
            };
            if !protector_ready {
                continue;
            }

            updates.add(RunUpdate::new("[0][守护][1]", link.owner, target_id, 40));
            let redirected_atp = {
                let protector = storage.just_get_player_mut(link.owner).expect("cannot get protect owner from storage");
                protector.pre_defend(atp, is_mag, caster, on_damage, randomer, updates, storage)
            };
            if redirected_atp == 0.0 {
                return 0.0;
            }
            let mut redirected_dmg = {
                let protector = storage.get_player(&link.owner).expect("cannot get protect owner from storage");
                (redirected_atp * 0.5 / protector.get_df(is_mag) as f64).floor() as i32
            };
            redirected_dmg = {
                let protector = storage.just_get_player_mut(link.owner).expect("cannot get protect owner from storage");
                protector.post_defend(redirected_dmg, caster, on_damage, randomer, updates, storage)
            };
            storage
                .just_get_player_mut(link.owner)
                .expect("cannot get protect owner from storage")
                .damage(redirected_dmg, caster, on_damage, randomer, updates, storage);
            return 0.0;
        }

        if !stale_owners.is_empty() {
            let mut clear_state = false;
            if let Some(state) = self.get_state_mut::<ProtectState>() {
                state.protect_from.retain(|entry| !stale_owners.contains(&entry.owner));
                clear_state = state.protect_from.is_empty();
            }
            if clear_state {
                self.clear_state::<ProtectState>();
            }
        }

        atp
    }

    fn apply_post_defend_states(&mut self, mut dmg: i32, caster: PlrId, randomer: &mut RC4, updates: &mut RunUpdates) -> i32 {
        use crate::player::skill::{curse::CurseState, shield::ShieldState};

        if let Some(shield) = self.get_state_mut::<ShieldState>() {
            if shield.shield > 0 {
                if dmg > shield.shield {
                    dmg -= shield.shield;
                    shield.shield = 0;
                } else {
                    shield.shield -= dmg;
                    dmg = 0;
                }
            }
        }
        if dmg > 0
            && let Some(curse) = self.get_state::<CurseState>()
            && randomer.r63() < curse.prob as u32
        {
            updates.add(RunUpdate::new("[诅咒]使伤害加倍", caster, self.as_ptr(), 0));
            dmg *= curse.multiply;
        }
        dmg
    }

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

    /// getAt
    pub fn get_at(&self, use_mag: bool, randomer: &mut RC4) -> f64 {
        let atk = if use_mag { self.status.magic } else { self.status.attack };
        let a = {
            let mut temp = [
                randomer.r127() as i32,
                randomer.r127() as i32,
                randomer.r127() as i32,
                atk + 64,
                atk,
            ];
            temp.sort_unstable();
            temp[2] as f64
        };
        let b = {
            let mut temp = [randomer.r63() as i32 + 64, randomer.r63() as i32 + 64, atk + 64];
            temp.sort_unstable();
            temp[1] as f64
        };
        a * b * self.status.at_boost
    }

    /// getDf
    pub fn get_df(&self, use_mag: bool) -> i32 {
        if use_mag {
            self.status.resistance + 64
        } else {
            self.status.defense + 64
        }
    }

    pub fn dodge(al_a: i32, al_d: i32, randomer: &mut RC4) -> bool {
        let ch = {
            let temp = 24 + al_d - al_a;
            if temp < 7 {
                7
            } else if temp > 64 {
                temp / 4 + 48
            } else {
                temp
            }
        };

        randomer.next_u8() as i32 <= ch
    }

    /// preDefend
    pub fn pre_defend(
        &mut self,
        mut atp: f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> f64 {
        atp = self.apply_pre_defend_states(atp, is_mag, caster, on_damage, randomer, updates, storage);
        if atp == 0.0 {
            return 0.0;
        }
        self.skills
            .pre_defend(atp, is_mag, caster, on_damage, (self.as_ptr(), randomer, updates, storage))
    }

    /// postDefend
    pub fn post_defend(
        &mut self,
        mut dmg: i32,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> i32 {
        dmg = self.apply_post_defend_states(dmg, caster, randomer, updates);
        self.skills
            .post_defend(dmg, caster, &on_damage, (self.as_ptr(), randomer, updates, storage))
    }

    pub fn attacked(
        &mut self,
        mut atp: f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> i32 {
        atp = self.pre_defend(atp, is_mag, caster, on_damage, randomer, updates, storage);
        if atp == 0.0 {
            return 0;
        }
        let (accure, dodgeval) = {
            let caster_plr = storage.get_player(&caster).expect("faild to get caster player");
            if is_mag {
                (
                    caster_plr.status.magic + caster_plr.status.agility,
                    self.status.resistance + self.status.agility,
                )
            } else {
                (
                    caster_plr.status.attack + caster_plr.status.agility,
                    self.status.defense + self.status.agility,
                )
            }
        };
        if self.active() && Self::dodge(accure, dodgeval, randomer) {
            let update = RunUpdate::new("[0][回避]了攻击", self.as_ptr(), caster, 20);
            updates.add(update);
            return 0;
        }
        self.defned(atp, is_mag, caster, on_damage, randomer, updates, storage)
    }

    pub fn defned(
        &mut self,
        atp: f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> i32 {
        let dfp = self.get_df(is_mag);
        let mut dmg = (atp / dfp as f64).ceil() as i32;
        dmg = self.post_defend(dmg, caster, on_damage, randomer, updates, storage);
        self.damage(dmg, caster, on_damage, randomer, updates, storage)
    }

    pub fn damage(
        &mut self,
        dmg: i32,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> i32 {
        if dmg < 0 {
            let _old_hp = self.status.hp;
            self.status.hp -= dmg;
            if self.status.hp > self.status.max_hp {
                self.status.hp = self.status.max_hp;
            }
            let update = RunUpdate::new("[1]回复体力[2]点", caster, self.as_ptr(), dmg.abs() as u32);
            updates.add(update);
            return 0;
        }
        if dmg == 0 {
            let update = RunUpdate::new("[0]受到[2]点伤害[s_dmg0]", self.as_ptr(), self.as_ptr(), 10);
            updates.add(update);
            return 0;
        }
        let old_hp = self.status.hp;
        self.status.hp -= dmg;
        if self.status.hp < 0 {
            self.status.hp = 0;
        }
        let mut msg = "[0]受到[2]点伤害".to_string();
        if dmg >= 160 {
            msg.push_str("[s_dmg160]");
        } else if dmg >= 120 {
            msg.push_str("[s_dmg120]");
        }
        let mut update = RunUpdate::new(msg, caster, self.as_ptr(), dmg as u32);
        update.delay0 = if dmg > 250 { 1500 } else { 1000 + dmg * 2 };
        updates.add(update);
        on_damage(caster, self.as_ptr(), dmg, randomer, updates);
        self.on_damaged(dmg, old_hp, caster, randomer, updates, storage)
    }

    pub fn on_damaged(
        &mut self,
        dmg: i32,
        old_hp: i32,
        caster: PlrId,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> i32 {
        let post_damaged_indices: Vec<_> = self.skills.post_damage.iter().cloned().collect();
        for skill_idx in post_damaged_indices {
            let ptr = self.as_ptr();
            let skill = self.skills.skill_by_id_mut(skill_idx);
            skill.post_damage(dmg, caster, (ptr, randomer, updates, storage));
        }
        if self.status.hp <= 0 {
            if caster != self.as_ptr()
                && let Some(killer) = storage.just_get_player_mut(caster)
            {
                killer.skills.kill(self.as_ptr(), (caster, randomer, updates, storage));
            }
            self.on_die(old_hp, caster, randomer, updates, storage);
            return old_hp;
        } else {
            return dmg;
        }
    }

    fn get_die_message(&self) -> &'static str { "[1]被击倒了" }

    pub fn on_die(&mut self, old_hp: i32, caster: PlrId, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
        use crate::player::skill::act::minion::MinionRuntimeState;

        if self.status.hp > 0 {
            return;
        }

        updates.add(RunUpdate::new_newline());
        updates.add(RunUpdate::new(self.get_die_message(), caster, self.as_ptr(), 50));

        let ptr = self.as_ptr();
        self.skills.die(old_hp, caster, (ptr, randomer, updates, storage));
        if self.status.hp > 0 {
            return;
        }
        self.status.hp = 0;
        self.status.set_alive(false);

        let owner_id = self.as_ptr();
        let linked_minions = storage
            .all_player_ids()
            .into_iter()
            .filter(|id| *id != owner_id)
            .filter(|id| {
                storage
                    .get_player(id)
                    .and_then(|player| player.get_state::<MinionRuntimeState>())
                    .map(|state| state.owner == Some(owner_id))
                    .unwrap_or(false)
            })
            .collect::<Vec<PlrId>>();
        for minion_id in linked_minions {
            if let Some(minion) = storage.just_get_player_mut(minion_id)
                && minion.alive()
            {
                minion.status.hp = 0;
                minion.status.set_alive(false);
                updates.add(RunUpdate::new_newline());
                updates.add(RunUpdate::new("[1]消失了", owner_id, minion_id, 30));
            }
            storage.queue_remove_player(minion_id);
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
    use crate::engine::update::UpdateType;

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

    fn noop_on_damage(_: PlrId, _: PlrId, _: i32, _: &mut RC4, _: &mut RunUpdates) {}

    #[test]
    fn check_move_threshold_matches_dart() {
        let mut status = PlayerStatus::default();
        status.move_point = MOVE_POINT_THRESHOLD;
        assert!(!status.check_move());
        status.move_point = MOVE_POINT_THRESHOLD + 1;
        assert!(status.check_move());
    }

    #[test]
    fn update_states_does_not_reset_hp_or_mp() {
        let storage = Storage::new_arc();
        let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();

        player.attr = [10, 20, 30, 40, 50, 60, 70, 80];
        player.init_values();
        assert_eq!(player.status.hp, player.status.max_hp);
        assert_eq!(player.status.mp, player.status.wisdom >> 1);

        player.status.hp = 11;
        player.status.mp = 22;
        player.update_states();

        assert_eq!(player.status.hp, 11);
        assert_eq!(player.status.mp, 22);
    }

    #[test]
    fn upgrade_uses_other_raw_name_base_rules() {
        let storage = Storage::new_arc();
        let mut lhs = Player::new_from_namerena_raw("lhs".to_string(), storage.clone()).unwrap();
        let mut rhs = Player::new_from_namerena_raw("rhs".to_string(), storage.clone()).unwrap();

        lhs.name_base = vec![0; 128];
        rhs.name_base = vec![0; 128];
        lhs.raw_name_base = [0; 128];
        rhs.raw_name_base = [0; 128];

        lhs.raw_name_base[10] = 42;
        lhs.name_base[10] = 50;
        rhs.raw_name_base[9] = 42;
        rhs.raw_name_base[10] = 99;
        rhs.name_base[10] = 1;

        lhs.raw_name_base[8] = 77;
        lhs.name_base[8] = 10;
        rhs.raw_name_base[6] = 77;
        rhs.raw_name_base[8] = 88;
        rhs.name_base[8] = 2;

        lhs.upgrade(&rhs);

        assert_eq!(lhs.name_base[10], 99);
        assert_eq!(lhs.name_base[8], 88);
    }

    #[test]
    fn damage_update_uses_caster_as_actor() {
        let storage = Storage::new_arc();
        let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
        let caster: PlrId = 999;
        let mut randomer = RC4::default();
        let mut updates = RunUpdates::new();

        player.status.max_hp = 100;
        player.status.hp = 100;
        let result = player.damage(7, caster, noop_on_damage, &mut randomer, &mut updates, &storage);

        assert_eq!(result, 7);
        assert!(!updates.updates.is_empty());
        let update = updates.updates.last().unwrap();
        assert_eq!(update.caster, caster);
        assert_eq!(update.target, player.as_ptr());
    }

    #[test]
    fn on_damaged_triggers_on_die() {
        let storage = Storage::new_arc();
        let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
        let mut randomer = RC4::default();
        let mut updates = RunUpdates::new();

        player.status.hp = 0;
        player.status.set_alive(true);

        let old_hp = 7;
        let result = player.on_damaged(7, old_hp, player.as_ptr(), &mut randomer, &mut updates, &storage);

        assert_eq!(result, old_hp);
        assert!(!player.status.alive());
        assert_eq!(player.status.hp, 0);
        assert_eq!(updates.updates.len(), 2);
        assert!(matches!(updates.updates[0].update_type, UpdateType::NextLine));
        assert_eq!(updates.updates[1].message, "[1]被击倒了");
    }

    #[test]
    fn check_immune_matches_player_type_rules() {
        let storage = Storage::new_arc();
        let boost = Player::new_from_namerena_raw("云剑穸跄祇@!".to_string(), storage.clone()).unwrap();
        let normal = Player::new_from_namerena_raw("normal".to_string(), storage.clone()).unwrap();
        let mut randomer = RC4 {
            i: 0,
            j: 0,
            main_val: vec![0; 256],
        };

        assert!(boost.check_immune(state_tag::<crate::player::skill::poison::PoisonState>(), &mut randomer));
        assert!(!normal.check_immune(state_tag::<crate::player::skill::poison::PoisonState>(), &mut randomer));
    }

    #[test]
    fn update_states_applies_haste_slow_and_ice_effects() {
        let storage = Storage::new_arc();
        let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
        let ptr = player.as_ptr();
        player.attr = [10, 10, 10, 10, 10, 10, 10, 100];
        player.update_states();
        let base_speed = player.get_status().speed;

        player.set_state(crate::player::skill::haste::HasteState {
            owner: Some(ptr),
            target: Some(ptr),
            on_post_action: None,
            faster: 2,
            step: 3,
        });
        player.update_states();
        assert_eq!(player.get_status().speed, base_speed * 2);

        player.clear_state::<crate::player::skill::haste::HasteState>();
        player.set_state(crate::player::skill::slow::SlowState {
            owner: Some(ptr),
            target: Some(ptr),
            on_post_action: None,
            step: 2,
        });
        player.update_states();
        assert_eq!(player.get_status().speed, base_speed / 2);

        player.set_state(crate::player::skill::ice::IceState {
            target: Some(ptr),
            pre_step_impl: None,
            frozen_step: 1024,
        });
        player.update_states();
        assert!(player.get_status().frozed());
    }

    #[test]
    fn clear_negative_states_keeps_positive_states() {
        let storage = Storage::new_arc();
        let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
        let ptr = player.as_ptr();

        player.set_state(crate::player::skill::haste::HasteState {
            owner: Some(ptr),
            target: Some(ptr),
            on_post_action: None,
            faster: 2,
            step: 3,
        });
        player.set_state(crate::player::skill::slow::SlowState {
            owner: Some(ptr),
            target: Some(ptr),
            on_post_action: None,
            step: 2,
        });
        player.set_state(crate::player::skill::poison::PoisonState {
            caster: Some(ptr),
            target: Some(ptr),
            atp: 12.0,
            count: 4,
        });
        player.clear_negative_states();

        assert!(player.has_state::<crate::player::skill::haste::HasteState>());
        assert!(!player.has_state::<crate::player::skill::slow::SlowState>());
        assert!(!player.has_state::<crate::player::skill::poison::PoisonState>());
    }

    #[test]
    fn ice_state_pre_step_expires_with_threshold_check() {
        let storage = Storage::new_arc();
        let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
        let ptr = player.as_ptr();
        let mut updates = RunUpdates::new();

        player.set_move_point(2000);
        player.set_state(crate::player::skill::ice::IceState {
            target: Some(ptr),
            pre_step_impl: None,
            frozen_step: 0,
        });
        let step = player.apply_pre_step_states(100, &mut updates);
        assert_eq!(step, 0);
        assert!(!player.has_state::<crate::player::skill::ice::IceState>());
        assert!(updates.updates.iter().any(|x| x.message.contains("冰冻")));
    }

    #[test]
    fn poison_state_ticks_and_expires_in_post_action() {
        let storage = Storage::new_arc();
        let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
        let ptr = player.as_ptr();
        let mut randomer = RC4::default();
        let mut updates = RunUpdates::new();

        player.status.max_hp = 100;
        player.status.hp = 100;
        player.status.magic = 32;
        player.set_state(crate::player::skill::poison::PoisonState {
            caster: Some(ptr),
            target: Some(ptr),
            atp: 80.0,
            count: 1,
        });
        player.action(&mut randomer, &mut updates, &storage, &ActionTargets::default());

        assert!(!player.has_state::<crate::player::skill::poison::PoisonState>());
        assert!(updates.updates.iter().any(|x| x.message.contains("[毒性发作]")));
        assert!(updates.updates.iter().any(|x| x.message.contains("从[中毒]中解除")));
    }

    #[test]
    fn post_defend_consumes_shield_state() {
        let storage = Storage::new_arc();
        let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
        let mut randomer = RC4::default();
        let mut updates = RunUpdates::new();

        player.set_state(crate::player::skill::shield::ShieldState {
            sort_id: 6000.0,
            target: Some(player.as_ptr()),
            shield: 10,
        });
        let dmg = player.post_defend(6, 999, noop_on_damage, &mut randomer, &mut updates, &storage);
        assert_eq!(dmg, 0);
        assert_eq!(
            player.get_state::<crate::player::skill::shield::ShieldState>().unwrap().shield,
            4
        );
    }

    #[test]
    fn post_defend_applies_curse_multiplier() {
        let storage = Storage::new_arc();
        let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
        let mut randomer = RC4 {
            i: 0,
            j: 0,
            main_val: vec![0; 256],
        };
        let mut updates = RunUpdates::new();

        player.set_state(crate::player::skill::curse::CurseState {
            owner: Some(777),
            target: Some(player.as_ptr()),
            on_update_state: None,
            prob: 42,
            multiply: 2,
        });
        let dmg = player.post_defend(5, 777, noop_on_damage, &mut randomer, &mut updates, &storage);
        assert_eq!(dmg, 10);
        assert!(updates.updates.iter().any(|x| x.message.contains("诅咒")));
    }

    #[test]
    fn action_expires_berserk_and_charm_states() {
        let storage = Storage::new_arc();
        let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
        let mut randomer = RC4::default();
        let mut updates = RunUpdates::new();

        player.set_state(crate::player::skill::berserk::BerserkState { step: 1 });
        player.set_state(crate::player::skill::charm::CharmState {
            group_id: 1,
            target: Some(player.as_ptr()),
            on_post_action: None,
            step: 1,
        });
        player.action(&mut randomer, &mut updates, &storage, &ActionTargets::default());

        assert!(!player.has_state::<crate::player::skill::berserk::BerserkState>());
        assert!(!player.has_state::<crate::player::skill::charm::CharmState>());
        assert!(updates.updates.iter().any(|x| x.message.contains("狂暴")));
        assert!(updates.updates.iter().any(|x| x.message.contains("魅惑")));
    }

    #[test]
    fn merge_and_zombie_kill_write_target_states() {
        let storage = Storage::new_arc();
        let target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
        let target_id = storage.just_insert_player(target);
        let mut randomer = RC4 {
            i: 0,
            j: 0,
            main_val: vec![0; 256],
        };
        let mut updates = RunUpdates::new();
        let mut merge = crate::player::skill::merge::MergeSkill::new();
        let mut zombie = crate::player::skill::zombie::ZombieSkill::new();

        let merged = <crate::player::skill::merge::MergeSkill as crate::player::skill::SkillTrait>::kill(
            &mut merge,
            target_id,
            (7, &mut randomer, &mut updates, &storage),
        );
        let zombied = <crate::player::skill::zombie::ZombieSkill as crate::player::skill::SkillTrait>::kill(
            &mut zombie,
            target_id,
            (7, &mut randomer, &mut updates, &storage),
        );
        assert!(merged);
        assert!(zombied);
        let target_ref = storage.get_player(&target_id).unwrap();
        assert!(target_ref.has_state::<crate::player::skill::merge::MergeState>());
        assert!(target_ref.has_state::<crate::player::skill::zombie::ZombieState>());
    }

    #[test]
    fn action_uses_fire_skill_when_available() {
        let storage = Storage::new_arc();
        let attacker = Player::new_from_namerena_raw("attacker".to_string(), storage.clone()).unwrap();
        let target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
        let attacker_id = storage.just_insert_player(attacker);
        let target_id = storage.just_insert_player(target);
        let mut randomer = RC4::default();
        let mut updates = RunUpdates::new();

        let attacker_mut = storage.just_get_player_mut(attacker_id).unwrap();
        attacker_mut.status.hp = 100;
        attacker_mut.status.max_hp = 100;
        attacker_mut.status.mp = 999;
        attacker_mut.skills.add_skill(Skill::new_with_id(255, 0));
        attacker_mut.skills.update_proc();
        let target_mut = storage.just_get_player_mut(target_id).unwrap();
        target_mut.status.hp = 100;
        target_mut.status.max_hp = 100;

        attacker_mut.action(
            &mut randomer,
            &mut updates,
            &storage,
            &ActionTargets::from_enemy_alive(&[target_id]),
        );
        assert!(updates.updates.iter().any(|x| x.message.contains("火球术")));
    }

    #[test]
    fn heal_action_targets_injured_ally() {
        let storage = Storage::new_arc();
        let healer = Player::new_from_namerena_raw("healer@red".to_string(), storage.clone()).unwrap();
        let ally = Player::new_from_namerena_raw("ally@red".to_string(), storage.clone()).unwrap();
        let enemy = Player::new_from_namerena_raw("enemy@blue".to_string(), storage.clone()).unwrap();
        let healer_id = storage.just_insert_player(healer);
        let ally_id = storage.just_insert_player(ally);
        let enemy_id = storage.just_insert_player(enemy);
        let mut randomer = RC4::default();
        let mut updates = RunUpdates::new();

        let healer_mut = storage.just_get_player_mut(healer_id).unwrap();
        healer_mut.status.mp = 999;
        healer_mut.skills.add_skill(Skill::new_with_id(255, 15));
        healer_mut.skills.update_proc();
        let ally_mut = storage.just_get_player_mut(ally_id).unwrap();
        ally_mut.status.max_hp = 240;
        ally_mut.status.hp = 40;
        let old_ally_hp = ally_mut.status.hp;

        let targets = ActionTargets {
            enemy_alive: vec![enemy_id],
            ally_alive: vec![healer_id, ally_id],
            ally_all: vec![healer_id, ally_id],
            ally_dead: vec![],
            all_alive: vec![healer_id, ally_id, enemy_id],
        };
        healer_mut.action(&mut randomer, &mut updates, &storage, &targets);

        let healed_hp = storage.get_player(&ally_id).unwrap().get_status().hp;
        assert!(healed_hp > old_ally_hp);
        assert!(updates.updates.iter().any(|u| u.message.contains("治愈魔法") && u.target == ally_id));
    }

    #[test]
    fn revive_action_targets_dead_ally() {
        let storage = Storage::new_arc();
        let healer = Player::new_from_namerena_raw("reviver@red".to_string(), storage.clone()).unwrap();
        let ally = Player::new_from_namerena_raw("corpse@red".to_string(), storage.clone()).unwrap();
        let enemy = Player::new_from_namerena_raw("enemy@blue".to_string(), storage.clone()).unwrap();
        let healer_id = storage.just_insert_player(healer);
        let ally_id = storage.just_insert_player(ally);
        let enemy_id = storage.just_insert_player(enemy);
        let mut randomer = RC4::default();
        let mut updates = RunUpdates::new();

        let healer_mut = storage.just_get_player_mut(healer_id).unwrap();
        healer_mut.status.mp = 999;
        healer_mut.skills.add_skill(Skill::new_with_id(255, 16));
        healer_mut.skills.update_proc();
        let ally_mut = storage.just_get_player_mut(ally_id).unwrap();
        ally_mut.status.max_hp = 200;
        ally_mut.status.hp = 0;
        ally_mut.status.set_alive(false);

        let targets = ActionTargets {
            enemy_alive: vec![enemy_id],
            ally_alive: vec![healer_id],
            ally_all: vec![healer_id, ally_id],
            ally_dead: vec![ally_id],
            all_alive: vec![healer_id, enemy_id],
        };
        healer_mut.action(&mut randomer, &mut updates, &storage, &targets);

        let revived = storage.get_player(&ally_id).unwrap();
        assert!(revived.alive());
        assert!(revived.get_status().hp > 0);
        assert!(updates.updates.iter().any(|u| u.message.contains("苏生术") && u.target == ally_id));
    }

    #[test]
    fn protect_redirects_damage_to_protector() {
        let storage = Storage::new_arc();
        let protector = Player::new_from_namerena_raw("protector@red".to_string(), storage.clone()).unwrap();
        let ally = Player::new_from_namerena_raw("ally@red".to_string(), storage.clone()).unwrap();
        let enemy = Player::new_from_namerena_raw("enemy@blue".to_string(), storage.clone()).unwrap();
        let protector_id = storage.just_insert_player(protector);
        let ally_id = storage.just_insert_player(ally);
        let enemy_id = storage.just_insert_player(enemy);
        let mut randomer = RC4::default();
        let mut updates = RunUpdates::new();

        let protector_mut = storage.just_get_player_mut(protector_id).unwrap();
        protector_mut.status.mp = 999;
        protector_mut.status.hp = 300;
        protector_mut.status.max_hp = 300;
        protector_mut.skills.add_skill(Skill::new_with_id(255, 26));
        protector_mut.skills.update_proc();
        let ally_mut = storage.just_get_player_mut(ally_id).unwrap();
        ally_mut.status.hp = 280;
        ally_mut.status.max_hp = 280;

        let targets = ActionTargets {
            enemy_alive: vec![enemy_id],
            ally_alive: vec![protector_id, ally_id],
            ally_all: vec![protector_id, ally_id],
            ally_dead: vec![],
            all_alive: vec![protector_id, ally_id, enemy_id],
        };
        protector_mut.action(&mut randomer, &mut updates, &storage, &targets);
        assert!(
            storage
                .get_player(&ally_id)
                .unwrap()
                .has_state::<crate::player::skill::protect::ProtectState>()
        );

        let protector_hp_before = storage.get_player(&protector_id).unwrap().get_status().hp;
        let ally_hp_before = storage.get_player(&ally_id).unwrap().get_status().hp;
        let mut damage_updates = RunUpdates::new();
        storage.just_get_player_mut(ally_id).unwrap().attacked(
            260.0,
            false,
            enemy_id,
            noop_on_damage,
            &mut randomer,
            &mut damage_updates,
            &storage,
        );

        let protector_hp_after = storage.get_player(&protector_id).unwrap().get_status().hp;
        let ally_hp_after = storage.get_player(&ally_id).unwrap().get_status().hp;
        assert!(protector_hp_after < protector_hp_before);
        assert_eq!(ally_hp_after, ally_hp_before);
        assert!(damage_updates.updates.iter().any(|u| u.message.contains("[守护]")));
    }

    #[test]
    fn action_falls_back_to_default_attack() {
        let storage = Storage::new_arc();
        let attacker = Player::new_from_namerena_raw("attacker".to_string(), storage.clone()).unwrap();
        let target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
        let attacker_id = storage.just_insert_player(attacker);
        let target_id = storage.just_insert_player(target);
        let mut randomer = RC4::default();
        let mut updates = RunUpdates::new();

        let attacker_mut = storage.just_get_player_mut(attacker_id).unwrap();
        attacker_mut.status.hp = 100;
        attacker_mut.status.max_hp = 100;
        attacker_mut.status.mp = 999;
        let target_mut = storage.just_get_player_mut(target_id).unwrap();
        target_mut.status.hp = 100;
        target_mut.status.max_hp = 100;

        attacker_mut.action(
            &mut randomer,
            &mut updates,
            &storage,
            &ActionTargets::from_enemy_alive(&[target_id]),
        );
        assert!(updates.updates.iter().any(|x| x.message.contains("发起攻击")));
    }

    #[test]
    fn reraise_skill_prevents_death() {
        let storage = Storage::new_arc();
        let caster = Player::new_from_namerena_raw("caster".to_string(), storage.clone()).unwrap();
        let target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
        let caster_id = storage.just_insert_player(caster);
        let target_id = storage.just_insert_player(target);
        let mut randomer = RC4 {
            i: 0,
            j: 0,
            main_val: vec![0; 256],
        };
        let mut updates = RunUpdates::new();

        let target_mut = storage.just_get_player_mut(target_id).unwrap();
        target_mut.status.hp = 20;
        target_mut.status.max_hp = 100;
        target_mut.skills.add_skill(Skill::new_with_id(255, 28));
        target_mut.skills.update_proc();

        target_mut.damage(120, caster_id, noop_on_damage, &mut randomer, &mut updates, &storage);
        assert!(target_mut.alive());
        assert!(target_mut.get_status().hp > 0);
        assert!(updates.updates.iter().any(|x| x.message.contains("护身符")));
    }

    #[test]
    fn assassinate_preaction_forces_backstab() {
        let storage = Storage::new_arc();
        let attacker = Player::new_from_namerena_raw("attacker".to_string(), storage.clone()).unwrap();
        let target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
        let attacker_id = storage.just_insert_player(attacker);
        let target_id = storage.just_insert_player(target);
        let mut randomer = RC4::default();
        let mut updates = RunUpdates::new();

        let attacker_mut = storage.just_get_player_mut(attacker_id).unwrap();
        attacker_mut.status.hp = 120;
        attacker_mut.status.max_hp = 120;
        attacker_mut.status.mp = 999;
        attacker_mut.skills.add_skill(Skill::new_with_id(255, 21));
        attacker_mut.skills.update_proc();

        attacker_mut.action(
            &mut randomer,
            &mut updates,
            &storage,
            &ActionTargets::from_enemy_alive(&[target_id]),
        );
        assert!(updates.updates.iter().any(|x| x.message.contains("潜行")));

        let mut updates2 = RunUpdates::new();
        attacker_mut.action(
            &mut randomer,
            &mut updates2,
            &storage,
            &ActionTargets::from_enemy_alive(&[target_id]),
        );
        assert!(updates2.updates.iter().any(|x| x.message.contains("背刺")));
    }

    #[test]
    fn damage_marks_high_damage_thresholds() {
        let storage = Storage::new_arc();
        let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
        let mut randomer = RC4::default();
        let mut updates = RunUpdates::new();
        player.status.hp = 500;
        player.status.max_hp = 500;

        player.damage(130, player.as_ptr(), noop_on_damage, &mut randomer, &mut updates, &storage);
        let hit120 = updates.updates.last().expect("120 damage update missing");
        assert!(hit120.message.contains("s_dmg120"));
        assert_eq!(hit120.delay0, 1260);

        player.status.hp = 500;
        updates.updates.clear();
        player.damage(170, player.as_ptr(), noop_on_damage, &mut randomer, &mut updates, &storage);
        let hit160 = updates.updates.last().expect("160 damage update missing");
        assert!(hit160.message.contains("s_dmg160"));
        assert_eq!(hit160.delay0, 1340);
    }

    #[test]
    fn build_applies_s11_weapon_bonus() {
        let storage = Storage::new_arc();
        let mut base = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
        let mut with_weapon = Player::new_from_namerena_raw("aaa+剁手刀".to_string(), storage.clone()).unwrap();
        base.build();
        with_weapon.build();
        assert_eq!(with_weapon.attr[0], base.attr[0] + 11);
        assert_eq!(with_weapon.attr[2], base.attr[2] + 11);
    }

    #[test]
    fn boss_has_higher_state_immunity() {
        let storage = Storage::new_arc();
        let boss = Player::new_from_namerena_raw("saitama@!".to_string(), storage.clone()).unwrap();
        let normal = Player::new_from_namerena_raw("normal".to_string(), storage.clone()).unwrap();
        let mut randomer = RC4 {
            i: 0,
            j: 0,
            main_val: vec![0; 256],
        };
        assert!(boss.check_immune(state_tag::<crate::player::skill::fire::FireState>(), &mut randomer));
        assert!(!normal.check_immune(state_tag::<crate::player::skill::fire::FireState>(), &mut randomer));
    }

    #[test]
    fn merge_kill_applies_owner_growth() {
        let storage = Storage::new_arc();
        let owner = Player::new_from_namerena_raw("owner".to_string(), storage.clone()).unwrap();
        let target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
        let owner_id = storage.just_insert_player(owner);
        let target_id = storage.just_insert_player(target);
        let mut randomer = RC4 {
            i: 0,
            j: 0,
            main_val: vec![0; 256],
        };
        let mut updates = RunUpdates::new();

        {
            let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
            owner_mut.status.hp = 200;
            owner_mut.status.max_hp = 200;
            owner_mut.skills.add_skill(Skill::new_with_id(255, 31));
            owner_mut.skills.update_proc();
            owner_mut.update_states();
            owner_mut.skills.update_state((owner_id, &mut randomer, &mut updates, &storage));
        }
        let base_attack = storage.get_player(&owner_id).unwrap().get_status().attack;

        {
            let target_mut = storage.just_get_player_mut(target_id).unwrap();
            target_mut.status.hp = 120;
            target_mut.status.max_hp = 240;
            target_mut.attr = [90, 80, 170, 70, 75, 65, 60, 240];
            target_mut.status.attack = 90;
            target_mut.status.defense = 80;
            target_mut.status.speed = 170;
            target_mut.status.agility = 70;
            target_mut.status.magic = 75;
            target_mut.status.resistance = 65;
            target_mut.status.wisdom = 60;
            target_mut.status.mp = 64;
            target_mut.status.move_point = 512;
            target_mut.status.set_alive(true);
        }

        storage.just_get_player_mut(target_id).unwrap().damage(
            999,
            owner_id,
            noop_on_damage,
            &mut randomer,
            &mut updates,
            &storage,
        );

        {
            let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
            owner_mut.update_states();
            owner_mut.skills.update_state((owner_id, &mut randomer, &mut updates, &storage));
            assert!(owner_mut.get_status().attack > base_attack);
            assert!(owner_mut.has_state::<crate::player::skill::merge::MergeState>());
        }
    }

    #[test]
    fn zombie_kill_marks_corpse_and_queues_minion_spawn() {
        let storage = Storage::new_arc();
        let owner = Player::new_from_namerena_raw("owner".to_string(), storage.clone()).unwrap();
        let target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
        let owner_id = storage.just_insert_player(owner);
        let target_id = storage.just_insert_player(target);
        let mut randomer = RC4 {
            i: 0,
            j: 0,
            main_val: vec![0; 256],
        };
        let mut updates = RunUpdates::new();

        {
            let owner_mut = storage.just_get_player_mut(owner_id).unwrap();
            owner_mut.status.hp = 160;
            owner_mut.status.max_hp = 160;
            owner_mut.skills.add_skill(Skill::new_with_id(255, 32));
            owner_mut.skills.update_proc();
            owner_mut.status.mp = 999;
        }

        {
            let target_mut = storage.just_get_player_mut(target_id).unwrap();
            target_mut.status.hp = 100;
            target_mut.status.max_hp = 200;
            target_mut.status.wisdom = 80;
            target_mut.status.set_alive(true);
        }

        storage.just_get_player_mut(target_id).unwrap().damage(
            999,
            owner_id,
            noop_on_damage,
            &mut randomer,
            &mut updates,
            &storage,
        );

        {
            let target_mut = storage.just_get_player_mut(target_id).unwrap();
            assert!(!target_mut.alive());
            assert_eq!(target_mut.get_status().hp, 0);
            assert!(target_mut.has_state::<crate::player::skill::zombie::ZombieState>());
        }
        let pending = storage.take_pending_spawns();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].owner, owner_id);
        assert!(pending[0].player.has_state::<crate::player::skill::act::minion::MinionRuntimeState>());
        assert!(updates.updates.iter().any(|x| x.message.contains("变成了")));
    }

    #[test]
    fn owner_death_marks_linked_minion_for_cleanup() {
        let storage = Storage::new_arc();
        let owner = Player::new_from_namerena_raw("owner".to_string(), storage.clone()).unwrap();
        let owner_id = storage.just_insert_player(owner);
        let mut minion = storage.get_player(&owner_id).expect("cannot get owner").clone();
        minion.id = storage.new_plr_id();
        minion.name = "owner?m".to_string();
        minion.set_state(crate::player::skill::act::minion::MinionRuntimeState {
            owner: Some(owner_id),
            kind: crate::player::skill::act::minion::MinionKind::Clone,
        });
        let minion_id = storage.just_insert_player(minion);
        let mut randomer = RC4 {
            i: 0,
            j: 0,
            main_val: vec![0; 256],
        };
        let mut updates = RunUpdates::new();

        storage.just_get_player_mut(owner_id).unwrap().damage(
            999,
            owner_id,
            noop_on_damage,
            &mut randomer,
            &mut updates,
            &storage,
        );

        assert!(!storage.get_player(&minion_id).expect("minion should exist").alive());
        let pending_remove = storage.take_pending_remove_players();
        assert!(pending_remove.contains(&minion_id));
    }
}

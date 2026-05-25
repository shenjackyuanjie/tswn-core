//! # 玩家构造 (impl_ctor)
//!
//! 本模块实现 [`Player`] 的构造函数和初始化逻辑。
//!
//! ## 功能说明
//!
//! - **创建玩家** — `new_and_init()` 创建新玩家并初始化
//! - **从原始数据创建** — `new_from_namerena_raw()` 从 namerena 原始数据创建玩家
//! - **克隆玩家** — `new_from_clone()` 创建玩家克隆
//! - **Boss 初始化** — 各种 Boss 的专用初始化函数
//!
//! ## 构造流程
//!
//! 1. **参数校验** — 校验队名、玩家名称长度和字符
//! 2. **初始化状态** — 创建初始的 [`PlayerStatus`]
//! 3. **计算名字系数** — 根据名字计算 name_factor
//! 4. **初始化武器** — 解析武器名称并计算武器效果
//! 5. **初始化技能** — 根据名字和武器初始化技能
//! 6. **构建属性** — 计算八围和技能熟练度
//!
//! ## Boss 初始化
//!
//! 为各种 Boss 提供专用初始化函数：
//! - `new_boss_covid()` — 新冠病毒 Boss
//! - `new_boss_lazy()` — 懒癌 Boss
//! - `new_boss_saitama()` — 一拳超人 Boss
//! - `new_boss_mario()` — 马里奥 Boss
//! - `new_boss_slime()` — 史莱姆 Boss
//! - `new_boss_sonic()` — 索尼克 Boss
//! - `new_boss_yuri()` — 尤里 Boss
//!
//! ## 示例
//!
//! ```rust,ignore
//! use tswn_core::player::Player;
//! use std::sync::Arc;
//! use tswn_core::engine::storage::Storage;
//!
//! let storage = Arc::new(Storage::default());
//! let player = Player::new_and_init(
//!     Some("TeamA".to_string()),
//!     "PlayerName".to_string(),
//!     Some("WeaponName".to_string()),
//!     storage
//! ).unwrap();
//! ```

use super::*;

impl Player {
    // /// 按照 namerena 的原始 new
    // pub fn namer_new(base_name: String, team_name: String, sgl_name: String, weapon: String) -> Self { todo!() }

    pub(crate) fn normal_raw_name_base(team: Option<&str>, name: &str) -> [u8; 128] {
        let name_bytes = [0_u8].iter().chain(name.as_bytes()).copied().collect::<Vec<u8>>();
        let team_bytes = [0_u8].iter().chain(team.unwrap_or(name).as_bytes()).copied().collect::<Vec<u8>>();

        let mut rand = RC4::new(&team_bytes, 1);
        rand.update(&name_bytes, 2);

        let mut name_base: Vec<u8> = Vec::with_capacity(128);
        for i in 0..=255 {
            let m = ((unsafe { rand.get_val_unchecked(i) } as u32 * 181) + 160) % 256;
            if (89..217).contains(&m) {
                name_base.push((m & 63) as u8);
            }
        }

        name_base
            .as_slice()
            .try_into()
            .unwrap_or_else(|_| unreachable!("normal raw name base must contain 128 entries"))
    }

    /// 创建一个新的玩家（便捷入口，委托给 [`new_and_init_with_overlay`]）。
    pub fn new_and_init(team: Option<String>, name: String, weapon: Option<String>, storage: Arc<Storage>) -> PlayerResult<Self> {
        Self::new_and_init_with_overlay(team, name, weapon, None, storage)
    }

    /// 创建战斗中生成的 minion（shadow / summon / zombie）。
    ///
    /// md5.js 对这些实体直接构造对应的 `PlrShadow` / `PlrSummon` / `PlrZombie`，
    /// 不会因为 owner 的队名是 `!` 或 `\u{0002}` 而走 `PlrEx` / `PlrBossTest` 的
    /// name_base 变换。输入 roster 中的同名玩家仍应使用普通入口。
    pub(crate) fn new_minion_and_init(
        team: Option<String>,
        name: String,
        weapon: Option<String>,
        storage: Arc<Storage>,
    ) -> PlayerResult<Self> {
        Self::new_and_init_inner(team, name, weapon, None, storage, true)
    }

    /// 创建一个新的玩家，支持传入 overlay 覆盖数据。
    ///
    /// overlay 可以覆盖八围属性、技能等级和武器名。
    /// 如果 overlay 提供了武器名，会覆盖 `weapon` 参数。
    /// 如果 overlay 提供了技能映射，skill_id 顺序会使用固定布局而非随机洗牌。
    pub fn new_and_init_with_overlay(
        team: Option<String>,
        name: String,
        weapon: Option<String>,
        overlay: Option<PlayerOverlay>,
        storage: Arc<Storage>,
    ) -> PlayerResult<Self> {
        Self::new_and_init_inner(team, name, weapon, overlay, storage, false)
    }

    fn new_and_init_inner(
        team: Option<String>,
        name: String,
        weapon: Option<String>,
        overlay: Option<PlayerOverlay>,
        storage: Arc<Storage>,
        force_normal_type: bool,
    ) -> PlayerResult<Self> {
        // 先校验长度
        if let Some(t) = team.as_ref()
            && t.len() > TEAM_MAX_LEN
        {
            return Err(PlayerError::TeamNameTooLong(t.len(), t.len()));
        }
        if name.len() > NAME_MAX_LEN {
            return Err(PlayerError::NameTooLong(name.len(), name.len()));
        }
        let player_type = {
            if force_normal_type {
                PlayerType::Normal
            } else if let Some(t) = team.as_ref() {
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
            let m = ((unsafe { rand.get_val_unchecked(i) } as u32 * 181) + 160) % 256;
            if (89..217).contains(&m) {
                name_base.push((m & 63) as u8);
            }
        }
        // 这里可以安全 unwrap：name_base.len() 恒为 128。
        let mut raw_name_base: [u8; 128] = name_base
            .as_slice()
            .try_into()
            .unwrap_or_else(|_| unreachable!("unreachable(如果真到这里了就tm得好好怀疑一下自己的代码是怎么写的了)"));

        // Test1/Test2/TestEx 特殊 name_base 修改（对应 JS e4/e5/e2）
        match player_type {
            PlayerType::Test1 => {
                // JS PlrBossTest.e4: for i in 0..50, if val < 12, val = 63 - val
                for val in name_base.iter_mut().take(50) {
                    if *val < 12 {
                        *val = 63 - *val;
                    }
                }
            }
            PlayerType::Test2 => {
                // JS PlrBossTest2.e5: for i in 0..50, if val < 32, val = 63 - val
                for val in name_base.iter_mut().take(50) {
                    if *val < 32 {
                        *val = 63 - *val;
                    }
                }
            }
            PlayerType::TestEx => {
                // JS PlrEx.e2: for i in 6..50, if val < 41, val = (val & 15) + 41
                for val in name_base.iter_mut().skip(6).take(50 - 6) {
                    if *val < 41 {
                        *val = (*val & 15) + 41;
                    }
                }
                // for i in 50..128, if val < 16, val += 32
                for val in name_base.iter_mut().skip(50).take(128 - 50) {
                    if *val < 16 {
                        *val += 32;
                    }
                }
                // TestEx 还会将修改后的 name_base 复制到 raw_name_base
                raw_name_base = name_base.as_slice().try_into().unwrap();
            }
            _ => {}
        }

        // 技能 ID 顺序（skil_id）：
        // - overlay 有技能映射时：使用固定顺序（主动→被动），保证槽位稳定；
        // - 否则：正常随机洗牌。
        let skills = if overlay.as_ref().and_then(|ov| ov.skills.as_ref()).is_some() {
            crate::player::skill::diy_skill_order()
                .into_iter()
                .map(|id| id as u32)
                .collect::<Vec<u32>>()
        } else {
            let mut skills = (0..40).collect::<Vec<u32>>();
            rand.sort_list(&mut skills);
            skills
        };

        // JS bf(): Test1/Test2/TestEx 的 name_factor 强制为 0
        // overlay.name_factor_enabled = false 时也强制为 0（八围不缩放）
        let name_factor = match player_type {
            PlayerType::Test1 | PlayerType::Test2 | PlayerType::TestEx => 0.0,
            _ => {
                if overlay.as_ref().map(|ov| !ov.name_factor_enabled).unwrap_or(false) {
                    0.0
                } else {
                    let eval_rq = storage.eval_rq();
                    let factor_name = eval_name::eval_str_common_with_rq(name.as_str(), true, eval_rq);
                    let factor_team = match team.as_ref() {
                        Some(team) => eval_name::eval_str_common_with_rq(team.as_str(), true, eval_rq),
                        None => factor_name,
                    };
                    factor_name.max(factor_team - 6.0)
                }
            }
        };

        let mut status = PlayerStatus::default();
        if player_type == PlayerType::Seed {
            status.set_alive(false);
        }

        let id = storage.new_plr_id();

        // overlay 装箱（从栈上移到堆上，减少 Player 结构体大小）
        let overlay = overlay.map(Box::new);

        // DIY 模式下武器不计入：当 overlay 包含八围或技能覆盖时，武器状态置空。
        let has_diy_attrs_or_skills = overlay.as_ref().map(|ov| ov.attrs.is_some() || ov.skills.is_some()).unwrap_or(false);
        // 武器名解析优先级：名字中的 weapon 段 > overlay 中的 weapon 字段
        let weapon = weapon.or_else(|| overlay.as_ref().and_then(|overlay| overlay.weapon.clone()));

        // 创建武器运行时状态（对应 JS: new T.Weapon + b3）
        // DIY 模式下武器不计入，weapon_state 强制为 None。
        let weapon_state = if has_diy_attrs_or_skills {
            None
        } else {
            weapon.as_deref().and_then(weapons::Weapon::create_state)
        };

        Ok(Player {
            team,
            name,
            id_name_override: None,
            display_name_override: None,
            minion_name_next_index: 0,
            weapon,
            overlay,
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
            weapon_state,
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

    /// 增减 move point (spsum)
    #[inline]
    pub fn add_move_point(&mut self, val: i32) { self.status.move_point += val }

    /// 获取武器名
    #[inline]
    pub fn get_weapon_name(&self) -> Option<&str> { self.weapon.as_deref() }

    #[inline]
    #[deprecated(note = "请使用 magic_point()")]
    pub fn mp(&self) -> i32 { self.status.magic_point }

    #[inline]
    pub fn magic_point(&self) -> i32 { self.status.magic_point }

    #[inline]
    #[deprecated(note = "请使用 set_magic_point()")]
    pub fn set_mp(&mut self, val: i32) { self.status.magic_point = val; }

    #[inline]
    pub fn set_magic_point(&mut self, val: i32) { self.status.magic_point = val; }

    #[inline]
    pub fn set_hp_raw(&mut self, val: i32) -> bool {
        self.status.hp = val.max(0);
        if self.status.hp <= 0 {
            self.status.set_alive(false);
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn mul_at_boost(&mut self, scale: f64) { self.status.at_boost *= scale; }

    #[inline]
    pub fn mul_attract(&mut self, scale: f64) { self.status.attract *= scale; }

    #[inline]
    pub fn div_attract(&mut self, scale: f64) { self.status.attract /= scale; }

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

    pub fn check_immune(&self, key: &str, randomer: &mut RC4) -> bool {
        match self.player_type {
            PlayerType::Boost => randomer.r127() < boost_value(&self.name),
            PlayerType::Boss => {
                let threshold = crate::player::boss::boss_immune_threshold(&self.name, key);
                (randomer.next_u8() as i32) < threshold
            }
            _ => false,
        }
    }

    /// 获取当前的玩家状态
    pub fn get_status(&self) -> &PlayerStatus { &self.status }

    #[inline]
    pub fn player_type(&self) -> PlayerType { self.player_type }

    #[inline]
    pub fn attr_sum(&self) -> i32 { self.status.attr_sum as i32 }

    #[inline]
    pub fn negative_state_count(&self) -> usize { self.state.negative_state_count() }

    /// 获取玩家句柄（兼容旧接口名）。
    #[inline]
    pub fn as_ptr(&self) -> PlrId { self.ptr() }

    /// 获取玩家句柄（推荐新接口名）。
    #[inline]
    pub fn ptr(&self) -> PlrId { self.id.try_into().expect("player id overflow usize") }

    pub fn id(&self) -> u64 { self.id }
}

use super::*;

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
            let m = ((unsafe { rand.get_val_unchecked(i) } as u32 * 181) + 160) % 256;
            if (89..217).contains(&m) {
                name_base.push((m & 63) as u8);
            }
        }
        // UNWRAP SAFE: name_base.len() == 128
        let raw_name_base: [u8; 128] = name_base
            .as_slice()
            .try_into()
            .unwrap_or_else(|_| unreachable!("unreachable(如果真到这里了就tm得好好怀疑一下自己的代码是怎么写的了)"));

        // 技能顺序
        let mut skills = (0..40).collect::<Vec<u32>>();
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
}

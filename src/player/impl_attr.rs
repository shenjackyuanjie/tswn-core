use super::*;

impl Player {
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
    pub fn build(&mut self) { self.build_inner(None); }

    /// Dart PlrClone 在 addSkillsToProc 中先 clamp 技能等级到 owner 的当前等级，
    /// 然后再执行 boost（super.addSkillsToProc）。
    /// 普通 build 不做 clamp，clone 通过传入 owner 的 skill store 来执行 clamp。
    pub fn build_for_clone(&mut self, owner_skills: &crate::player::skill::store::SkillStorage) {
        self.build_inner(Some(owner_skills));
    }

    fn build_inner(&mut self, clamp_source: Option<&crate::player::skill::store::SkillStorage>) {
        // pre_upgrade: 修改 name_base (JS: weapon.bn)
        if let Some(mut ws) = self.weapon_state.take() {
            weapons::Weapon::pre_upgrade(&mut ws, self);
            self.weapon_state = Some(ws);
        }

        // init raw attr
        let mut rand_vals = [0_u8; 32];
        rand_vals.copy_from_slice(&self.name_base[0..32]);
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
        attr[7] = 154 + rand_vals[3] as u32 + rand_vals[4] as u32 + rand_vals[5] as u32 + rand_vals[6] as u32;
        self.attr = attr;

        // Boss appendAttr: 在基础八围之上加成
        if self.player_type == PlayerType::Boss {
            let bonus = boss_append_attr(&self.name);
            for (a, b) in self.attr.iter_mut().zip(bonus.iter()) {
                *a = (*a as i32 + *b).max(0) as u32;
            }
        }
        // println!("attr: {:?} {:?}", self.attr, self.name_base);

        // init skills
        // 技能熟练度计算
        // 计算 skl_id 的已经在初始化做完了
        // DIY TODO
        self.skills = crate::player::skill::store::SkillStorage::new();
        for skill_id in 0..40u8 {
            self.skills.add_skill(Skill::new_with_id(0, skill_id));
        }
        self.skills.skill = self.skil_id.iter().map(|id| *id as usize).collect();
        let mut slot_skill_keys: [Option<usize>; 16] = [None; 16];
        // JS PlrBoss.dm() overrides initSkills: boss skills are all level 0.
        // All 40 skills are created (for k4 prob byte consumption in action loop)
        // but no levels are set. This prevents boss from using normal skills.
        let is_boss = self.player_type == PlayerType::Boss;
        if !is_boss {
            for (j, i) in (64..128).step_by(4).enumerate() {
                // 取 val index ~ val index + 3 的最小值
                let small = min(
                    min(self.name_base[i], self.name_base[i + 1]),
                    min(self.name_base[i + 2], self.name_base[i + 3]),
                );
                if small > 10 && self.skil_id[j] < 35 {
                    let skill_id = self.skil_id[j] as usize;
                    let skill = self.skills.skill_by_id_mut(skill_id);
                    skill.set_level((small - 10) as u32);
                    let raw_small = min(
                        min(self.raw_name_base[i], self.raw_name_base[i + 1]),
                        min(self.raw_name_base[i + 2], self.raw_name_base[i + 3]),
                    );
                    // 其实是懒得读取原始的last skill, 就直接按照原始代码来了
                    if raw_small <= 10 {
                        skill.boosted = true;
                    }
                    slot_skill_keys[j] = Some(skill_id);
                }
            }
        }

        // post_upgrade: 加八围 + boost skill (JS: weapon.cs)
        if let Some(ref ws) = self.weapon_state {
            if std::env::var_os("TSWN_DEBUG_STATS").is_some() {
                eprintln!(
                    "[WEAPON] {}: attr_bonus={:?} skill_idx={} skill_factor={}",
                    self.id_name(),
                    ws.attr_bonus,
                    ws.skill_index,
                    ws.skill_factor
                );
            }
            let ws = ws.clone();
            weapons::Weapon::post_upgrade(&ws, self);
        }

        // Dart PlrClone.addSkillsToProc: clamp 发生在 boost 之前
        if let Some(owner_skills) = clamp_source {
            let skill_keys = self.skills.skill.clone();
            for skill_key in skill_keys {
                let owner_level = owner_skills.skill_by_id(skill_key).level();
                let skill = self.skills.skill_by_id_mut(skill_key);
                if skill.level() > owner_level {
                    skill.set_level(owner_level);
                }
            }
        }

        // boost skills(addSkillsToProc)
        // boost最后一个
        self.skills.boost_last();
        // 然后是 boost passive
        if let Some(skill_key) = slot_skill_keys[14] {
            let skill_14 = self.skills.skill_by_id_mut(skill_key);
            if skill_14.level() > 0 && !skill_14.boosted {
                let boost_level = min(min(self.name_base[60], self.name_base[61]) as u32, skill_14.level());
                skill_14.boost_level(boost_level);
            }
        }
        if let Some(skill_key) = slot_skill_keys[15] {
            let skill_15 = self.skills.skill_by_id_mut(skill_key);
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
        if std::env::var_os("TSWN_DEBUG_STATS").is_some() {
            eprintln!(
                "[STATS] {}: atk={} def={} spd={} agl={} mag={} mdf={} wis={} hp={} name_factor={} attr={:?} name_base[0..10]={:?}",
                self.id_name(),
                self.status.attack,
                self.status.defense,
                self.status.speed,
                self.status.agility,
                self.status.magic,
                self.status.resistance,
                self.status.wisdom,
                self.status.max_hp,
                self.name_factor,
                self.attr,
                &self.name_base[0..10]
            );
        }
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

        // println!("status before calc_attr_sum, factor: {}: {}", self.name_factor, self.status);

        self.calc_attr_sum();

        self.status.at_boost = 1.0;
        self.status.set_frozen(false);
        self.apply_update_state_effects();
        // JS 的 F() (updateStates) 遍历 rx 列表，其中包含 state 和 skill 的 update_state 回调。
        // apply_update_state_effects 已处理 state 回调，下面调用 skill 的 update_state 回调。
        self.skills.update_state_inline(&mut self.status);
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

    pub(super) fn init_skills(&mut self) { self.skills.update_proc(); }

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
    /// 获取 短号系数
    pub fn get_name_factor(&self) -> f64 { self.name_factor }

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
        let no_weapon = if let Some((left, _)) = raw_name.split_once("+") {
            left
        } else {
            raw_name
        };
        if let Some((name, team)) = no_weapon.split_once("@") {
            if team.is_empty() || team.contains(":") {
                name.to_string()
            } else {
                format!("{name}@{team}")
            }
        } else {
            no_weapon.to_string()
        }
    }
}

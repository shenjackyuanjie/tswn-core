//! # 玩家属性计算 (impl_attr)
//!
//! 本模块实现 [`Player`] 的属性计算逻辑。
//!
//! ## 功能说明
//!
//! - **属性构建** — `build()` 和 `build_for_clone()` 计算具体属性
//! - **属性升级** — `upgrade()` 升级属性
//! - **八围推导** — 根据名字系数推导八围属性
//! - **技能熟练度** — 计算技能熟练度
//!
//! ## 属性计算流程
//!
//! 1. **名字系数调整** — 根据名字系数调整数值
//! 2. **八围计算** — 根据名字系数推导八围属性
//! 3. **技能熟练度** — 计算技能熟练度
//! 4. **属性汇总** — 计算属性总和、攻击总和、全部总和
//!
//! ## 名字系数调整
//!
//! 根据名字系数调整数值：
//! ```javascript
//! const result = Math.round(a * (1 - this.x / b))
//! ```
//!
//! ## 克隆玩家处理
//!
//! Dart `PlrClone` 在 `addSkillsToProc` 中先 clamp 技能等级到 owner 的当前等级，
//! 然后再执行 boost（`super.addSkillsToProc`）。
//! 普通 `build` 不做 clamp，clone 通过传入 owner 的 skill store 来执行 clamp。
//!
//! ## 示例
//!
//! ```rust,ignore
//! use tswn_core::player::Player;
//!
//! let mut player = /* ... */;
//! player.build(); // 计算属性
//! ```

use super::utils::{trim_js_line_end, trim_js_name_like};
use super::*;

/// 按 `+` 分割字符串，但跳过双引号字符串内的 `+`。
///
/// 例如 `diy[...]{"sklfire":"40+30"}` 不会被切分，
/// 而 `fire+diy[...]` 会被切为 `["fire", "diy[...]"]`。
fn split_by_plus_outside_quotes(raw: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;
    for ch in raw.chars() {
        if in_string {
            current.push(ch);
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
        } else if ch == '+' {
            segments.push(std::mem::take(&mut current));
        } else {
            current.push(ch);
            if ch == '"' {
                in_string = true;
            }
        }
    }
    segments.push(current);
    segments
}

impl Player {
    /// 根据名字系数调整数值
    ///
    /// ```javascript
    /// const result = Math.round(a * (1 - this.x / b))
    /// ```
    #[allow(dead_code)]
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
    pub fn build(&mut self) { self.build_inner(None, true); }

    /// clone 不是直接复用 owner 的技能对象，而是会重新 `build` 一次。
    ///
    /// 因此如果 owner 某些技能已经在战斗中因为 `post_act_level()` 衰减，
    /// clone build 后就必须把这些技能 clamp 到 owner 的“当前等级”，
    /// 否则会把已经消耗过的熟练度刷新回名字 `build()` 的初始值。
    ///
    /// 当前仓库中已确认会在出手后降低当前熟练度的主动技能有：
    /// - `生命之轮`：`(level + 1) >> 1`
    /// - `治愈魔法`：`level > 8` 时每次 `-1`
    /// - `苏生术`：`(level + 1) >> 1`
    /// - `分身`：随机衰减，见 `CloneSkill::act_with_level()`
    /// - `幻术`：`ceil(level * 0.75)`
    ///
    /// Dart `PlrClone.addSkillsToProc` 也是先 clamp 到 owner 当前等级，
    /// 然后再执行 boost（`super.addSkillsToProc`）。
    ///
    /// ## DIY 模式下的 clone 行为
    ///
    /// 当 owner 是 DIY 玩家（`owner_skills.is_diy == true`）时，clone 构建分三步：
    ///
    /// 1. **应用 overlay**：与 owner 相同的 overlay 技能配置被写入 clone，
    ///    包含 [`SkillBoost`] 元数据（`diy_boost`）。
    /// 2. **clamp 到 owner**：将 clone 的技能等级截断到 owner 当前（已衰减）等级。
    /// 3. **重新执行 SkillBoost 加成**（不依赖 name_base）：
    ///    - [`SkillBoost::Normal`]：不处理
    ///    - [`SkillBoost::LastBoost`]：衰减后等级 × 2（翻倍）
    ///    - [`SkillBoost::SlotBoost`]：衰减后等级 + boost（执行 +b）
    ///
    /// 这模拟了 JS "先 clamp 再 boost" 的衰减下限语义：
    /// - 普通玩家：owner Shadow 92 → 衰减到 40 → clone clamp=40 → boost_last 翻倍 → 80
    /// - DIY 玩家：owner Shadow 92(2*46) → 衰减到 40 → clone clamp=40 → LastBoost 翻倍 → 80
    pub fn build_for_clone(&mut self, owner_skills: &crate::player::skill::store::SkillStorage) {
        let apply_overlay = owner_skills.is_diy;
        self.build_inner(Some(owner_skills), apply_overlay);
    }

    fn build_inner(&mut self, clamp_source: Option<&crate::player::skill::store::SkillStorage>, apply_overlay: bool) {
        // pre_upgrade: 修改 name_base (JS: weapon.bn)
        if let Some(mut ws) = self.weapon_state.take() {
            weapons::Weapon::pre_upgrade(&mut ws, self);
            self.weapon_state = Some(ws);
        }

        // 初始化原始八围
        let mut rand_vals = [0_u8; 32];
        rand_vals.copy_from_slice(&self.name_base[0..32]);
        rand_vals.get_mut(0..10).unwrap().sort_unstable();

        let mut attr = [0, 0, 0, 0, 0, 0, 0, 0];
        // 10 ~ 31
        // rand_vals 10~12 的中位数
        attr[0] = median(rand_vals[10], rand_vals[11], rand_vals[12]) as u32;
        attr[1] = median(rand_vals[13], rand_vals[14], rand_vals[15]) as u32;
        attr[2] = median(rand_vals[16], rand_vals[17], rand_vals[18]) as u32;
        attr[3] = median(rand_vals[19], rand_vals[20], rand_vals[21]) as u32;
        attr[4] = median(rand_vals[22], rand_vals[23], rand_vals[24]) as u32;
        attr[5] = median(rand_vals[25], rand_vals[26], rand_vals[27]) as u32;
        attr[6] = median(rand_vals[28], rand_vals[29], rand_vals[30]) as u32;
        // 第 7 项 = rand 3 + 4 + 5 + 6
        attr[7] = 154 + rand_vals[3] as u32 + rand_vals[4] as u32 + rand_vals[5] as u32 + rand_vals[6] as u32;

        // 如果 overlay 提供了八围属性，直接使用覆盖值（已剪裁到非负）；
        // 否则走正常的随机生成 + Boss 加成流程。
        let overlay_attrs = if apply_overlay {
            self.overlay.as_ref().and_then(|overlay| overlay.attrs)
        } else {
            None
        };
        if let Some(attrs) = overlay_attrs {
            self.attr = attrs.map(|value| value.max(0) as u32);
        } else {
            self.attr = attr;

            // Boss 的 appendAttr：在基础八围之上叠加额外属性
            if self.player_type == PlayerType::Boss {
                let bonus = boss_append_attr(&self.name);
                for (a, b) in self.attr.iter_mut().zip(bonus.iter()) {
                    *a = (*a as i32 + *b).max(0) as u32;
                }
            }
        }
        // println!("attr: {:?} {:?}", self.attr, self.name_base);

        // 技能熟练度计算
        // skil_id 的映射已在 init 阶段完成
        // 如果 overlay 携带技能等级映射，之后会走 apply_diy_skill_levels 直接覆盖；
        // 否则走正常的名字推导 + boost 流程。
        let diy_skill_levels = if apply_overlay {
            self.overlay.as_ref().and_then(|overlay| overlay.skills.clone())
        } else {
            None
        };
        self.skills = crate::player::skill::store::SkillStorage::new();
        for skill_id in 0..40u8 {
            self.skills.add_skill(Skill::new_with_id(0, skill_id));
        }
        if diy_skill_levels.is_none() {
            // 正常模式：slot_skill 保持创建时的稳定顺序 (0..40)
            // JS `k1` 是”固定技能对象数组”，普通玩家这里保持创建时的稳定顺序；
            // 真正用于 action() 主动技能扫描的是 `k4`，它才会按名字解出的 `skil_id` 洗牌。
            //
            // 这个区别对 merge 很关键：
            // - `k1` 负责”同一个固定槽位里的 skill 对象”逐位继承等级
            // - `k4` 负责”本回合按什么顺序尝试主动技能”
            //
            // 如果把 `slot_skill` 也改成打乱后的 `skil_id`，merge 就会把本来应该同槽位的
            // Shield / Defend / PassiveSkill 错位掉，导致吞噬后漏继承 pre_action / post_damage hook。
            self.skills.slot_skill = (0..40usize).collect();
            self.skills.skill = self.skil_id.iter().map(|id| *id as usize).collect();
        }
        let mut slot_skill_keys: [Option<usize>; 16] = [None; 16];
        if let Some(skill_levels) = diy_skill_levels.as_ref() {
            // overlay 模式：直接用指定的等级覆盖技能，不走名字推导和 boost
            crate::player::skill::apply_diy_skill_levels(&mut self.skills, skill_levels);
        } else {
            // JS PlrBoss.dm() 覆写了 initSkills：Boss 的全部 40 个技能等级为 0。
            // 创建 40 个技能仅为了 action 循环中 k4 的 prob 字节消费，
            // 不设等级，从而阻止 Boss 使用普通技能。
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
                        // 这里懒得回头读取原始的最后一个技能，直接按原始代码逻辑处理
                        if raw_small <= 10 {
                            skill.boosted = true;
                        }
                        slot_skill_keys[j] = Some(skill_id);
                    }
                }
            }
        }

        // post_upgrade：加八围并处理技能 boost（JS: `weapon.cs`）
        if let Some(ref ws) = self.weapon_state {
            let ws = ws.clone();
            weapons::Weapon::post_upgrade(&ws, self);
        }

        // Dart / JS `PlrClone.addSkillsToProc`：这里只 clamp “固定槽位里已有的技能等级”，
        // 不改槽位本身。也就是说 clone 继承 owner 时，`slot_skill` 仍然表示 clone build
        // 出来的固定槽位类型，行动顺序也沿用该视图；只有等级会在这里被截到 owner 当前等级。
        //
        // 这里截的是“当前战斗中的技能等级”，不是重新 build 出来的原始熟练度。
        // 例如 owner 已经把 `幻术 10` 用成 `8`，或者把 `生命之轮 9` 用成 `5` 后，
        // clone 如果不在此处截断，就会把这些已衰减技能恢复成名字 build 的初始值，
        // 进而改变后续的概率判定、行动顺序和整场回放。
        if let Some(owner_skills) = clamp_source {
            let skill_keys = self.skills.skill.clone();
            for skill_key in skill_keys {
                let owner_level = owner_skills.skill_by_id(skill_key).level();
                let skill = self.skills.skill_by_id_mut(skill_key);
                if skill.level() > owner_level {
                    skill.set_level(owner_level);
                } // DIY clone：从 owner 继承 diy_boost 信息，确保衰减下限可预测。
                // 注意：owner 的技能可能已经在战斗中衰减，
                // clone 拿到的是 overlay 初始等级 clamp 到 owner 当前等级后的值。
                // diy_boost 信息已在 apply_diy_skill_levels 阶段写入 clone 的 skill，
                // 这里额外确保它也同步自 owner（处理 owner 运行时可能修改过的 boost 元数据）。
                if owner_skills.is_diy {
                    if let Some(ref owner_diy_boost) = owner_skills.skill_by_id(skill_key).diy_boost {
                        if skill.diy_boost.is_none() {
                            skill.diy_boost = Some(owner_diy_boost.clone());
                        }
                    }
                }
            }
        }

        if diy_skill_levels.is_none() {
            // 普通玩家和普通 clone 都走 JS `super.addSkillsToProc` 语义：
            // clamp 之后按当前实体自己的行动顺序执行 boost。
            self.skills.boost_last();
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
        } else {
            // DIY overlay：apply_diy_skill_levels 写入的是 base_level；
            // 这里按 overlay 的 SkillBoost 元数据重放加成。clone 场景会先 clamp 再执行本段。
            let skill_keys = self.skills.skill.clone();
            for skill_key in &skill_keys {
                let Some(boost_info) = self.skills.skill_by_id(*skill_key).diy_boost.clone() else {
                    continue;
                };
                let skill = self.skills.skill_by_id_mut(*skill_key);
                let current = skill.level();
                match boost_info {
                    SkillBoost::Normal(_) => {}
                    SkillBoost::LastBoost(_) => {
                        skill.set_level(current.saturating_mul(2));
                        skill.boosted = true;
                    }
                    SkillBoost::SlotBoost { boost, .. } => {
                        skill.set_level(current.saturating_add(boost));
                        skill.boosted = true;
                    }
                }
            }
        }
        // 更新 proc(其实就是缓存)
        self.skills.update_proc();

        self.init_values();
    }
    /// 初始化生命/蓝条（只在 build 阶段调用一次）
    pub fn init_values(&mut self) {
        self.update_states();
        self.status.hp = self.status.max_hp;
        // 对齐 Dart：mp = itl ~/ 2
        self.status.magic_point = self.status.wisdom >> 1;
    }

    /// 导出时使用的队伍名片段：`@Team` 或空字符串。
    fn team_name_for_export(&self) -> String {
        let team = self.clan_name();
        let name = self.base_name();
        if team != name { format!("@{}", team) } else { String::new() }
    }

    /// 将已 build 的玩家导出为紧凑 DIY 格式字符串。
    ///
    /// 格式：`Name@Team+diy[atk,def,spd,agi,mag,res,wis,maxhp]{"sklFire":lv,...}`
    ///
    /// 前七围值会自动 +36 以匹配 JS 侧编码（解析时 -36），HP 保持不变。
    /// 技能等级按当前实际等级输出，加成类型（LastBoost/SlotBoost）
    /// 通过 `diy_boost` 元数据反推。
    pub fn to_diy_compact(&self) -> String {
        // 前七围 +36，HP 不变（JS 仅对索引 0~6 做 -=36）
        let attrs: Vec<String> = self
            .attr
            .iter()
            .enumerate()
            .map(|(i, v)| if i < 7 { (v + 36).to_string() } else { v.to_string() })
            .collect();
        let mut skills = String::from('{');
        let mut first = true;
        for skill_key in &self.skills.skill {
            let skill = self.skills.skill_by_id(*skill_key);
            let name = skill_name_for_export(*skill_key);
            if !first {
                skills.push(',');
            }
            first = false;
            if skill.level() == 0 {
                continue;
            }
            match &skill.diy_boost {
                Some(SkillBoost::SlotBoost { boost, .. }) => {
                    let base = skill.level().saturating_sub(*boost);
                    skills.push_str(&format!("\"{}\":\"{}+{}\"", name, base, boost));
                }
                Some(SkillBoost::LastBoost(_)) => {
                    let base = skill.level() / 2;
                    skills.push_str(&format!("\"{}\":\"2*{}\"", name, base));
                }
                _ => {
                    skills.push_str(&format!("\"{}\":{}", name, skill.level()));
                }
            }
        }
        skills.push('}');
        let team_part = self.team_name_for_export();
        format!("{}{}+diy[{}]{}", self.id_name(), team_part, attrs.join(","), skills)
    }

    /// 将已 build 的玩家导出为 ol: JSON 格式字符串。
    ///
    /// 格式：`Name@Team+ol:{"attrs":[...],"skills":{...},"name_factor_enabled":bool}`
    ///
    /// 技能按行动顺序输出（先列出的先尝试），包含全部 40 个技能槽位。
    /// 前七围 +36，HP 不变。
    pub fn to_ol_json(&self) -> String {
        let attrs: Vec<String> = self
            .attr
            .iter()
            .enumerate()
            .map(|(i, v)| if i < 7 { (v + 36).to_string() } else { v.to_string() })
            .collect();
        let mut skills = String::from('{');
        let mut first = true;
        for skill_key in &self.skills.skill {
            let skill = self.skills.skill_by_id(*skill_key);
            let name = skill_name_for_export(*skill_key);
            if !first {
                skills.push(',');
            }
            first = false;
            if skill.level() == 0 {
                continue;
            }
            match &skill.diy_boost {
                Some(SkillBoost::SlotBoost { boost, .. }) => {
                    let base = skill.level().saturating_sub(*boost);
                    skills.push_str(&format!("\"{}\":\"{}+{}\"", name, base, boost));
                }
                Some(SkillBoost::LastBoost(_)) => {
                    let base = skill.level() / 2;
                    skills.push_str(&format!("\"{}\":\"2*{}\"", name, base));
                }
                _ => {
                    skills.push_str(&format!("\"{}\":{}", name, skill.level()));
                }
            }
        }
        skills.push('}');
        let team_part = self.team_name_for_export();
        format!(
            "{}{}+ol:{{\"attrs\":[{}],\"skills\":{},\"name_factor_enabled\":{}}}",
            self.id_name(),
            team_part,
            attrs.join(","),
            skills,
            self.overlay.as_ref().map_or(true, |ov| ov.name_factor_enabled)
        )
    }

    /// 更新状态
    pub fn update_states(&mut self) {
        #[cfg(not(feature = "no_debug"))]
        let debug_update_this = crate::debug::debug_action_matches(&self.id_name());
        #[cfg(not(feature = "no_debug"))]
        let before = if debug_update_this {
            Some((
                self.status.attack,
                self.status.speed,
                self.status.magic,
                self.status.at_boost,
                self.status.move_point,
            ))
        } else {
            None
        };
        // 初始化运行时数值
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
        #[cfg(not(feature = "no_debug"))]
        if let Some((attack_before, speed_before, magic_before, boost_before, move_before)) = before {
            eprintln!(
                "[update_states] actor={} atk {}->{} spd {}->{} mag {}->{} boost {:.6}->{:.6} move={} hp={}",
                self.id_name(),
                attack_before,
                self.status.attack,
                speed_before,
                self.status.speed,
                magic_before,
                self.status.magic,
                boost_before,
                self.status.at_boost,
                move_before,
                self.status.hp,
            );
            if std::env::var_os("TSWN_DEBUG_UPDATE_BT").is_some() {
                eprintln!("[update_states_bt] {:?}", std::backtrace::Backtrace::capture());
            }
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

    pub(super) fn init_skills(&mut self) { self.skills.update_proc(); }

    /// 同队升级
    pub fn upgrade(&mut self, other: &Self) {
        // JS `PlrEx.cA()` 是空操作。`@!` TestEx 玩家会保留自己的
        // 变换后 `name_base`，不会接受同队升级。
        if self.player_type == PlayerType::TestEx {
            return;
        }

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

    /// 从名竞原始输入字符串创建 Player。
    ///
    /// # 要求
    /// 输入中不得包含 `\n`。
    ///
    /// # 支持的输入格式
    ///
    /// | 格式 | 含义 |
    /// |------|------|
    /// | `<name>` | 纯名字，无队伍无武器 |
    /// | `<name>@<team>` | 名字 + 队伍 |
    /// | `<name>+<weapon>` | 名字 + 武器 |
    /// | `<name>+diy[attrs]{skills}` | 名字 + DIY 紧凑格式 overlay（无武器） |
    /// | `<name>+ol:{...}` | 名字 + JSON 对象格式 overlay |
    /// | `<name>@<team>+<weapon>+diy[...]{...}` | 名字 + 队伍 + 武器 + overlay |
    ///
    /// overlay 段（`diy[...]{...}` 或 `ol:{...}`）通过 `+` 与武器名分隔，
    /// 最多出现一次；多个 `+` 段会拼接为复合武器名。
    pub fn new_from_namerena_raw(raw_name: String, storage: Arc<Storage>) -> PlayerResult<Self> {
        let raw_name = trim_js_line_end(&raw_name);
        // 先判断是否有 + 和 @
        if !raw_name.contains("@") && !raw_name.contains("+") {
            return Player::new_and_init(None, raw_name.to_string(), None, storage);
        }
        // 第一步：按 @ 分离名字和队伍（队伍段可能进一步包含武器/overlay）
        let name: &str;
        let mut team: &str;
        let weapon: Option<String>;
        let mut overlay: Option<PlayerOverlay> = None;
        if raw_name.contains("@") {
            (name, team) = raw_name.split_once("@").unwrap();
            // 队伍段中再按 + 分离队伍名和武器/overlay
            if team.contains("+") {
                let tmp;
                (team, tmp) = team.split_once("+").unwrap();
                team = trim_js_line_end(team);
                let (parsed_weapon, parsed_overlay) = Self::split_weapon_overlay(tmp);
                weapon = parsed_weapon;
                overlay = parsed_overlay;
            } else {
                weapon = None;
            }
            Player::new_and_init_with_overlay(Some(team.to_string()), name.to_string(), weapon, overlay, storage)
        } else {
            // 无队伍名：按 + 分离名字和武器/overlay
            if raw_name.contains("+") {
                let (name, rest) = raw_name.split_once("+").unwrap();
                let (parsed_weapon, parsed_overlay) = Self::split_weapon_overlay(rest);
                weapon = parsed_weapon;
                overlay = parsed_overlay;
                Player::new_and_init_with_overlay(None, trim_js_line_end(name).to_string(), weapon, overlay, storage)
            } else {
                Player::new_and_init(None, raw_name.to_string(), None, storage)
            }
        }
    }

    /// 按 `+` 分割后缀段，从中分离武器名和 overlay。
    ///
    /// 遍历每个 `+` 分隔的段：
    /// - 如果能解析为 `PlayerOverlay`（以 `diy[` 或 `ol:` 开头），记录为 overlay；
    /// - 否则拼接到武器名中（多个非 overlay 段通过 `+` 连接）。
    ///
    /// 注意：overlay 最多只有一个，后续匹配到的 overlay 段会覆盖前者。
    ///
    /// 分割时会跳过双引号字符串内的 `+`，避免把
    /// `"40+30"` 这类 SkillBoost 格式的值错误切分。
    fn split_weapon_overlay(raw: &str) -> (Option<String>, Option<PlayerOverlay>) {
        let mut weapon: Option<String> = None;
        let mut overlay: Option<PlayerOverlay> = None;
        for segment in split_by_plus_outside_quotes(raw) {
            let trimmed = trim_js_name_like(&segment);
            if trimmed.is_empty() {
                continue;
            }
            if let Some(parsed) = PlayerOverlay::parse_inline(trimmed) {
                overlay = Some(parsed);
                continue;
            }
            weapon = Some(match weapon {
                Some(existing) => format!("{existing}+{trimmed}"),
                None => trimmed.to_string(),
            });
        }
        (weapon, overlay)
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
            if team.is_empty() || team == name || team.contains(":") {
                name.to_string()
            } else {
                format!("{name}@{team}")
            }
        } else {
            no_weapon.to_string()
        }
    }
}

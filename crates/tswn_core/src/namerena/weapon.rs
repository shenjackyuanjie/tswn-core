use crate::rc4::RC4;

const ATTR_COUNT: usize = 8;
const HP_INDEX: usize = 7;

/// 0.5.5 `$.r1()`：桂月奖杯生效的阵营。
const GUI_YUE_CLANS: [&str; 17] = [
    "Rinick",
    "库瓒",
    "庫瓒",
    "涵虚",
    "霛雲",
    "云剑",
    "新纪元",
    "琪拉拉",
    "纯菜",
    "学车中学",
    "学🚗🀄学",
    "昀澤",
    "锦依卫",
    "Σσ",
    "Ø",
    "∅",
    "斜眼笑",
];

/// 0.5.5 `$.rk()`：玄月奖杯生效的阵营。
const XUAN_YUE_CLANS: [&str; 22] = [
    "Rinick",
    "库瓒",
    "涵虚",
    "霛雲",
    "云剑",
    "新纪元",
    "琪拉拉",
    "学车中学",
    "昀澤",
    "Σσ",
    "滑稽",
    "坤灵剑",
    "RailGun",
    "巡洋舰",
    "Hell",
    "佘山",
    "房刚",
    "五班",
    "XJ联队",
    "乐正绫",
    "文哥",
    "geometrydash",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WeaponKind {
    Generic,
    S11,
    DeathNote,
    BossEx,
    RinickModifier,
    GuiYue,
    XuanYue,
    NoWeapon,
}

impl WeaponKind {
    /// 只有会走 `bn()` 前缀强化的武器才调整名字三元组。
    fn adjusts_name_triplets(self) -> bool {
        matches!(
            self,
            Self::Generic | Self::S11 | Self::DeathNote | Self::BossEx | Self::RinickModifier
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct WeaponBuild {
    seed: Vec<u8>,
    pub(crate) attr_bonus: [i32; ATTR_COUNT],
    skill_index: usize,
    skill_factor: i32,
    kind: WeaponKind,
}

impl WeaponBuild {
    pub(crate) fn parse(name: &str, clan_name: &str, id_key_name: &str) -> Option<Self> {
        // 0.5.5 中 `+diy...` 直接作为自定义数据，不会创建武器。
        if name.is_empty() || name.starts_with("diy") {
            return None;
        }
        let kind = match name {
            "剁手刀" => WeaponKind::S11,
            "死亡笔记" => WeaponKind::DeathNote,
            "属性修改器" if clan_name == "Rinick" => WeaponKind::RinickModifier,
            "属性修改器" => WeaponKind::NoWeapon,
            "桂月奖杯" if GUI_YUE_CLANS.contains(&clan_name) => WeaponKind::GuiYue,
            "桂月奖杯" => WeaponKind::NoWeapon,
            "玄月奖杯" if XUAN_YUE_CLANS.contains(&clan_name) => WeaponKind::XuanYue,
            "玄月奖杯" => WeaponKind::NoWeapon,
            _ if name.ends_with('!') => WeaponKind::BossEx,
            _ => WeaponKind::Generic,
        };
        if matches!(kind, WeaponKind::GuiYue | WeaponKind::XuanYue | WeaponKind::NoWeapon) {
            return Some(Self {
                seed: Vec::new(),
                attr_bonus: [0; ATTR_COUNT],
                skill_index: 0,
                skill_factor: 0,
                kind,
            });
        }
        // BossWeapon 先用去掉末尾 `!` 的武器名初始化，再用角色 id 名重新混淆。
        let base_name = if kind == WeaponKind::BossEx {
            &name[..name.len() - 1]
        } else {
            name
        };
        let mut key = Vec::with_capacity(base_name.len() + 1);
        key.push(0);
        key.extend_from_slice(base_name.as_bytes());
        let mut rc4 = RC4::new(&key, 2);
        if kind == WeaponKind::BossEx {
            let mut id_key = Vec::with_capacity(id_key_name.len() + 1);
            id_key.push(0);
            id_key.extend_from_slice(id_key_name.as_bytes());
            rc4.update(&id_key, 2);
        }
        let seed = rc4.main_val.iter().map(|value| value & 63).collect::<Vec<_>>();
        let skill_index = rc4.next_i32(40) as usize;
        let attr_index = rc4.next_i32(8) as usize;
        let mut weights = [0i32; ATTR_COUNT];
        if attr_index == 6 {
            for index in 0..ATTR_COUNT {
                weights[index] = seed[40 + index] as i32;
            }
        } else {
            for index in 0..ATTR_COUNT {
                let value = seed[40 + index] as i32;
                weights[index] = if value > 53 { value - 50 } else { 0 };
            }
            weights[attr_index] = 18;
        }
        let positive_count = weights.iter().filter(|value| **value > 0).count() as i32;
        let weight_sum = weights.iter().filter(|value| **value > 0).sum::<i32>() * 3;
        let mut head: [u8; ATTR_COUNT] = seed[..ATTR_COUNT].try_into().expect("weapon seed head has fixed size");
        head.sort_unstable();
        let total = head[1] as i32 + head[4] as i32 + positive_count;
        let mut attr_bonus = [0; ATTR_COUNT];
        if weight_sum > 0 {
            let mut remaining = total;
            for index in 0..HP_INDEX {
                let value = total * weights[index] / weight_sum;
                remaining -= value * 3;
                attr_bonus[index] = value;
            }
            if weights[HP_INDEX] > 0 {
                attr_bonus[HP_INDEX] = remaining;
            }
        }
        if kind == WeaponKind::S11 {
            attr_bonus = [11, 0, 11, 0, 0, 0, 0, 0];
        }
        Some(Self {
            seed,
            attr_bonus,
            skill_index,
            skill_factor: 0,
            kind,
        })
    }

    pub(crate) fn pre_upgrade(&mut self, raw_name_base: &[u8; 128], name_base: &mut [u8; 128]) {
        if self.kind == WeaponKind::BossEx {
            // 0.5.5 `BossWeapon.cB`：与增益后的名字比较，差值不足 `32` 时补 `32`。
            for offset in 7..10 {
                let weapon = self.seed[offset] as i32;
                let name = name_base[offset] as i32;
                let delta = weapon - name;
                name_base[offset] = if delta > 0 {
                    (name + delta) as u8
                } else if name < 32 {
                    (name + 32) as u8
                } else {
                    name as u8
                };
            }
        }
        if !self.kind.adjusts_name_triplets() {
            return;
        }
        let mut delta_sum = 0;
        for offset in (10..31).step_by(3) {
            delta_sum += self.adjust_name_triplet(raw_name_base, name_base, offset);
        }
        self.skill_factor = ((480 - delta_sum) / 6).max(0);
    }

    fn adjust_name_triplet(&self, raw_name_base: &[u8; 128], name_base: &mut [u8; 128], offset: usize) -> i32 {
        let left = self.seed[offset] as i32 - raw_name_base[offset] as i32;
        let middle = self.seed[offset + 1] as i32 - raw_name_base[offset + 1] as i32;
        let right = self.seed[offset + 2] as i32 - raw_name_base[offset + 2] as i32;
        if left > 0 && middle > 0 && right > 0 {
            let target = offset + ((left + middle + right + 999) / 3) as usize;
            if target < name_base.len() && target < self.seed.len() {
                let delta = (self.seed[target] as i32 - name_base[target] as i32) / 2 + 1;
                if delta > 0 {
                    name_base[target] = (name_base[target] as i32 + delta) as u8;
                }
            }
        }
        left.abs() + middle.abs() + right.abs()
    }

    pub(crate) fn post_upgrade(&self, attrs: &mut [u32; 8], levels: &mut [u32; 40], boosted: &mut [bool; 40]) {
        // 桂月/玄月奖杯与无效武器在 0.5.5 中的 `b3()/bn()` 均为空，不参与属性与技能。
        if matches!(self.kind, WeaponKind::GuiYue | WeaponKind::XuanYue | WeaponKind::NoWeapon) {
            return;
        }
        let attr_bonus = if self.kind == WeaponKind::RinickModifier {
            // 0.5.5 `RinickModifier.cs()` 把八围映射成 `63 - v`（HP 为 `324 - v`），
            // 再加回自身后前七围恒为 63，HP 至少为 324。
            let mut bonus = [0; ATTR_COUNT];
            for index in 0..HP_INDEX {
                bonus[index] = 63 - attrs[index] as i32;
            }
            bonus[HP_INDEX] = (324 - attrs[HP_INDEX] as i32).max(0);
            bonus
        } else {
            self.attr_bonus
        };
        for index in 0..ATTR_COUNT {
            attrs[index] = (attrs[index] as i32 + attr_bonus[index]) as u32;
        }
        // 这些武器覆盖了 `b6()`，不会再把 `bn()` 算出的技能因子加到技能表上。
        if matches!(self.kind, WeaponKind::S11 | WeaponKind::DeathNote | WeaponKind::RinickModifier) {
            return;
        }
        let old = levels[self.skill_index];
        if old == 0 {
            boosted[self.skill_index] = true;
        }
        levels[self.skill_index] = old.saturating_add(self.skill_factor as u32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind_of(name: &str, clan: &str) -> WeaponKind {
        WeaponBuild::parse(name, clan, "alice@team").expect("weapon should build").kind
    }

    #[test]
    fn weapon_names_match_the_055_exact_table() {
        assert_eq!(kind_of("剁手刀", "team"), WeaponKind::S11);
        // 精确表未命中后才会看 `!` 后缀。
        assert_eq!(kind_of("剁手刀!", "team"), WeaponKind::BossEx);
        assert_eq!(kind_of("某剁手刀", "team"), WeaponKind::Generic);
        assert_eq!(kind_of("死亡笔记", "team"), WeaponKind::DeathNote);
        assert_eq!(kind_of("属性修改器", "Rinick"), WeaponKind::RinickModifier);
        assert_eq!(kind_of("属性修改器", "team"), WeaponKind::NoWeapon);
        // 纯菜只在桂月名单，不在玄月名单。
        assert_eq!(kind_of("桂月奖杯", "Rinick"), WeaponKind::GuiYue);
        assert_eq!(kind_of("桂月奖杯", "纯菜"), WeaponKind::GuiYue);
        assert_eq!(kind_of("桂月奖杯", "team"), WeaponKind::NoWeapon);
        assert_eq!(kind_of("玄月奖杯", "Rinick"), WeaponKind::XuanYue);
        assert_eq!(kind_of("玄月奖杯", "geometrydash"), WeaponKind::XuanYue);
        assert_eq!(kind_of("玄月奖杯", "纯菜"), WeaponKind::NoWeapon);
        assert_eq!(kind_of("玄月奖杯", "team"), WeaponKind::NoWeapon);
        assert_eq!(kind_of("saber", "team"), WeaponKind::Generic);
        assert_eq!(kind_of("saber!", "team"), WeaponKind::BossEx);
        assert_eq!(kind_of("ex", "team"), WeaponKind::Generic);
        assert_eq!(kind_of("EX", "team"), WeaponKind::Generic);
        assert!(WeaponBuild::parse("", "team", "alice@team").is_none());
        assert!(WeaponBuild::parse("diy[1,2]", "team", "alice@team").is_none());
    }

    #[test]
    fn boss_weapon_seed_matches_055_for_saber() {
        let weapon = WeaponBuild::parse("saber!", "team", "alice@team").unwrap();
        assert_eq!(weapon.kind, WeaponKind::BossEx);
        assert_eq!(
            &weapon.seed[..48],
            &[
                32, 5, 24, 53, 43, 60, 24, 15, 48, 12, 58, 10, 7, 21, 36, 39, 58, 55, 29, 19, 9, 48, 43, 37, 63, 47, 49, 19, 15,
                26, 16, 50, 3, 22, 36, 37, 5, 27, 42, 50, 16, 13, 7, 49, 52, 51, 47, 47
            ]
        );
        assert_eq!(weapon.skill_index, 3);
        assert_eq!(weapon.attr_bonus, [0, 0, 0, 0, 0, 16, 0, 0]);
    }

    #[test]
    fn plain_weapon_seed_matches_055_for_saber() {
        let weapon = WeaponBuild::parse("saber", "team", "alice@team").unwrap();
        assert_eq!(weapon.kind, WeaponKind::Generic);
        assert_eq!(
            &weapon.seed[..48],
            &[
                24, 27, 2, 38, 35, 30, 48, 39, 36, 62, 16, 54, 44, 17, 57, 40, 11, 35, 38, 4, 6, 25, 61, 7, 41, 46, 26, 53, 27,
                5, 32, 61, 53, 51, 5, 32, 62, 6, 12, 19, 20, 22, 19, 29, 9, 34, 24, 0
            ]
        );
        assert_eq!(weapon.skill_index, 30);
        assert_eq!(weapon.attr_bonus, [0, 0, 0, 20, 0, 0, 0, 0]);
    }

    #[test]
    fn boss_ex_weapon_floor_is_32() {
        let mut weapon = WeaponBuild::parse("saber!", "team", "alice@team").unwrap();
        weapon.seed[7] = 40;
        weapon.seed[8] = 10;
        weapon.seed[9] = 20;
        let mut name_base = [0u8; 128];
        name_base[7] = 30;
        name_base[8] = 30;
        name_base[9] = 40;
        weapon.pre_upgrade(&[0u8; 128], &mut name_base);
        assert_eq!(&name_base[7..10], &[40, 62, 40]);
    }

    #[test]
    fn no_effect_weapons_skip_attrs_names_and_skills() {
        for (name, clan) in [("属性修改器", "team"), ("桂月奖杯", "team"), ("玄月奖杯", "team")] {
            let mut weapon = WeaponBuild::parse(name, clan, "alice@team").unwrap();
            assert_eq!(weapon.attr_bonus, [0; 8], "{name}");
            let mut name_base = [9u8; 128];
            weapon.pre_upgrade(&[7u8; 128], &mut name_base);
            assert_eq!(name_base, [9u8; 128], "{name}");
            assert_eq!(weapon.skill_factor, 0, "{name}");
            let mut attrs = [11u32; 8];
            let mut levels = [0u32; 40];
            let mut boosted = [false; 40];
            weapon.post_upgrade(&mut attrs, &mut levels, &mut boosted);
            assert_eq!(attrs, [11u32; 8], "{name}");
            assert_eq!(levels, [0u32; 40], "{name}");
            assert!(boosted.iter().all(|flag| !flag), "{name}");
        }
    }

    #[test]
    fn s11_applies_fixed_attrs_without_skill_factor() {
        let mut weapon = WeaponBuild::parse("剁手刀", "team", "alice@team").unwrap();
        assert_eq!(weapon.kind, WeaponKind::S11);
        assert_eq!(weapon.skill_index, 16);
        assert_eq!(weapon.attr_bonus, [11, 0, 11, 0, 0, 0, 0, 0]);
        let mut name_base = [0u8; 128];
        weapon.pre_upgrade(&[0u8; 128], &mut name_base);
        let mut attrs = [10u32; 8];
        let mut levels = [0u32; 40];
        let mut boosted = [false; 40];
        weapon.post_upgrade(&mut attrs, &mut levels, &mut boosted);
        assert_eq!(attrs, [21, 10, 21, 10, 10, 10, 10, 10]);
        assert_eq!(levels, [0u32; 40]);
        assert!(boosted.iter().all(|flag| !flag));
    }

    #[test]
    fn death_note_keeps_attrs_but_skips_skill_factor() {
        let mut weapon = WeaponBuild::parse("死亡笔记", "team", "alice@team").unwrap();
        assert_eq!(weapon.kind, WeaponKind::DeathNote);
        assert_eq!(weapon.skill_index, 15);
        assert_eq!(weapon.attr_bonus, [0, 0, 0, 0, 16, 0, 3, 0]);
        let mut name_base = [0u8; 128];
        weapon.pre_upgrade(&[0u8; 128], &mut name_base);
        let mut attrs = [10u32; 8];
        let mut levels = [0u32; 40];
        let mut boosted = [false; 40];
        weapon.post_upgrade(&mut attrs, &mut levels, &mut boosted);
        let expected = std::array::from_fn(|index| (10i32 + weapon.attr_bonus[index]) as u32);
        assert_eq!(attrs, expected);
        assert_eq!(levels, [0u32; 40]);
    }

    #[test]
    fn rinick_modifier_forces_attrs_to_63_and_hp_to_324() {
        let weapon = WeaponBuild::parse("属性修改器", "Rinick", "rinick@Rinick").unwrap();
        assert_eq!(weapon.kind, WeaponKind::RinickModifier);
        let mut attrs = [80u32, 10, 63, 64, 0, 100, 7, 200];
        let mut levels = [0u32; 40];
        let mut boosted = [false; 40];
        weapon.post_upgrade(&mut attrs, &mut levels, &mut boosted);
        assert_eq!(attrs, [63, 63, 63, 63, 63, 63, 63, 324]);
    }
}

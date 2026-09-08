use crate::rc4::RC4;

const ATTR_COUNT: usize = 8;
const HP_INDEX: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WeaponKind {
    Generic,
    S11,
    DeathNote,
    BossEx,
    RinickModifier,
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
    pub(crate) fn parse(name: &str) -> Option<Self> {
        if name.is_empty() {
            return None;
        }
        let kind = if name.contains("剁手刀") {
            WeaponKind::S11
        } else if name.contains("死亡笔记") {
            WeaponKind::DeathNote
        } else if name.contains("属性修改器") {
            WeaponKind::RinickModifier
        } else if name.ends_with("ex") || name.ends_with("EX") {
            WeaponKind::BossEx
        } else {
            WeaponKind::Generic
        };
        let key = std::iter::once(0u8).chain(name.as_bytes().iter().copied()).collect::<Vec<_>>();
        let mut rc4 = RC4::new(&key, 2);
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
            skill_index: if kind == WeaponKind::S11 { 0 } else { skill_index },
            skill_factor: 0,
            kind,
        })
    }

    pub(crate) fn pre_upgrade(&mut self, raw_name_base: &[u8; 128], name_base: &mut [u8; 128]) {
        if self.kind == WeaponKind::BossEx {
            for offset in 7..10 {
                let weapon = self.seed[offset] as i32;
                let name = name_base[offset] as i32;
                let delta = weapon - name;
                name_base[offset] = if delta > 0 {
                    (name + delta) as u8
                } else if name < 63 {
                    (name + 63) as u8
                } else {
                    name as u8
                };
            }
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
        let attr_bonus = if self.kind == WeaponKind::RinickModifier {
            let mut bonus = [0; ATTR_COUNT];
            for index in 0..HP_INDEX {
                bonus[index] = (63 - attrs[index] as i32).max(0);
            }
            bonus[HP_INDEX] = (324 - attrs[HP_INDEX] as i32).max(0);
            bonus
        } else {
            self.attr_bonus
        };
        for index in 0..ATTR_COUNT {
            attrs[index] = (attrs[index] as i32 + attr_bonus[index]) as u32;
        }
        if matches!(self.kind, WeaponKind::S11 | WeaponKind::RinickModifier) {
            return;
        }
        let old = levels[self.skill_index];
        if old == 0 {
            boosted[self.skill_index] = true;
        }
        levels[self.skill_index] = old.saturating_add(self.skill_factor as u32);
    }
}

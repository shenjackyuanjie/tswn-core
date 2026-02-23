use crate::player::Player;
use crate::rc4::RC4;

pub struct Weapon {
    weapon_type: WeaponType,
}

impl Weapon {
    pub fn from_name(name: &str) -> Self {
        Self {
            weapon_type: WeaponType::from_name(name),
        }
    }

    pub fn pre_upgrade(&self, _player: &mut Player) {
        match self.weapon_type {
            // 目前仅保留挂钩位，行为集中在 post_upgrade
            WeaponType::Generic | WeaponType::DeathNote | WeaponType::S11 | WeaponType::RinickModifier | WeaponType::BossEx => {}
            WeaponType::None => {}
        }
    }

    pub fn post_upgrade(&self, player: &mut Player) {
        match self.weapon_type {
            WeaponType::None => {}
            WeaponType::S11 => {
                // 对齐 Dart WeaponS11 的固定属性加成
                player.attr[0] += 11;
                player.attr[2] += 11;
                boost_random_skill(player, 6, b"s11");
            }
            WeaponType::RinickModifier => {
                for i in 0..7 {
                    if player.attr[i] < 63 {
                        player.attr[i] = 63;
                    }
                }
                if player.attr[7] < 324 {
                    player.attr[7] = 324;
                }
                // 优先抬高护身符，保证 modifier 具备保命特征
                if let Some(skill) = player
                    .skills
                    .skill
                    .iter()
                    .copied()
                    .find(|idx| player.skills.skill_by_id(*idx).level() > 0)
                    .map(|idx| player.skills.skill_by_id_mut(idx))
                {
                    skill.boost_level(8);
                }
            }
            WeaponType::DeathNote => {
                boost_random_skill(player, 4, b"deathnote");
            }
            WeaponType::BossEx => {
                boost_random_skill(player, 8, b"bossex");
            }
            WeaponType::Generic => {
                let seed = player.weapon.clone().unwrap_or_default().into_bytes();
                boost_random_skill(player, 4, seed.as_slice());
            }
        }
    }
}

fn boost_random_skill(player: &mut Player, bonus: u32, seed: &[u8]) {
    if player.skills.skill.is_empty() {
        return;
    }
    let mut rng = RC4::new(if seed.is_empty() { b"weapon" } else { seed }, 2);
    let idx = rng.next_i32(player.skills.skill.len() as i32) as usize;
    let skill_key = player.skills.skill[idx];
    player.skills.skill_by_id_mut(skill_key).boost_level(bonus);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WeaponType {
    None,
    Generic,
    DeathNote,
    S11,
    RinickModifier,
    BossEx,
}

impl WeaponType {
    pub fn from_name(name: &str) -> Self {
        if name.is_empty() {
            return Self::None;
        }
        if name.contains("剁手刀") {
            return Self::S11;
        }
        if name.contains("死亡笔记") {
            return Self::DeathNote;
        }
        if name.contains("属性修改器") {
            return Self::RinickModifier;
        }
        if name.ends_with("ex") || name.ends_with("EX") {
            return Self::BossEx;
        }
        Self::Generic
    }
}

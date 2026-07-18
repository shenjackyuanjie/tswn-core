use std::cmp::Ordering;

use crate::player::{PlayerStatus, boss_append_attr, boss_display_name, median, skill::SkillBoost};
use crate::rc4::RC4;

use super::weapon::WeaponBuild;
use super::{NAME_MAX_LEN, NamerenaInput, PlayerClass, PlayerSpec, SkillLoadoutSpec, TEAM_MAX_LEN};

#[derive(Debug, Clone)]
pub struct PreparedPlayer {
    pub id: usize,
    pub name: String,
    pub clan_name: String,
    pub id_key_name: String,
    pub display_name: String,
    pub class: PlayerClass,
    pub attrs: [u32; 8],
    pub status: PlayerStatus,
    pub skills: SkillLoadoutSpec,
    pub name_factor: f64,
    pub weapon_attr_bonus: [i32; 8],
    pub name_base: [u8; 128],
    pub raw_name_base: [u8; 128],
}

#[derive(Debug, Clone)]
pub struct PreparedRoster {
    pub players: Vec<PreparedPlayer>,
    pub groups: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedRosterError {
    OverlayNotPrepared {
        team_index: usize,
        player_index: usize,
        raw: String,
    },
}

#[derive(Debug, Clone)]
struct PlayerBuild {
    id: usize,
    name: String,
    clan_name: String,
    id_key_name: String,
    display_name: String,
    class: PlayerClass,
    name_base: [u8; 128],
    raw_name_base: [u8; 128],
    action_order: [u32; 40],
    name_factor: f64,
    weapon: Option<WeaponBuild>,
}

impl PreparedRoster {
    pub fn build(input: &NamerenaInput, eval_rq: f64) -> Result<Self, PreparedRosterError> {
        let mut builds = Vec::new();
        let mut groups = Vec::with_capacity(input.groups.len());
        for (team_index, raw_group) in input.groups.iter().enumerate() {
            let mut group = Vec::with_capacity(raw_group.len());
            for (player_index, spec) in raw_group.iter().enumerate() {
                if spec.overlay_raw.is_some() {
                    return Err(PreparedRosterError::OverlayNotPrepared {
                        team_index,
                        player_index,
                        raw: spec.raw.clone(),
                    });
                }
                let id = builds.len();
                builds.push(PlayerBuild::new(id, spec, eval_rq));
                group.push(id);
            }
            if !group.is_empty() {
                groups.push(group);
            }
        }
        apply_team_upgrades(&mut builds, &mut groups);
        let mut build_order = (0..builds.len()).collect::<Vec<_>>();
        build_order.sort_by(|left, right| builds[*left].id_key_name.cmp(&builds[*right].id_key_name));
        let mut players = vec![None; builds.len()];
        for id in build_order {
            players[id] = Some(builds[id].clone().finish());
        }
        Ok(Self {
            players: players.into_iter().map(Option::unwrap).collect(),
            groups,
        })
    }
}

impl PlayerBuild {
    fn new(id: usize, spec: &PlayerSpec, eval_rq: f64) -> Self {
        debug_assert!(spec.name.len() <= NAME_MAX_LEN);
        debug_assert!(spec.team.as_ref().is_none_or(|team| team.len() <= TEAM_MAX_LEN));
        let clan_name = spec.team.clone().unwrap_or_else(|| spec.name.clone());
        let mut name_key = [0u8; NAME_MAX_LEN + 1];
        name_key[1..1 + spec.name.len()].copy_from_slice(spec.name.as_bytes());
        let mut team_key = [0u8; TEAM_MAX_LEN + 1];
        team_key[1..1 + clan_name.len()].copy_from_slice(clan_name.as_bytes());
        let mut rand = RC4::new(&team_key[..1 + clan_name.len()], 1);
        rand.update(&name_key[..1 + spec.name.len()], 2);
        let mut name_base = map_name_base(&rand);
        let mut raw_name_base = name_base;
        match spec.class {
            PlayerClass::Test1 => {
                for value in &mut name_base[..50] {
                    if *value < 12 {
                        *value = 63 - *value;
                    }
                }
            }
            PlayerClass::Test2 => {
                for value in &mut name_base[..50] {
                    if *value < 32 {
                        *value = 63 - *value;
                    }
                }
            }
            PlayerClass::TestEx => {
                for value in &mut name_base[6..50] {
                    if *value < 41 {
                        *value = (*value & 15) + 41;
                    }
                }
                for value in &mut name_base[50..] {
                    if *value < 16 {
                        *value += 32;
                    }
                }
                raw_name_base = name_base;
            }
            _ => {}
        }
        let mut action_order = std::array::from_fn(|index| index as u32);
        rand.sort_list(&mut action_order);
        let name_factor = match spec.class {
            PlayerClass::Test1 | PlayerClass::Test2 | PlayerClass::TestEx => 0.0,
            _ => {
                let name = crate::player::eval_name::eval_str_common_with_rq(&spec.name, true, eval_rq);
                let team = crate::player::eval_name::eval_str_common_with_rq(&clan_name, true, eval_rq);
                name.max(team - 6.0)
            }
        };
        let id_key_name = if clan_name.is_empty() || clan_name == spec.name {
            spec.name.clone()
        } else {
            format!("{}@{clan_name}", spec.name)
        };
        let display_name = if spec.class == PlayerClass::Boss {
            boss_display_name(&spec.name).to_owned()
        } else {
            spec.name.split(' ').next().unwrap_or_default().to_owned()
        };
        Self {
            id,
            name: spec.name.clone(),
            clan_name,
            id_key_name,
            display_name,
            class: spec.class,
            name_base,
            raw_name_base,
            action_order,
            name_factor,
            weapon: spec.weapon.as_deref().and_then(WeaponBuild::parse),
        }
    }

    fn upgrade(&mut self, other: &Self) {
        if self.class == PlayerClass::TestEx {
            return;
        }
        for index in 7..128 {
            if other.raw_name_base[index - 1] == self.raw_name_base[index] && other.raw_name_base[index] > self.name_base[index] {
                self.name_base[index] = other.raw_name_base[index];
            }
        }
        if self.name == self.clan_name {
            for index in 5..128 {
                if other.raw_name_base[index - 2] == self.raw_name_base[index]
                    && other.raw_name_base[index] > self.name_base[index]
                {
                    self.name_base[index] = other.raw_name_base[index];
                }
            }
        }
    }

    fn finish(mut self) -> PreparedPlayer {
        if let Some(weapon) = &mut self.weapon {
            weapon.pre_upgrade(&self.raw_name_base, &mut self.name_base);
        }
        let mut head: [u8; 10] = self.name_base[..10].try_into().expect("name head has fixed size");
        head.sort_unstable();
        let mut attrs = [0u32; 8];
        for (attr, offset) in attrs[..7].iter_mut().zip((10..31).step_by(3)) {
            *attr = u32::from(median(
                self.name_base[offset],
                self.name_base[offset + 1],
                self.name_base[offset + 2],
            ));
        }
        attrs[7] = 154 + u32::from(head[3]) + u32::from(head[4]) + u32::from(head[5]) + u32::from(head[6]);
        if self.class == PlayerClass::Boss {
            for (attr, bonus) in attrs.iter_mut().zip(boss_append_attr(&self.name)) {
                *attr = (*attr as i32 + bonus).max(0) as u32;
            }
        }

        let mut levels = [0u32; 40];
        let mut boosted = [false; 40];
        let mut boosts: [Option<SkillBoost>; 40] = std::array::from_fn(|_| None);
        let mut slot_skill_keys = [None; 16];
        if self.class != PlayerClass::Boss {
            for (slot, offset) in (64..128).step_by(4).enumerate() {
                let small = self.name_base[offset..offset + 4].iter().copied().min().unwrap();
                let key = self.action_order[slot] as usize;
                if small <= 10 || key >= 35 {
                    continue;
                }
                levels[key] = u32::from(small - 10);
                boosted[key] = self.raw_name_base[offset..offset + 4].iter().copied().min().unwrap() <= 10;
                slot_skill_keys[slot] = Some(key);
            }
        }
        if let Some(weapon) = &self.weapon {
            weapon.post_upgrade(&mut attrs, &mut levels, &mut boosted);
        }
        for &key in self.action_order.iter().rev() {
            let key = key as usize;
            if key >= 25 || levels[key] == 0 || boosted[key] {
                continue;
            }
            let base = levels[key];
            levels[key] = base.saturating_mul(2);
            boosted[key] = true;
            boosts[key] = Some(SkillBoost::LastBoost(base));
            break;
        }
        for (slot, left, right) in [(14usize, 60usize, 61usize), (15, 62, 63)] {
            let Some(key) = slot_skill_keys[slot] else {
                continue;
            };
            if levels[key] == 0 || boosted[key] {
                continue;
            }
            let base = levels[key];
            let amount = u32::from(self.name_base[left].min(self.name_base[right])).min(base);
            levels[key] = base.saturating_add(amount);
            boosted[key] = true;
            boosts[key] = Some(SkillBoost::SlotBoost { base, boost: amount });
        }

        let scale = |value: i32, factor: i32| (value as f64 * (1.0 - self.name_factor / factor as f64)).round() as i32;
        let attr_sum = attrs[..7].iter().sum();
        let atk_sum = (attrs[0] as i32 - attrs[1] as i32 + attrs[2] as i32 + attrs[4] as i32 - attrs[5] as i32) * 2
            + attrs[3] as i32
            + attrs[6] as i32;
        let status = PlayerStatus {
            alive: self.class != PlayerClass::Seed,
            hp: attrs[7] as i32,
            max_hp: attrs[7] as i32,
            attack: scale(attrs[0] as i32, 128),
            defense: scale(attrs[1] as i32, 128),
            speed: scale(attrs[2] as i32, 128) + 160,
            agility: scale(attrs[3] as i32, 128),
            magic: scale(attrs[4] as i32, 128),
            magic_point: scale(attrs[6] as i32, 80) >> 1,
            resistance: scale(attrs[5] as i32, 128),
            wisdom: scale(attrs[6] as i32, 80),
            attr_sum,
            atk_sum,
            all_sum: attr_sum * 3 + attrs[7],
            ..PlayerStatus::default()
        };
        let skills = SkillLoadoutSpec::standard(&levels, &boosted, &boosts, &self.action_order);
        let weapon_attr_bonus = self.weapon.as_ref().map_or([0; 8], |weapon| weapon.attr_bonus);
        PreparedPlayer {
            id: self.id,
            name: self.name,
            clan_name: self.clan_name,
            id_key_name: self.id_key_name,
            display_name: self.display_name,
            class: self.class,
            attrs,
            status,
            skills,
            name_factor: self.name_factor,
            weapon_attr_bonus,
            name_base: self.name_base,
            raw_name_base: self.raw_name_base,
        }
    }
}

fn apply_team_upgrades(players: &mut [PlayerBuild], groups: &mut [Vec<usize>]) {
    for group in groups {
        group.sort_by(|left, right| compare_builds(&players[*left], &players[*right]));
        for left_index in 0..group.len() {
            for right_index in (left_index + 1)..group.len() {
                let left_id = group[left_index];
                let right_id = group[right_index];
                if players[left_id].clan_name != players[right_id].clan_name {
                    continue;
                }
                let (left, right) = two_mut(players, left_id, right_id);
                left.upgrade(right);
                right.upgrade(left);
            }
        }
    }
}

fn compare_builds(left: &PlayerBuild, right: &PlayerBuild) -> Ordering {
    left.id_key_name.cmp(&right.id_key_name).then_with(|| left.id.cmp(&right.id))
}

fn two_mut(values: &mut [PlayerBuild], left: usize, right: usize) -> (&mut PlayerBuild, &mut PlayerBuild) {
    if left < right {
        let (before, after) = values.split_at_mut(right);
        (&mut before[left], &mut after[0])
    } else {
        let (before, after) = values.split_at_mut(left);
        (&mut after[0], &mut before[right])
    }
}

fn map_name_base(rand: &RC4) -> [u8; 128] {
    let mut output = [0u8; 128];
    let mut index = 0;
    for &value in &rand.main_val {
        let mapped = ((u32::from(value) * 181) + 160) & 255;
        if (89..217).contains(&mapped) {
            output[index] = (mapped & 63) as u8;
            index += 1;
        }
    }
    assert_eq!(index, output.len());
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::storage::Storage;
    use crate::player::Player;

    fn assert_status(actual: &PlayerStatus, expected: &PlayerStatus) {
        assert_eq!(actual.alive, expected.alive);
        assert_eq!(actual.hp, expected.hp);
        assert_eq!(actual.max_hp, expected.max_hp);
        assert_eq!(actual.attack, expected.attack);
        assert_eq!(actual.defense, expected.defense);
        assert_eq!(actual.speed, expected.speed);
        assert_eq!(actual.agility, expected.agility);
        assert_eq!(actual.magic, expected.magic);
        assert_eq!(actual.magic_point, expected.magic_point);
        assert_eq!(actual.resistance, expected.resistance);
        assert_eq!(actual.wisdom, expected.wisdom);
        assert_eq!(actual.attr_sum, expected.attr_sum);
        assert_eq!(actual.atk_sum, expected.atk_sum);
        assert_eq!(actual.all_sum, expected.all_sum);
        assert_eq!(actual.at_boost.to_bits(), expected.at_boost.to_bits());
        assert_eq!(actual.attract.to_bits(), expected.attract.to_bits());
    }

    #[test]
    fn native_attributes_and_skills_match_legacy_builds() {
        let cases = [
            "alice",
            "alice@red",
            "covid@!",
            "云剑狄卡敢@!",
            "target@!",
            "target@\u{0002}",
            "target@\u{0003}",
            "alice@red+普通武器",
            "alice@red+剁手刀",
            "alice@red+死亡笔记",
            "alice@red+属性修改器",
            "alice@red+bladeEX",
        ];
        for raw in cases {
            let input = NamerenaInput::parse(raw).unwrap();
            let actual = PreparedRoster::build(&input, crate::player::eval_name::DEFAULT_EVAL_RQ)
                .unwrap()
                .players
                .remove(0);
            let storage = Storage::new_arc();
            let mut expected = Player::new_from_namerena_raw(raw.to_owned(), storage).unwrap();
            expected.build();
            if expected.player_type() == crate::player::PlayerType::Boss {
                crate::player::boss::init_boss_state(&mut expected);
            }
            assert_eq!(actual.name_base.as_slice(), expected.name_base.as_slice(), "{raw}");
            let (attrs, weapon_bonus, name_factor) = expected.clone_build_inputs();
            assert_eq!(actual.attrs, attrs, "{raw}");
            assert_eq!(actual.weapon_attr_bonus, weapon_bonus, "{raw}");
            assert_eq!(actual.name_factor.to_bits(), name_factor.to_bits(), "{raw}");
            assert_status(&actual.status, expected.get_status());
            let snapshot = expected.skill_loadout_snapshot();
            for entry in &actual.skills.entries {
                let legacy = snapshot.entries.iter().find(|legacy| legacy.key == entry.key).unwrap();
                assert_eq!(entry.level, legacy.level, "{raw}, skill {}", entry.key);
                assert_eq!(entry.boosted, legacy.boosted, "{raw}, skill {}", entry.key);
                assert_eq!(entry.boost, legacy.boost, "{raw}, skill {}", entry.key);
            }
            assert_eq!(actual.skills.fixed_lanes, snapshot.fixed_lanes[..35], "{raw}");
            assert_eq!(
                actual.skills.active_order,
                snapshot.active_order.iter().copied().filter(|key| *key < 35).collect::<Vec<_>>(),
                "{raw}"
            );
            assert_eq!(actual.skills.pre_action_order, snapshot.pre_action_order, "{raw}");
            assert_eq!(actual.skills.post_damage_order, snapshot.post_damage_order, "{raw}");
            let config = crate::runtime::default_custom_runtime_import_config().unwrap();
            let importer = crate::runtime::PlainLegacySkillImportMap::new(&config.registry);
            assert_eq!(
                importer.import_namerena(&actual.skills),
                importer.import(&snapshot),
                "{raw}: Runtime skill loadout"
            );
        }
    }

    #[test]
    fn native_team_upgrades_match_legacy_builds() {
        let raw = "alice@red\nbob@red\n\nsolo";
        let input = NamerenaInput::parse(raw).unwrap();
        let actual = PreparedRoster::build(&input, crate::player::eval_name::DEFAULT_EVAL_RQ).unwrap();
        let storage = Storage::new_arc();
        let mut expected = input
            .groups
            .iter()
            .flatten()
            .map(|spec| Player::new_from_namerena_raw(spec.raw.clone(), storage.clone()).unwrap())
            .collect::<Vec<_>>();
        let mut groups = actual.groups.clone();
        for group in &mut groups {
            group.sort_by(|left, right| expected[*left].cmp_for_sort(&expected[*right]));
            for left_index in 0..group.len() {
                for right_index in (left_index + 1)..group.len() {
                    let left_id = group[left_index];
                    let right_id = group[right_index];
                    if expected[left_id].clan_name() != expected[right_id].clan_name() {
                        continue;
                    }
                    let (left, right) = if left_id < right_id {
                        let (before, after) = expected.split_at_mut(right_id);
                        (&mut before[left_id], &mut after[0])
                    } else {
                        let (before, after) = expected.split_at_mut(left_id);
                        (&mut after[0], &mut before[right_id])
                    };
                    left.upgrade(right);
                    right.upgrade(left);
                }
            }
        }
        let mut order = (0..expected.len()).collect::<Vec<_>>();
        order.sort_by(|left, right| expected[*left].cmp_by_id_name(&expected[*right]));
        for id in order {
            expected[id].build();
            if expected[id].player_type() == crate::player::PlayerType::Boss {
                crate::player::boss::init_boss_state(&mut expected[id]);
            }
        }
        for (actual, expected) in actual.players.iter().zip(&expected) {
            assert_eq!(actual.name_base.as_slice(), expected.name_base.as_slice());
            assert_eq!(actual.attrs, expected.clone_build_inputs().0);
            assert_status(&actual.status, expected.get_status());
        }
    }
}

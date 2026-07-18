use std::cmp::Ordering;

use crate::player::{PlayerStatus, boss_append_attr, boss_display_name, median, skill::SkillBoost};
use crate::rc4::RC4;

use super::weapon::WeaponBuild;
use super::{NAME_MAX_LEN, NamerenaInput, PlayerClass, PlayerOverlay, PlayerSpec, SkillLoadoutSpec, TEAM_MAX_LEN};

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
    pub overlay: Option<PlayerOverlay>,
}

#[derive(Debug, Clone)]
pub struct PreparedRoster {
    pub players: Vec<PreparedPlayer>,
    pub groups: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinionKind {
    Shadow,
    Summon,
    Zombie,
}

#[derive(Debug, Clone)]
pub struct PreparedMinionBlueprint {
    pub player: PreparedPlayer,
    pub kind: MinionKind,
    pub reserved_player_ids_before_spawn: usize,
    pub reuse_skills_on_recast: bool,
    pub reuse_stats_on_recast: bool,
    pub inherit_owner_def_res: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedRosterError {}

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
    overlay: Option<PlayerOverlay>,
}

impl PreparedRoster {
    pub fn build(input: &NamerenaInput, eval_rq: f64) -> Result<Self, PreparedRosterError> {
        let mut builds = Vec::new();
        let mut groups = Vec::with_capacity(input.groups.len());
        for raw_group in &input.groups {
            let mut group = Vec::with_capacity(raw_group.len());
            for spec in raw_group {
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

impl PreparedPlayer {
    pub fn minion_blueprint(&self, kind: MinionKind, eval_rq: f64) -> PreparedMinionBlueprint {
        let (suffix, display_name) = match kind {
            MinionKind::Shadow => ("shadow", "幻影"),
            MinionKind::Summon => ("summon", "使魔"),
            MinionKind::Zombie => ("zombie", "丧尸"),
        };
        let name = format!("{}?{suffix}", self.name);
        let spec = PlayerSpec {
            raw: format!("{name}@{}", self.clan_name),
            name: name.clone(),
            team: Some(self.clan_name.clone()),
            weapon: None,
            overlay: None,
            class: PlayerClass::Normal,
        };
        let mut build = PlayerBuild::new(0, &spec, eval_rq);
        build.name_factor = 0.0;
        build.weapon = None;
        let mut player = build.finish();
        let overlay = self.minion_overlay(kind);

        if let Some(attrs) = overlay.and_then(|overlay| overlay.attrs) {
            player.attrs = attrs.map(|value| value.max(0) as u32);
            if kind == MinionKind::Summon && overlay.is_some_and(|overlay| overlay.inherit_owner_def_res) {
                player.attrs[1] = self.attrs[1];
                player.attrs[5] = self.attrs[5];
            }
        } else {
            match kind {
                MinionKind::Shadow => player.attrs[7] /= 2,
                MinionKind::Summon => {
                    player.attrs[7] = (player.attrs[7] / 3).max(1);
                    player.attrs[0] = 0;
                    player.attrs[1] = self.attrs[1];
                    player.attrs[4] = 0;
                    player.attrs[5] = self.attrs[5];
                }
                MinionKind::Zombie => {
                    player.attrs[0] = 0;
                    player.attrs[6] = 0;
                    player.attrs[7] = (player.attrs[7] >> 1).max(1);
                }
            }
        }
        player.status = status_from_attrs(player.attrs, 0.0, true);
        player.class = PlayerClass::Clone;
        player.display_name = display_name.to_owned();
        player.name_factor = 0.0;
        player.weapon_attr_bonus = [0; 8];
        player.overlay = overlay.and_then(|overlay| overlay.child_player_overlay());

        player.skills = match (kind, overlay.and_then(|overlay| overlay.skills.as_ref())) {
            (MinionKind::Shadow | MinionKind::Zombie, Some(skills)) => SkillLoadoutSpec::generic_minion_overlay(skills),
            (MinionKind::Summon, Some(skills)) => SkillLoadoutSpec::summon_overlay(skills, true),
            (MinionKind::Shadow, None) => {
                let raw = player.name_base[64..68].iter().copied().min().unwrap_or(0);
                SkillLoadoutSpec::shadow_default(((i32::from(raw) - 10) / 2 + 36).max(0) as u32)
            }
            (MinionKind::Summon, None) => {
                let levels = std::array::from_fn(|slot| {
                    let offset = 64 + slot * 4;
                    u32::from(player.name_base[offset..offset + 4].iter().copied().min().unwrap_or(0).saturating_sub(10))
                });
                let mut order = [0usize, 1, 2];
                name_rng(&self.clan_name, &name).sort_list(&mut order);
                SkillLoadoutSpec::summon_default(levels, order)
            }
            (MinionKind::Zombie, None) => SkillLoadoutSpec::default(),
        };
        if kind == MinionKind::Shadow {
            player.status.move_point = if self.status.at_boost >= 3.0 { 2048 } else { -2048 };
        }

        let has_overlay_attrs = overlay.is_some_and(|overlay| overlay.attrs.is_some());
        PreparedMinionBlueprint {
            player,
            kind,
            reserved_player_ids_before_spawn: usize::from(matches!(kind, MinionKind::Summon | MinionKind::Zombie)),
            reuse_skills_on_recast: kind == MinionKind::Summon && overlay.is_none_or(|overlay| overlay.reuse_skills_on_recast),
            reuse_stats_on_recast: kind == MinionKind::Summon && !has_overlay_attrs,
            inherit_owner_def_res: kind == MinionKind::Summon
                && (!has_overlay_attrs || overlay.is_some_and(|overlay| overlay.inherit_owner_def_res)),
        }
    }

    fn minion_overlay(&self, kind: MinionKind) -> Option<&crate::namerena::MinionOverlay> {
        let overlay = self.overlay.as_ref()?;
        match kind {
            MinionKind::Shadow => overlay.shadow.as_ref(),
            MinionKind::Summon => overlay.summon.as_ref(),
            MinionKind::Zombie => overlay.zombie.as_ref(),
        }
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
        if spec.overlay.as_ref().and_then(|overlay| overlay.skills.as_ref()).is_none() {
            rand.sort_list(&mut action_order);
        }
        let name_factor = match spec.class {
            PlayerClass::Test1 | PlayerClass::Test2 | PlayerClass::TestEx => 0.0,
            _ if spec.overlay.as_ref().is_some_and(|overlay| !overlay.name_factor_enabled) => 0.0,
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
        let has_diy = spec
            .overlay
            .as_ref()
            .is_some_and(|overlay| overlay.attrs.is_some() || overlay.skills.is_some());
        let weapon_name = spec
            .weapon
            .as_deref()
            .or_else(|| spec.overlay.as_ref().and_then(|overlay| overlay.weapon.as_deref()));
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
            weapon: (!has_diy).then(|| weapon_name.and_then(WeaponBuild::parse)).flatten(),
            overlay: spec.overlay.clone(),
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
        if let Some(overlay_attrs) = self.overlay.as_ref().and_then(|overlay| overlay.attrs) {
            attrs = overlay_attrs.map(|value| value.max(0) as u32);
        } else if self.class == PlayerClass::Boss {
            for (attr, bonus) in attrs.iter_mut().zip(boss_append_attr(&self.name)) {
                *attr = (*attr as i32 + bonus).max(0) as u32;
            }
        }

        let mut levels = [0u32; 40];
        let mut boosted = [false; 40];
        let mut boosts: [Option<SkillBoost>; 40] = std::array::from_fn(|_| None);
        let mut slot_skill_keys = [None; 16];
        let overlay_skills = self.overlay.as_ref().and_then(|overlay| overlay.skills.as_ref());
        if overlay_skills.is_none() && self.class != PlayerClass::Boss {
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
        if overlay_skills.is_none() {
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
        let skills = overlay_skills.map_or_else(
            || SkillLoadoutSpec::standard(&levels, &boosted, &boosts, &self.action_order),
            |skills| SkillLoadoutSpec::player_overlay(skills),
        );
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
            overlay: self.overlay,
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

fn name_rng(team: &str, name: &str) -> RC4 {
    let mut team_key = [0u8; TEAM_MAX_LEN + 1];
    team_key[1..1 + team.len()].copy_from_slice(team.as_bytes());
    let mut name_key = [0u8; NAME_MAX_LEN + 1];
    name_key[1..1 + name.len()].copy_from_slice(name.as_bytes());
    let mut rng = RC4::new(&team_key[..1 + team.len()], 1);
    rng.update(&name_key[..1 + name.len()], 2);
    rng
}

fn status_from_attrs(attrs: [u32; 8], name_factor: f64, alive: bool) -> PlayerStatus {
    let scale = |value: i32, factor: i32| (value as f64 * (1.0 - name_factor / factor as f64)).round() as i32;
    let attr_sum = attrs[..7].iter().sum();
    let atk_sum = (attrs[0] as i32 - attrs[1] as i32 + attrs[2] as i32 + attrs[4] as i32 - attrs[5] as i32) * 2
        + attrs[3] as i32
        + attrs[6] as i32;
    let wisdom = scale(attrs[6] as i32, 80);
    PlayerStatus {
        alive,
        hp: attrs[7] as i32,
        max_hp: attrs[7] as i32,
        attack: scale(attrs[0] as i32, 128),
        defense: scale(attrs[1] as i32, 128),
        speed: scale(attrs[2] as i32, 128) + 160,
        agility: scale(attrs[3] as i32, 128),
        magic: scale(attrs[4] as i32, 128),
        magic_point: wisdom >> 1,
        resistance: scale(attrs[5] as i32, 128),
        wisdom,
        attr_sum,
        atk_sum,
        all_sum: attr_sum * 3 + attrs[7],
        ..PlayerStatus::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::storage::Storage;
    use crate::player::Player;

    fn assert_status(actual: &PlayerStatus, expected: &PlayerStatus) {
        assert_eq!(actual.frozen, expected.frozen);
        assert_eq!(actual.alive, expected.alive);
        assert_eq!(actual.point, expected.point);
        assert_eq!(actual.move_point, expected.move_point);
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

    fn assert_skills_match_legacy(actual: &SkillLoadoutSpec, expected: &Player, context: &str) {
        let snapshot = expected.skill_loadout_snapshot();
        let actual_keys = actual.entries.iter().map(|entry| entry.key).collect::<Vec<_>>();
        for entry in &actual.entries {
            let legacy = snapshot.entries.iter().find(|legacy| legacy.key == entry.key).unwrap();
            assert_eq!(entry.level, legacy.level, "{context}, skill {}", entry.key);
            assert_eq!(entry.boosted, legacy.boosted, "{context}, skill {}", entry.key);
            assert_eq!(entry.boost, legacy.boost, "{context}, skill {}", entry.key);
        }
        assert_eq!(
            actual.fixed_lanes,
            snapshot
                .fixed_lanes
                .iter()
                .copied()
                .filter(|key| actual_keys.contains(key))
                .collect::<Vec<_>>(),
            "{context}: fixed lanes"
        );
        assert_eq!(
            actual.active_order,
            snapshot
                .active_order
                .iter()
                .copied()
                .filter(|key| actual_keys.contains(key))
                .collect::<Vec<_>>(),
            "{context}: active order"
        );
        assert_eq!(
            actual.pre_action_order, snapshot.pre_action_order,
            "{context}: pre-action order"
        );
        assert_eq!(
            actual.post_damage_order, snapshot.post_damage_order,
            "{context}: post-damage order"
        );
        assert_eq!(
            actual.post_action_after_states, snapshot.post_action_after_states,
            "{context}: post-action state order"
        );
        let config = crate::runtime::default_custom_runtime_import_config().unwrap();
        let importer = crate::runtime::PlainLegacySkillImportMap::new(&config.registry);
        assert_eq!(
            importer.import_namerena(actual),
            importer.import(&snapshot),
            "{context}: Runtime skill loadout"
        );
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
            r#"mario+diy[72,39,69,76,67,66,0,84]{"sklfire":5,"sklheal":"40+30","sklshadow":"2*4"}"#,
            r#"luigi+ol:{"attrs":[37,38,39,40,41,42,43,300],"skills":{"fire":4},"weapon":"剁手刀"}"#,
            r#"aaaaa+ol:{"attrs":[86,86,86,86,86,86,86,300],"name_factor_enabled":false}"#,
            r#"owner@same+ol:{"attrs":[86,86,86,86,86,86,86,300],"skills":{"sklfire":3,"sklfire1":5,"summon:sklfire2":7,"sklexplode":11,"sklpossess":13}}"#,
            r#"alice@red+ol:{"weapon":"剁手刀"}"#,
            r#"alice@red+普通武器+ol:{"weapon":"剁手刀"}"#,
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
            assert_eq!(actual.name, expected.base_name(), "{raw}");
            assert_eq!(actual.clan_name, expected.clan_name(), "{raw}");
            assert_eq!(actual.id_key_name, expected.id_key_name(), "{raw}");
            assert_eq!(actual.display_name, expected.display_name(), "{raw}");
            assert_eq!(actual.overlay.as_ref(), expected.overlay.as_deref(), "{raw}");
            assert_status(&actual.status, expected.get_status());
            assert_skills_match_legacy(&actual.skills, &expected, raw);
        }
    }

    #[test]
    fn native_minion_blueprints_match_legacy_builds() {
        use crate::player::skill::act::minion::MinionBlueprintOwner;

        let cases = [
            "owner@same",
            r#"owner@same+ol:{"attrs":[86,86,86,86,86,86,86,300],"shadow":{"attrs":[46,47,48,49,50,51,52,200],"skills":{"sklpossess":9}},"summon":{"attrs":[50,51,52,53,54,55,56,180],"skills":{"sklfire2":12,"sklexplode":3,"sklfire1":"2*4"}},"zombie":{"attrs":[40,41,42,43,44,45,46,90],"skills":{"sklrapid":7}}}"#,
            r#"owner@same+ol:{"attrs":[86,86,86,86,86,86,86,300],"shadow":{"skills":{"phantom:sklpossess":5,"normal:sklrapid":7}},"summon":{"attrs":[50,51,52,53,54,55,56,180],"skills":{"normal:sklrapid":9,"sklfire1":5,"summon:sklexplode":3},"reuse_skills_on_recast":true,"inherit_owner_def_res":true},"zombie":{"skills":{"normal:sklheal":"40+30"}}}"#,
        ];
        for raw in cases {
            let input = NamerenaInput::parse(raw).unwrap();
            let owner = PreparedRoster::build(&input, crate::player::eval_name::DEFAULT_EVAL_RQ)
                .unwrap()
                .players
                .remove(0);
            let storage = Storage::new_arc();
            let mut legacy_owner = Player::new_from_namerena_raw(raw.to_owned(), storage.clone()).unwrap();
            legacy_owner.build();
            let legacy_owner = MinionBlueprintOwner::from_player(0, &legacy_owner);

            for kind in [MinionKind::Shadow, MinionKind::Summon, MinionKind::Zombie] {
                let actual = owner.minion_blueprint(kind, crate::player::eval_name::DEFAULT_EVAL_RQ);
                let expected = match kind {
                    MinionKind::Shadow => {
                        crate::player::skill::act::shadow::build_shadow_minion_from_owner(&legacy_owner, &storage)
                    }
                    MinionKind::Summon => {
                        crate::player::skill::act::summon::build_summon_minion_from_owner(&legacy_owner, &storage, true)
                    }
                    MinionKind::Zombie => {
                        crate::player::skill::skl::zombie::build_zombie_minion_blueprint_from_owner(&legacy_owner, &storage)
                    }
                };
                let context = format!("{raw}, {kind:?}");
                let (attrs, weapon_bonus, name_factor) = expected.clone_build_inputs();
                assert_eq!(actual.player.attrs, attrs, "{context}: attrs");
                assert_eq!(actual.player.weapon_attr_bonus, weapon_bonus, "{context}: weapon bonus");
                assert_eq!(
                    actual.player.name_factor.to_bits(),
                    name_factor.to_bits(),
                    "{context}: name factor"
                );
                assert_eq!(
                    actual.player.name_base.as_slice(),
                    expected.name_base.as_slice(),
                    "{context}: name base"
                );
                assert_eq!(actual.player.name, expected.base_name(), "{context}: name");
                assert_eq!(actual.player.clan_name, expected.clan_name(), "{context}: clan");
                assert_eq!(actual.player.id_key_name, expected.id_key_name(), "{context}: id key");
                assert_eq!(actual.player.display_name, expected.display_name(), "{context}: display name");
                assert_eq!(
                    actual.player.overlay.as_ref(),
                    expected.overlay.as_deref(),
                    "{context}: child overlay"
                );
                assert_eq!(actual.player.class, PlayerClass::Clone, "{context}: class");
                assert_eq!(
                    expected.player_type(),
                    crate::player::PlayerType::Clone,
                    "{context}: legacy class"
                );
                assert_status(&actual.player.status, expected.get_status());
                assert_skills_match_legacy(&actual.player.skills, &expected, &context);

                assert_eq!(
                    actual.reserved_player_ids_before_spawn,
                    usize::from(matches!(kind, MinionKind::Summon | MinionKind::Zombie)),
                    "{context}: reserved ids"
                );
                if kind != MinionKind::Summon {
                    assert!(!actual.reuse_skills_on_recast, "{context}: skill reuse");
                    assert!(!actual.reuse_stats_on_recast, "{context}: stat reuse");
                    assert!(!actual.inherit_owner_def_res, "{context}: inherited attrs");
                } else {
                    let overlay = owner.overlay.as_ref().and_then(|overlay| overlay.summon.as_ref());
                    let has_overlay_attrs = overlay.is_some_and(|overlay| overlay.attrs.is_some());
                    assert_eq!(
                        actual.reuse_skills_on_recast,
                        overlay.is_none_or(|overlay| overlay.reuse_skills_on_recast),
                        "{context}: skill reuse"
                    );
                    assert_eq!(actual.reuse_stats_on_recast, !has_overlay_attrs, "{context}: stat reuse");
                    assert_eq!(
                        actual.inherit_owner_def_res,
                        !has_overlay_attrs || overlay.is_some_and(|overlay| overlay.inherit_owner_def_res),
                        "{context}: inherited attrs"
                    );
                }
            }
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

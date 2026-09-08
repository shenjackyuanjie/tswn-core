use std::cmp::Ordering;

use crate::rc4::RC4;

use super::weapon::WeaponBuild;
use super::{
    NAME_MAX_LEN, NamerenaInput, PlayerClass, PlayerOverlay, PlayerSpec, PlayerStats, SkillBoost, SkillLoadoutSpec, TEAM_MAX_LEN,
    boss_append_attr, boss_display_name, median,
};

#[derive(Debug, Clone)]
pub struct PreparedPlayer {
    pub id: usize,
    pub name: String,
    pub clan_name: String,
    pub id_key_name: String,
    pub display_name: String,
    pub class: PlayerClass,
    pub attrs: [u32; 8],
    pub status: PlayerStats,
    pub skills: SkillLoadoutSpec,
    pub name_factor: f64,
    pub weapon_attr_bonus: [i32; 8],
    pub name_base: [u8; 128],
    pub raw_name_base: [u8; 128],
    pub clone_initially_boosted_mask: Option<u64>,
    pub clone_slot_boosts: [Option<(u8, u8)>; 2],
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
    normal_raw_name_base: [u8; 128],
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

/// 预先计算 score profile 共用的队名 KSA 状态。
pub(crate) fn score_profile_team_rng(team: &str) -> RC4 {
    assert!(team.len() <= TEAM_MAX_LEN, "score profile team name is too long");
    let mut key = [0u8; TEAM_MAX_LEN + 1];
    key[1..1 + team.len()].copy_from_slice(team.as_bytes());
    RC4::new(&key[..1 + team.len()], 1)
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
        let normal_raw_name_base = name_base;
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
                let name = crate::namerena::eval_name::eval_str_common_with_rq(&spec.name, true, eval_rq);
                let team = crate::namerena::eval_name::eval_str_common_with_rq(&clan_name, true, eval_rq);
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
            normal_raw_name_base,
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
        let clone_initially_boosted_mask = (overlay_skills.is_none()
            && self.weapon.is_none()
            && matches!(self.class, PlayerClass::Test1 | PlayerClass::Test2 | PlayerClass::TestEx))
        .then(|| {
            let mut mask = 0u64;
            for (slot, offset) in (64..128).step_by(4).enumerate() {
                let Some(key) = slot_skill_keys[slot] else {
                    continue;
                };
                if self.normal_raw_name_base[offset..offset + 4].iter().copied().min().unwrap() <= 10 {
                    mask |= 1u64 << key;
                }
            }
            mask
        });
        let clone_slot_boosts = [(14usize, 60usize, 61usize), (15, 62, 63)].map(|(slot, left, right)| {
            slot_skill_keys[slot].map(|key| {
                (
                    key.try_into().expect("clone skill key must fit u8"),
                    self.name_base[left].min(self.name_base[right]),
                )
            })
        });
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
        let mut status = PlayerStats {
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
            ..PlayerStats::default()
        };
        // 旧版 boss 初始化会在构建后安装 runtime 状态。TestSubject 也会替换复制到 Runtime 模板中的
        // 可观察冷快照。
        if self.class == PlayerClass::Boss && self.name == "testsubject" {
            self.name_factor = 0.0;
            attrs = [80, 80, 80, 80, 80, 80, 80, 100];
            status = status_from_attrs(attrs, self.name_factor, true);
            status.magic_point = 1000;
        }
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
            clone_initially_boosted_mask,
            clone_slot_boosts,
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

fn status_from_attrs(attrs: [u32; 8], name_factor: f64, alive: bool) -> PlayerStats {
    let scale = |value: i32, factor: i32| (value as f64 * (1.0 - name_factor / factor as f64)).round() as i32;
    let attr_sum = attrs[..7].iter().sum();
    let atk_sum = (attrs[0] as i32 - attrs[1] as i32 + attrs[2] as i32 + attrs[4] as i32 - attrs[5] as i32) * 2
        + attrs[3] as i32
        + attrs[6] as i32;
    let wisdom = scale(attrs[6] as i32, 80);
    PlayerStats {
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
        ..PlayerStats::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_roster_builds_representative_inputs_deterministically() {
        let raw = concat!(
            "alice@red+剁手刀\n",
            "covid@!\n",
            "target@\u{0002}\n",
            r#"diy@red+diy[72,39,69,76,67,66,36,84]{"sklfire":5,"sklheal":"40+30"}"#,
            "\n\nplain@blue"
        );
        let input = NamerenaInput::parse(raw).unwrap();
        let first = PreparedRoster::build(&input, crate::namerena::eval_name::DEFAULT_EVAL_RQ).unwrap();
        let second = PreparedRoster::build(&input, crate::namerena::eval_name::DEFAULT_EVAL_RQ).unwrap();

        assert_eq!(first.groups, vec![vec![0, 1, 3, 2], vec![4]]);
        assert_eq!(first.players.len(), 5);
        for (id, (left, right)) in first.players.iter().zip(&second.players).enumerate() {
            assert_eq!(left.id, id);
            assert_eq!(left.attrs, right.attrs);
            assert_eq!(left.status, right.status);
            assert_eq!(left.skills, right.skills);
            assert!(left.status.max_hp > 0);
        }
        let diy = &first.players[3];
        assert_eq!(diy.attrs, [36, 3, 33, 40, 31, 30, 0, 84]);
        assert_eq!(diy.skills.entries.iter().find(|entry| entry.key == 0).unwrap().level, 5);
        assert_eq!(diy.skills.entries.iter().find(|entry| entry.key == 15).unwrap().level, 70);
    }

    #[test]
    fn native_minion_blueprints_apply_overlay_data() {
        let raw = r#"owner@same+ol:{"attrs":[86,86,86,86,86,86,86,300],"skills":{"sklshadow":10,"sklsummon":10,"sklzombie":10},"shadow":{"attrs":[46,47,48,49,50,51,52,200],"skills":{"phantom:sklpossess":9}},"summon":{"attrs":[50,51,52,53,54,55,56,180],"skills":{"normal:sklrapid":9,"sklfire1":5,"summon:sklexplode":3},"reuse_skills_on_recast":true,"inherit_owner_def_res":true},"zombie":{"attrs":[40,41,42,43,44,45,46,90],"skills":{"normal:sklheal":7}}}"#;
        let input = NamerenaInput::parse(raw).unwrap();
        let owner = PreparedRoster::build(&input, crate::namerena::eval_name::DEFAULT_EVAL_RQ)
            .unwrap()
            .players
            .remove(0);

        let shadow = owner.minion_blueprint(MinionKind::Shadow, crate::namerena::eval_name::DEFAULT_EVAL_RQ);
        let summon = owner.minion_blueprint(MinionKind::Summon, crate::namerena::eval_name::DEFAULT_EVAL_RQ);
        let zombie = owner.minion_blueprint(MinionKind::Zombie, crate::namerena::eval_name::DEFAULT_EVAL_RQ);

        assert_eq!(shadow.player.attrs, [10, 11, 12, 13, 14, 15, 16, 200]);
        assert_eq!(summon.player.attrs, [14, 50, 16, 17, 18, 50, 20, 180]);
        assert_eq!(zombie.player.attrs, [4, 5, 6, 7, 8, 9, 10, 90]);
        assert_eq!(shadow.player.class, PlayerClass::Clone);
        assert_eq!(summon.reserved_player_ids_before_spawn, 1);
        assert!(summon.reuse_skills_on_recast);
        assert!(!summon.reuse_stats_on_recast);
        assert!(summon.inherit_owner_def_res);
        assert_eq!(zombie.reserved_player_ids_before_spawn, 1);
    }

    #[test]
    fn native_team_upgrade_keeps_dense_ids_and_group_layout() {
        let input = NamerenaInput::parse("alice@red\nbob@red\n\nsolo").unwrap();
        let roster = PreparedRoster::build(&input, crate::namerena::eval_name::DEFAULT_EVAL_RQ).unwrap();
        assert_eq!(roster.groups, vec![vec![0, 1], vec![2]]);
        assert_eq!(roster.players.iter().map(|player| player.id).collect::<Vec<_>>(), vec![0, 1, 2]);
        assert_eq!(roster.players[0].clan_name, "red");
        assert_eq!(roster.players[1].clan_name, "red");
        assert_eq!(roster.players[2].clan_name, "solo");
    }
}

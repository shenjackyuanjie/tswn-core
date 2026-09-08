/// 技能最终等级的构成方式；用于 overlay 导入和 clone 重建。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillBoost {
    Normal(u32),
    SlotBoost { base: u32, boost: u32 },
    LastBoost(u32),
}

impl SkillBoost {
    pub fn final_level(&self) -> u32 {
        match self {
            Self::Normal(level) => *level,
            Self::SlotBoost { base, boost } => base + boost,
            Self::LastBoost(base) => base * 2,
        }
    }

    pub fn base_level(&self) -> u32 {
        match self {
            Self::Normal(level) => *level,
            Self::SlotBoost { base, .. } | Self::LastBoost(base) => *base,
        }
    }

    pub fn decayed_base_from_level(&self, current_level: u32) -> u32 {
        match self {
            Self::Normal(_) => current_level,
            Self::SlotBoost { boost, .. } => current_level.saturating_sub(*boost).max(1),
            Self::LastBoost(_) => current_level / 2,
        }
    }

    pub fn final_level_from_decayed_base(&self, decayed_base: u32) -> u32 {
        match self {
            Self::Normal(_) => decayed_base,
            Self::SlotBoost { boost, .. } => decayed_base.saturating_add(*boost),
            Self::LastBoost(_) => decayed_base.saturating_mul(2),
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if let Ok(value) = raw.parse::<u32>() {
            return Some(Self::Normal(value));
        }
        if let Some((base, boost)) = raw.split_once('+') {
            return Some(Self::SlotBoost {
                base: base.trim().parse().ok()?,
                boost: boost.trim().parse().ok()?,
            });
        }
        if let Some((multiplier, base)) = raw.split_once('*')
            && multiplier.trim().parse::<u32>().ok()? == 2
        {
            return Some(Self::LastBoost(base.trim().parse().ok()?));
        }
        None
    }
}

pub const CLASSIFIED_SKILL_SLOT_COUNT: usize = 44;
pub const SUMMON_FIRE1_SKILL_KEY: usize = 40;
pub const SUMMON_FIRE2_SKILL_KEY: usize = 41;
pub const SUMMON_EXPLODE_SKILL_KEY: usize = 42;
pub const PHANTOM_POSSESS_SKILL_KEY: usize = 43;
pub const SUMMON_MINION_NORMAL_SKILL_KEY_BASE: usize = 80;
pub const SUMMON_SHARE_DAMAGE_SKILL_KEY: usize = 255;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinSkillRef {
    Normal(usize),
    SummonFire,
    SummonExplode,
    SummonShareDamage,
    Possess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassifiedSkillRef {
    Normal(usize),
    SummonFire1,
    SummonFire2,
    SummonExplode,
    PhantomPossess,
}

impl ClassifiedSkillRef {
    pub fn player_key(self) -> usize {
        match self {
            Self::Normal(id) => id,
            Self::SummonFire1 => SUMMON_FIRE1_SKILL_KEY,
            Self::SummonFire2 => SUMMON_FIRE2_SKILL_KEY,
            Self::SummonExplode => SUMMON_EXPLODE_SKILL_KEY,
            Self::PhantomPossess => PHANTOM_POSSESS_SKILL_KEY,
        }
    }

    pub fn summon_minion_key(self) -> usize {
        match self {
            Self::Normal(id) => SUMMON_MINION_NORMAL_SKILL_KEY_BASE + id,
            _ => self.player_key(),
        }
    }

    pub fn builtin(self) -> BuiltinSkillRef {
        match self {
            Self::Normal(id) => BuiltinSkillRef::Normal(id),
            Self::SummonFire1 | Self::SummonFire2 => BuiltinSkillRef::SummonFire,
            Self::SummonExplode => BuiltinSkillRef::SummonExplode,
            Self::PhantomPossess => BuiltinSkillRef::Possess,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillEntrySpec {
    pub key: usize,
    pub skill: BuiltinSkillRef,
    pub level: u32,
    pub boosted: bool,
    pub boost: Option<SkillBoost>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillLoadoutSpec {
    pub entries: Vec<SkillEntrySpec>,
    pub fixed_lanes: Vec<usize>,
    pub active_order: Vec<usize>,
    pub pre_action_order: Vec<usize>,
    pub post_damage_order: Vec<usize>,
    pub post_action_after_states: Vec<(u64, usize)>,
}

impl SkillLoadoutSpec {
    pub(crate) fn standard(
        levels: &[u32; 40],
        boosted: &[bool; 40],
        boosts: &[Option<SkillBoost>; 40],
        action_order: &[u32; 40],
    ) -> Self {
        let entries = (0..35)
            .map(|key| SkillEntrySpec {
                key,
                skill: BuiltinSkillRef::Normal(key),
                level: levels[key],
                boosted: boosted[key],
                boost: boosts[key].clone(),
            })
            .collect();
        let active_order = action_order.iter().map(|key| *key as usize).filter(|key| *key < 35).collect();
        let positive = |key: usize| levels[key] > 0;
        Self {
            entries,
            fixed_lanes: (0..35).collect(),
            active_order,
            pre_action_order: [29, 34].into_iter().filter(|key| positive(*key)).collect(),
            post_damage_order: [30, 33, 34, 21].into_iter().filter(|key| positive(*key)).collect(),
            post_action_after_states: Vec::new(),
        }
    }

    pub(crate) fn player_overlay(skills: &[(String, SkillBoost)]) -> Self {
        let classified = skills.iter().any(|(name, _)| is_player_classified_skill_name(name));
        let mut entries = if classified {
            classified_player_entries()
        } else {
            normal_player_entries()
        };
        let mut active_order = Vec::with_capacity(entries.len());
        for (name, boost) in skills {
            let key = if classified {
                player_classified_skill_ref_from_name(name).map(ClassifiedSkillRef::player_key)
            } else {
                skill_name_to_id(name)
            };
            let Some(key) = key else { continue };
            if active_order.contains(&key) {
                continue;
            }
            active_order.push(key);
            if let Some(entry) = entries.iter_mut().find(|entry| entry.key == key) {
                apply_boost(entry, boost);
            }
        }
        for entry in &entries {
            if !active_order.contains(&entry.key) {
                active_order.push(entry.key);
            }
        }
        let fixed_lanes = entries.iter().map(|entry| entry.key).collect();
        Self::from_entries(entries, fixed_lanes, active_order)
    }

    pub(crate) fn generic_minion_overlay(skills: &[(String, SkillBoost)]) -> Self {
        if skills.iter().any(|(name, _)| parse_prefixed_classified_skill_name(name).is_some()) {
            let mut entries = classified_player_entries();
            let mut active_order = Vec::new();
            for (name, boost) in skills {
                let Some(skill) = player_classified_skill_ref_from_name(name) else {
                    continue;
                };
                let key = skill.player_key();
                if active_order.contains(&key) {
                    continue;
                }
                active_order.push(key);
                apply_boost(entries.iter_mut().find(|entry| entry.key == key).unwrap(), boost);
            }
            let fixed_lanes = entries.iter().map(|entry| entry.key).collect();
            return Self::from_entries(entries, fixed_lanes, active_order);
        }

        let mut entries = Vec::new();
        for (name, boost) in skills {
            let skill = match normalize_skill_name(name).as_str() {
                "possess" | "possession" | "附体" => Some(BuiltinSkillRef::Possess),
                "explode" | "selfdestruct" | "self_destruct" | "summonexplode" | "自爆" => Some(BuiltinSkillRef::SummonExplode),
                _ => skill_name_to_id(name).map(BuiltinSkillRef::Normal),
            };
            let Some(skill) = skill else { continue };
            let key = entries.len();
            let mut entry = empty_entry(key, skill);
            apply_boost(&mut entry, boost);
            entries.push(entry);
        }
        let fixed_lanes = (0..entries.len()).collect::<Vec<_>>();
        Self::from_entries(entries, fixed_lanes.clone(), fixed_lanes)
    }

    pub(crate) fn summon_overlay(skills: &[(String, SkillBoost)], share_damage: bool) -> Self {
        let classified = skills.iter().any(|(name, _)| parse_prefixed_classified_skill_name(name).is_some());
        let (mut entries, fixed_lanes) = if classified {
            let entries = classified_summon_entries();
            let fixed = entries.iter().map(|entry| entry.key).collect();
            (entries, fixed)
        } else {
            (
                vec![
                    empty_entry(0, BuiltinSkillRef::SummonFire),
                    empty_entry(1, BuiltinSkillRef::SummonFire),
                    empty_entry(2, BuiltinSkillRef::SummonExplode),
                ],
                vec![0, 1, 2],
            )
        };
        let mut active_order = Vec::new();
        for (name, boost) in skills {
            let key = if classified {
                parse_prefixed_classified_skill_name(name)
                    .or_else(|| summon_slot_skill_ref_from_name(name))
                    .map(ClassifiedSkillRef::summon_minion_key)
            } else {
                match name.trim().to_ascii_lowercase().as_str() {
                    "sklfire1" => Some(0),
                    "sklfire2" => Some(1),
                    "sklexplode" => Some(2),
                    _ => None,
                }
            };
            let Some(key) = key else { continue };
            if active_order.contains(&key) {
                continue;
            }
            active_order.push(key);
            if let Some(entry) = entries.iter_mut().find(|entry| entry.key == key) {
                apply_boost(entry, boost);
            }
        }
        entries.push(SkillEntrySpec {
            key: SUMMON_SHARE_DAMAGE_SKILL_KEY,
            skill: BuiltinSkillRef::SummonShareDamage,
            level: u32::from(share_damage),
            boosted: false,
            boost: None,
        });
        Self::from_entries(entries, fixed_lanes, active_order)
    }

    pub(crate) fn shadow_default(base_level: u32) -> Self {
        let boost = (base_level > 0).then_some(SkillBoost::LastBoost(base_level));
        Self::from_entries(
            vec![SkillEntrySpec {
                key: 0,
                skill: BuiltinSkillRef::Possess,
                level: base_level.saturating_mul(if base_level > 0 { 2 } else { 1 }),
                boosted: base_level > 0,
                boost,
            }],
            vec![0],
            vec![0],
        )
    }

    pub(crate) fn summon_default(slot_levels: [u32; 3], action_order: [usize; 3]) -> Self {
        let mut entries = vec![
            empty_entry(0, BuiltinSkillRef::SummonFire),
            empty_entry(1, BuiltinSkillRef::SummonFire),
            empty_entry(2, BuiltinSkillRef::SummonExplode),
        ];
        for (slot, key) in action_order.iter().copied().enumerate() {
            entries[key].level = slot_levels[slot];
        }
        for &key in action_order.iter().rev() {
            if entries[key].level == 0 {
                continue;
            }
            let base = entries[key].level;
            entries[key].level *= 2;
            entries[key].boosted = true;
            entries[key].boost = Some(SkillBoost::LastBoost(base));
            break;
        }
        entries.push(SkillEntrySpec {
            key: SUMMON_SHARE_DAMAGE_SKILL_KEY,
            skill: BuiltinSkillRef::SummonShareDamage,
            level: 1,
            boosted: false,
            boost: None,
        });
        Self::from_entries(entries, vec![0, 1, 2], action_order.into_iter().collect())
    }

    fn from_entries(entries: Vec<SkillEntrySpec>, fixed_lanes: Vec<usize>, active_order: Vec<usize>) -> Self {
        let keys_for_normal = |skill_key: usize| {
            entries
                .iter()
                .filter(move |entry| entry.skill == BuiltinSkillRef::Normal(skill_key) && entry.level > 0)
                .map(|entry| entry.key)
        };
        let mut post_damage_order = [30, 33, 34, 21].into_iter().flat_map(keys_for_normal).collect::<Vec<_>>();
        if entries
            .iter()
            .any(|entry| entry.skill == BuiltinSkillRef::SummonShareDamage && entry.level > 0)
        {
            post_damage_order.push(SUMMON_SHARE_DAMAGE_SKILL_KEY);
        }
        let pre_action_order = entries
            .iter()
            .filter(|entry| entry.level > 0 && matches!(entry.skill, BuiltinSkillRef::Normal(29 | 34)))
            .map(|entry| entry.key)
            .collect();
        Self {
            entries,
            fixed_lanes,
            active_order,
            pre_action_order,
            post_damage_order,
            post_action_after_states: Vec::new(),
        }
    }
}

fn empty_entry(key: usize, skill: BuiltinSkillRef) -> SkillEntrySpec {
    SkillEntrySpec {
        key,
        skill,
        level: 0,
        boosted: false,
        boost: None,
    }
}

fn apply_boost(entry: &mut SkillEntrySpec, boost: &SkillBoost) {
    entry.level = boost.final_level();
    entry.boosted = !matches!(boost, SkillBoost::Normal(_));
    entry.boost = entry.boosted.then(|| boost.clone());
}

fn normal_player_entries() -> Vec<SkillEntrySpec> { (0..40).map(|key| empty_entry(key, BuiltinSkillRef::Normal(key))).collect() }

fn classified_player_entries() -> Vec<SkillEntrySpec> {
    normal_player_entries()
        .into_iter()
        .chain([
            empty_entry(SUMMON_FIRE1_SKILL_KEY, BuiltinSkillRef::SummonFire),
            empty_entry(SUMMON_FIRE2_SKILL_KEY, BuiltinSkillRef::SummonFire),
            empty_entry(SUMMON_EXPLODE_SKILL_KEY, BuiltinSkillRef::SummonExplode),
            empty_entry(PHANTOM_POSSESS_SKILL_KEY, BuiltinSkillRef::Possess),
        ])
        .collect()
}

fn classified_summon_entries() -> Vec<SkillEntrySpec> {
    (0..40)
        .map(|id| empty_entry(SUMMON_MINION_NORMAL_SKILL_KEY_BASE + id, BuiltinSkillRef::Normal(id)))
        .chain([
            empty_entry(SUMMON_FIRE1_SKILL_KEY, BuiltinSkillRef::SummonFire),
            empty_entry(SUMMON_FIRE2_SKILL_KEY, BuiltinSkillRef::SummonFire),
            empty_entry(SUMMON_EXPLODE_SKILL_KEY, BuiltinSkillRef::SummonExplode),
            empty_entry(PHANTOM_POSSESS_SKILL_KEY, BuiltinSkillRef::Possess),
        ])
        .collect()
}

pub fn skill_name_to_id(name: &str) -> Option<usize> {
    let normalized = normalize_skill_name(name);
    match normalized.as_str() {
        "fire" => Some(0),
        "ice" => Some(1),
        "thunder" => Some(2),
        "quake" => Some(3),
        "absorb" => Some(4),
        "poison" => Some(5),
        "rapid" => Some(6),
        "critical" => Some(7),
        "half" => Some(8),
        "exchange" => Some(9),
        "berserk" => Some(10),
        "charm" => Some(11),
        "haste" => Some(12),
        "slow" => Some(13),
        "curse" => Some(14),
        "heal" => Some(15),
        "revive" => Some(16),
        "disperse" => Some(17),
        "iron" => Some(18),
        "charge" => Some(19),
        "accumulate" => Some(20),
        "assassinate" => Some(21),
        "summon" => Some(22),
        "clone" => Some(23),
        "shadow" => Some(24),
        "defend" => Some(25),
        "protect" => Some(26),
        "reflect" => Some(27),
        "reraise" => Some(28),
        "shield" => Some(29),
        "counter" => Some(30),
        "merge" => Some(31),
        "zombie" => Some(32),
        "upgrade" => Some(33),
        "hide" => Some(34),
        "none" => Some(35),
        _ => normalized.parse().ok().filter(|id| *id < 40),
    }
}

pub fn skill_name_for_export(skill_id: usize) -> String {
    let name = match skill_id {
        0 => "fire",
        1 => "ice",
        2 => "thunder",
        3 => "quake",
        4 => "absorb",
        5 => "poison",
        6 => "rapid",
        7 => "critical",
        8 => "half",
        9 => "exchange",
        10 => "berserk",
        11 => "charm",
        12 => "haste",
        13 => "slow",
        14 => "curse",
        15 => "heal",
        16 => "revive",
        17 => "disperse",
        18 => "iron",
        19 => "charge",
        20 => "accumulate",
        21 => "assassinate",
        22 => "summon",
        23 => "clone",
        24 => "shadow",
        25 => "defend",
        26 => "protect",
        27 => "reflect",
        28 => "reraise",
        29 => "shield",
        30 => "counter",
        31 => "merge",
        32 => "zombie",
        33 => "upgrade",
        34 => "hide",
        35 => "none",
        _ => return format!("skill{skill_id}"),
    };
    format!("skl{name}")
}

pub fn classified_player_skill_name_for_export(skill_key: usize) -> Option<String> {
    match skill_key {
        SUMMON_FIRE1_SKILL_KEY => Some("summon:sklfire1".to_owned()),
        SUMMON_FIRE2_SKILL_KEY => Some("summon:sklfire2".to_owned()),
        SUMMON_EXPLODE_SKILL_KEY => Some("summon:sklexplode".to_owned()),
        PHANTOM_POSSESS_SKILL_KEY => Some("phantom:sklpossess".to_owned()),
        _ => None,
    }
}

pub fn classified_summon_minion_skill_name_for_export(skill_key: usize) -> Option<String> {
    if (SUMMON_MINION_NORMAL_SKILL_KEY_BASE..SUMMON_MINION_NORMAL_SKILL_KEY_BASE + 40).contains(&skill_key) {
        return Some(format!(
            "normal:{}",
            skill_name_for_export(skill_key - SUMMON_MINION_NORMAL_SKILL_KEY_BASE)
        ));
    }
    classified_player_skill_name_for_export(skill_key)
}

pub fn parse_prefixed_classified_skill_name(name: &str) -> Option<ClassifiedSkillRef> {
    let (prefix, name) = name.trim().split_once(':')?;
    match prefix.trim().to_ascii_lowercase().as_str() {
        "normal" | "player" | "plr" | "玩家" => skill_name_to_id(name).map(ClassifiedSkillRef::Normal),
        "summon" | "familiar" | "使魔" => summon_slot_skill_ref_from_name(name),
        "phantom" | "shadow" | "幻影" => phantom_skill_ref_from_name(name),
        _ => None,
    }
}

pub fn player_classified_skill_ref_from_name(name: &str) -> Option<ClassifiedSkillRef> {
    parse_prefixed_classified_skill_name(name)
        .or_else(|| summon_slot_skill_ref_from_name(name))
        .or_else(|| phantom_skill_ref_from_name(name))
        .or_else(|| skill_name_to_id(name).map(ClassifiedSkillRef::Normal))
}

fn is_player_classified_skill_name(name: &str) -> bool {
    parse_prefixed_classified_skill_name(name).is_some()
        || summon_slot_skill_ref_from_name(name).is_some()
        || phantom_skill_ref_from_name(name).is_some()
}

pub fn summon_slot_skill_ref_from_name(name: &str) -> Option<ClassifiedSkillRef> {
    match name.trim().to_ascii_lowercase().as_str() {
        "sklfire1" => Some(ClassifiedSkillRef::SummonFire1),
        "sklfire2" => Some(ClassifiedSkillRef::SummonFire2),
        "sklexplode" => Some(ClassifiedSkillRef::SummonExplode),
        _ => None,
    }
}

pub fn phantom_skill_ref_from_name(name: &str) -> Option<ClassifiedSkillRef> {
    match normalize_skill_name(name).as_str() {
        "possess" | "possession" | "附体" => Some(ClassifiedSkillRef::PhantomPossess),
        _ => None,
    }
}

fn normalize_skill_name(name: &str) -> String {
    let lower = name.trim().to_ascii_lowercase();
    lower
        .strip_prefix("skl")
        .or_else(|| lower.strip_prefix("skill"))
        .unwrap_or(&lower)
        .to_owned()
}

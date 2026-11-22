use super::EffectiveSkillWeight;
use super::weapon::WeaponKind;

/// 技能最终等级的构成方式；用于 overlay 导入和 clone 重建。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillBoost {
    Normal(u32),
    SlotBoost { base: u32, boost: u32 },
    LastBoost(u32),
}

impl SkillBoost {
    /// 计算最终等级：座位加成取 `base + boost`，末尾翻倍取 `base × 2`。
    pub fn final_level(&self) -> u32 {
        match self {
            Self::Normal(level) => *level,
            Self::SlotBoost { base, boost } => base + boost,
            Self::LastBoost(base) => base * 2,
        }
    }

    /// 取不含任何加成的基级。
    pub fn base_level(&self) -> u32 {
        match self {
            Self::Normal(level) => *level,
            Self::SlotBoost { base, .. } | Self::LastBoost(base) => *base,
        }
    }

    /// 从当前最终等级反推衰减后的基级：座位加成减去 boost（至少 1），翻倍除以 2。
    pub fn decayed_base_from_level(&self, current_level: u32) -> u32 {
        match self {
            Self::Normal(_) => current_level,
            Self::SlotBoost { boost, .. } => current_level.saturating_sub(*boost).max(1),
            Self::LastBoost(_) => current_level / 2,
        }
    }

    /// 从衰减基级重建最终等级，与 [`decayed_base_from_level`](Self::decayed_base_from_level) 互逆。
    pub fn final_level_from_decayed_base(&self, decayed_base: u32) -> u32 {
        match self {
            Self::Normal(_) => decayed_base,
            Self::SlotBoost { boost, .. } => decayed_base.saturating_add(*boost),
            Self::LastBoost(_) => decayed_base.saturating_mul(2),
        }
    }

    /// 解析 overlay 文本：纯数字为 `Normal`，`base+boost` 为座位加成，`2*base` 为末尾翻倍。
    /// 无法识别时返回 `None`。
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

/// 玩家分类技能表的标准槽数。
pub const CLASSIFIED_SKILL_SLOT_COUNT: usize = 44;
/// 召唤火 1 在分类技能表中的键。
pub const SUMMON_FIRE1_SKILL_KEY: usize = 40;
/// 召唤火 2 在分类技能表中的键。
pub const SUMMON_FIRE2_SKILL_KEY: usize = 41;
/// 召唤爆炸在分类技能表中的键。
pub const SUMMON_EXPLODE_SKILL_KEY: usize = 42;
/// 幻影附体在分类技能表中的键。
pub const PHANTOM_POSSESS_SKILL_KEY: usize = 43;
/// 武器 S11 追加技能在分类技能表中的键。
pub const WEAPON_S11_SKILL_KEY: usize = 44;
/// 武器 DeathNote 追加技能在分类技能表中的键。
pub const WEAPON_DEATH_NOTE_SKILL_KEY: usize = 45;
/// 武器玄月反击追加技能在分类技能表中的键。
pub const WEAPON_XUAN_YUE_COUNTER_SKILL_KEY: usize = 46;
/// Rinick 身份分支的冰河世纪追加技能键。
pub const WEAPON_RINICK_ICE_AGE_SKILL_KEY: usize = 47;
/// Rinick 身份分支的尤里控制追加技能键。
pub const WEAPON_RINICK_YURI_CONTROL_SKILL_KEY: usize = 48;
/// 召唤物普通技能键的基址；实际键为 `基址 + 技能 id`。
pub const SUMMON_MINION_NORMAL_SKILL_KEY_BASE: usize = 80;
/// 召唤物分担伤害在分类技能表中的键。
pub const SUMMON_SHARE_DAMAGE_SKILL_KEY: usize = 255;

/// 内置技能引用：普通技能 id，以及召唤、附身、武器与身份分支等特殊技能。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinSkillRef {
    Normal(usize),
    SummonFire,
    SummonExplode,
    SummonShareDamage,
    Possess,
    WeaponS11,
    WeaponDeathNote,
    WeaponXuanYue,
    /// 0.5.5 `RinickModifier.b6()` 身份分支追加的 `SklAokijiIceAge`。
    RinickIceAge,
    /// 0.5.5 `RinickModifier.b6()` 身份分支追加的 `SklYuriControl`。
    RinickYuriControl,
}

/// 分类技能引用：玩家侧普通技能，以及召唤火/爆炸与幻影附体等特殊键。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassifiedSkillRef {
    Normal(usize),
    SummonFire1,
    SummonFire2,
    SummonExplode,
    PhantomPossess,
}

impl ClassifiedSkillRef {
    /// 映射为玩家分类技能表中的键。
    pub fn player_key(self) -> usize {
        match self {
            Self::Normal(id) => id,
            Self::SummonFire1 => SUMMON_FIRE1_SKILL_KEY,
            Self::SummonFire2 => SUMMON_FIRE2_SKILL_KEY,
            Self::SummonExplode => SUMMON_EXPLODE_SKILL_KEY,
            Self::PhantomPossess => PHANTOM_POSSESS_SKILL_KEY,
        }
    }

    /// 映射为召唤物分类技能表中的键：普通技能加基址，特殊键原样返回。
    pub fn summon_minion_key(self) -> usize {
        match self {
            Self::Normal(id) => SUMMON_MINION_NORMAL_SKILL_KEY_BASE + id,
            _ => self.player_key(),
        }
    }

    /// 映射为内置技能引用；两种召唤火合并为同一个内置召唤火。
    pub fn builtin(self) -> BuiltinSkillRef {
        match self {
            Self::Normal(id) => BuiltinSkillRef::Normal(id),
            Self::SummonFire1 | Self::SummonFire2 => BuiltinSkillRef::SummonFire,
            Self::SummonExplode => BuiltinSkillRef::SummonExplode,
            Self::PhantomPossess => BuiltinSkillRef::Possess,
        }
    }
}

/// 技能表中的一项：槽位键、内置引用、等级与加成构成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillEntrySpec {
    pub key: usize,
    pub skill: BuiltinSkillRef,
    pub level: u32,
    pub boosted: bool,
    pub boost: Option<SkillBoost>,
}

/// 完整技能装载：全部条目、固定 lanes、各阶段行动顺序与权重配置。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillLoadoutSpec {
    pub entries: Vec<SkillEntrySpec>,
    pub fixed_lanes: Vec<usize>,
    pub active_order: Vec<usize>,
    pub pre_action_order: Vec<usize>,
    pub post_damage_order: Vec<usize>,
    pub post_action_after_states: Vec<(u64, usize)>,
    pub effective_skill_weights: Option<Vec<(usize, EffectiveSkillWeight)>>,
    pub effective_skill_redistribution: Option<EffectiveSkillWeight>,
    pub effective_skill_adjustment: Option<Vec<(usize, EffectiveSkillWeight)>>,
}

/// effective_skills 调整项的数量上限。
pub(crate) const MAX_EFFECTIVE_SKILL_ADJUSTMENTS: usize = 10;

/// 条件受战况影响较大的主动技，以及需要保留发动位置的分身。
const DYNAMIC_EFFECTIVE_SKILLS: [usize; MAX_EFFECTIVE_SKILL_ADJUSTMENTS] = [4, 8, 9, 15, 16, 18, 19, 21, 23, 24];

/// 传统选技中，门槛通过比例与命中后成功选目标比例的乘积，再对十项评分环境取均值。
/// 双参数技能用运行时判定，不读取这里的系数。
const EFFECTIVE_SKILL_CONSUMPTION: [f64; 25] = [
    1.0,
    1.0,
    1.0,
    1.0,
    0.690622644804,
    1.0,
    1.0,
    1.0,
    0.740312795133,
    0.501942736741,
    0.978465351834,
    0.995293017446,
    0.913580299714,
    0.880615921261,
    0.879706399232,
    0.634811449246,
    0.159272254125,
    1.0,
    0.683077370271,
    0.717064745689,
    0.582567015189,
    0.799295810602,
    0.733380167844,
    1.0,
    0.905984506680,
];

impl SkillLoadoutSpec {
    /// 用等级 / 加成 / 行动序数组构造标准技能装载。
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
            effective_skill_weights: None,
            effective_skill_redistribution: None,
            effective_skill_adjustment: None,
        }
    }

    /// 用 overlay 的技能与 effective 列表构造玩家技能装载；含分类技能时走分类条目。
    pub(crate) fn player_overlay(
        skills: &[(String, SkillBoost)],
        effective_skills: Option<&[(String, EffectiveSkillWeight)]>,
    ) -> Self {
        let classified = skills.iter().any(|(name, _)| is_player_classified_skill_name(name))
            || effective_skills
                .into_iter()
                .flatten()
                .any(|(name, _)| is_player_classified_skill_name(name));
        let mut entries = if classified {
            classified_player_entries()
        } else {
            normal_player_entries()
        };
        let mut active_order = Vec::with_capacity(entries.len());
        let key_for_name = |name: &str| {
            if classified {
                player_classified_skill_ref_from_name(name).map(ClassifiedSkillRef::player_key)
            } else {
                skill_name_to_id(name)
            }
        };
        for (name, boost) in skills {
            let Some(key) = key_for_name(name) else { continue };
            if active_order.contains(&key) {
                continue;
            }
            active_order.push(key);
            if let Some(entry) = entries.iter_mut().find(|entry| entry.key == key) {
                apply_boost(entry, boost);
            }
        }
        let effective_skill_weights = effective_skills.map(|weights| {
            let mut by_key = Vec::new();
            for (name, weight) in weights {
                let Some(key) = key_for_name(name) else { continue };
                if by_key.iter().any(|(existing, _)| *existing == key) {
                    continue;
                }
                if let Some(entry) = entries.iter_mut().find(|entry| entry.key == key) {
                    if !active_order.contains(&key) {
                        apply_boost(entry, &SkillBoost::Normal(weight.default_level()));
                    }
                    by_key.push((key, *weight));
                }
            }
            by_key.sort_unstable_by_key(|&(key, _)| key);
            by_key
        });
        for entry in &entries {
            if !active_order.contains(&entry.key) {
                active_order.push(entry.key);
            }
        }
        if effective_skill_weights.is_some() {
            // 被动钩子的同优先级排序也使用行动顺序，等效模式统一以固定槽位为准。
            active_order.sort_unstable();
        }
        let fixed_lanes = entries.iter().map(|entry| entry.key).collect();
        let mut spec = Self::from_entries(entries, fixed_lanes, active_order);
        spec.effective_skill_weights = effective_skill_weights;
        spec
    }

    /// 按 active_order 与等级自动分配 effective_skills 权重。
    pub(crate) fn enable_auto_effective_skills(&mut self) {
        let mut weights = Vec::new();
        let mut adjustments = Vec::new();
        let mut remaining = 1.0;
        for &key in &self.active_order {
            let Some(entry) = self.entries.iter().find(|entry| entry.key == key && entry.level > 0) else {
                continue;
            };
            let level = f64::from(entry.level);
            let weight = match entry.skill {
                BuiltinSkillRef::Normal(id) if DYNAMIC_EFFECTIVE_SKILLS.contains(&id) => {
                    adjustments.push((key, 1.0 - remaining));
                    level
                }
                BuiltinSkillRef::Normal(id @ 0..=24) => {
                    let weight = remaining * level.min(128.0);
                    remaining *= 1.0 - EFFECTIVE_SKILL_CONSUMPTION[id] * level.min(128.0) / 128.0;
                    weight
                }
                BuiltinSkillRef::Normal(25..=34) => level,
                _ => continue,
            };
            weights.push((key, EffectiveSkillWeight::new(weight).unwrap()));
        }
        weights.sort_unstable_by_key(|&(key, _)| key);
        let ordinary = 1.0 - remaining;
        let mut adjustments = adjustments
            .into_iter()
            .enumerate()
            .map(|(index, (key, used))| {
                let cut = if ordinary > 0.0 { used / ordinary } else { 0.0 };
                // 给相邻双参数技能留出百万分位，保留它们之间的先后。
                let cut = cut * (1.0 - MAX_EFFECTIVE_SKILL_ADJUSTMENTS as f64 * 0.000_001) + index as f64 * 0.000_001;
                (key, EffectiveSkillWeight::new(cut).unwrap())
            })
            .collect::<Vec<_>>();
        adjustments.sort_unstable_by_key(|&(key, _)| key);
        self.effective_skill_weights = Some(weights);
        self.effective_skill_adjustment = Some(adjustments);
        self.effective_skill_redistribution = EffectiveSkillWeight::new(1.0);
    }

    /// 用技能列表构造通用小兵 overlay。
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

    /// 用技能列表构造召唤小兵 overlay；share_damage 控制分担伤害技能。
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

    /// 构造默认影子（附身）overlay。
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

    /// 按槽位等级与行动序构造默认召唤 overlay。
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

    /// 从条目表构造技能装载：推导 active_order 与 post_damage 顺序。
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
            effective_skill_weights: None,
            effective_skill_redistribution: None,
            effective_skill_adjustment: None,
        }
    }

    /// 0.5.5 `bs()` 中武器 `b6()` 追加的技能表。
    ///
    /// 武器技能排在全部普通技能之后：剁手刀进入 k2 主动表，死亡笔记同时进入
    /// k1 与 k2，玄月奖杯只注册 k1 受击回调。`e=true` 在产物中不参与末位翻倍
    /// 与槽位强化，这里仅保留标记。属性修改器在身份分支还会追加
    /// `SklAokijiIceAge`（20 级）与 `SklYuriControl`（10 级，随后吃末位翻倍到 20）。
    pub(crate) fn append_weapon_skills(&mut self, kind: WeaponKind, rinick_identity: bool) {
        if kind == WeaponKind::RinickModifier && rinick_identity {
            self.entries.push(SkillEntrySpec {
                key: WEAPON_RINICK_ICE_AGE_SKILL_KEY,
                skill: BuiltinSkillRef::RinickIceAge,
                level: 20,
                boosted: false,
                boost: None,
            });
            self.entries.push(SkillEntrySpec {
                key: WEAPON_RINICK_YURI_CONTROL_SKILL_KEY,
                skill: BuiltinSkillRef::RinickYuriControl,
                level: 20,
                boosted: true,
                boost: Some(SkillBoost::LastBoost(10)),
            });
            self.active_order.push(WEAPON_RINICK_ICE_AGE_SKILL_KEY);
            self.active_order.push(WEAPON_RINICK_YURI_CONTROL_SKILL_KEY);
            return;
        }
        let (key, skill, level, boosted) = match kind {
            WeaponKind::S11 => (WEAPON_S11_SKILL_KEY, BuiltinSkillRef::WeaponS11, 31, true),
            WeaponKind::DeathNote => (WEAPON_DEATH_NOTE_SKILL_KEY, BuiltinSkillRef::WeaponDeathNote, 1, true),
            WeaponKind::XuanYue => (WEAPON_XUAN_YUE_COUNTER_SKILL_KEY, BuiltinSkillRef::WeaponXuanYue, 1, false),
            _ => return,
        };
        self.entries.push(SkillEntrySpec {
            key,
            skill,
            level,
            boosted,
            boost: None,
        });
        if matches!(kind, WeaponKind::S11 | WeaponKind::DeathNote) {
            self.active_order.push(key);
        }
        if matches!(kind, WeaponKind::DeathNote | WeaponKind::XuanYue) {
            self.post_damage_order.push(key);
        }
    }
}

/// 构造 0 级空技能条目。
fn empty_entry(key: usize, skill: BuiltinSkillRef) -> SkillEntrySpec {
    SkillEntrySpec {
        key,
        skill,
        level: 0,
        boosted: false,
        boost: None,
    }
}

/// 把加成应用到条目：设置最终等级、boosted 与 boost。
fn apply_boost(entry: &mut SkillEntrySpec, boost: &SkillBoost) {
    entry.level = boost.final_level();
    entry.boosted = !matches!(boost, SkillBoost::Normal(_));
    entry.boost = entry.boosted.then(|| boost.clone());
}

/// 普通玩家的 40 个空技能条目。
fn normal_player_entries() -> Vec<SkillEntrySpec> { (0..40).map(|key| empty_entry(key, BuiltinSkillRef::Normal(key))).collect() }

/// 分类玩家条目：普通 40 项加召唤 / 附身等固定键。
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

/// 召唤小兵条目：40 个映射键加召唤 / 附身固定键。
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

/// 技能名到数字 id：剥离 `skl` / `skill` 前缀后查表；
/// 无法命中时接受小于 40 的纯数字 id，否则返回 `None`。
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

/// 数字 id 到导出名：命名表内返回 `skl<名字>`，
/// 命名表之外返回 `skill<id>`。
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

/// 玩家分类键的导出名（如 `summon:sklfire1`）；非特殊键返回 `None`。
pub fn classified_player_skill_name_for_export(skill_key: usize) -> Option<String> {
    match skill_key {
        SUMMON_FIRE1_SKILL_KEY => Some("summon:sklfire1".to_owned()),
        SUMMON_FIRE2_SKILL_KEY => Some("summon:sklfire2".to_owned()),
        SUMMON_EXPLODE_SKILL_KEY => Some("summon:sklexplode".to_owned()),
        PHANTOM_POSSESS_SKILL_KEY => Some("phantom:sklpossess".to_owned()),
        _ => None,
    }
}

/// 召唤物分类键的导出名：普通段转 `normal:<导出名>`，
/// 其余键转玩家分类导出名。
pub fn classified_summon_minion_skill_name_for_export(skill_key: usize) -> Option<String> {
    if (SUMMON_MINION_NORMAL_SKILL_KEY_BASE..SUMMON_MINION_NORMAL_SKILL_KEY_BASE + 40).contains(&skill_key) {
        return Some(format!(
            "normal:{}",
            skill_name_for_export(skill_key - SUMMON_MINION_NORMAL_SKILL_KEY_BASE)
        ));
    }
    classified_player_skill_name_for_export(skill_key)
}

/// 解析 `前缀:名字` 形式：`normal` / `summon` / `phantom` 三类前缀
/// （含中文与别名）分别映射到对应分类引用。
pub fn parse_prefixed_classified_skill_name(name: &str) -> Option<ClassifiedSkillRef> {
    let (prefix, name) = name.trim().split_once(':')?;
    match prefix.trim().to_ascii_lowercase().as_str() {
        "normal" | "player" | "plr" | "玩家" => skill_name_to_id(name).map(ClassifiedSkillRef::Normal),
        "summon" | "familiar" | "使魔" => summon_slot_skill_ref_from_name(name),
        "phantom" | "shadow" | "幻影" => phantom_skill_ref_from_name(name),
        _ => None,
    }
}

/// 按前缀、召唤、幻影、普通技能的优先级从名字推断分类引用。
pub fn player_classified_skill_ref_from_name(name: &str) -> Option<ClassifiedSkillRef> {
    parse_prefixed_classified_skill_name(name)
        .or_else(|| summon_slot_skill_ref_from_name(name))
        .or_else(|| phantom_skill_ref_from_name(name))
        .or_else(|| skill_name_to_id(name).map(ClassifiedSkillRef::Normal))
}

/// 判断名字是否为分类玩家技能（前缀 / 召唤 / 附身）。
fn is_player_classified_skill_name(name: &str) -> bool {
    parse_prefixed_classified_skill_name(name).is_some()
        || summon_slot_skill_ref_from_name(name).is_some()
        || phantom_skill_ref_from_name(name).is_some()
}

/// 召唤槽技能名（`sklfire1` / `sklfire2` / `sklexplode`，大小写不敏感）到分类引用。
pub fn summon_slot_skill_ref_from_name(name: &str) -> Option<ClassifiedSkillRef> {
    match name.trim().to_ascii_lowercase().as_str() {
        "sklfire1" => Some(ClassifiedSkillRef::SummonFire1),
        "sklfire2" => Some(ClassifiedSkillRef::SummonFire2),
        "sklexplode" => Some(ClassifiedSkillRef::SummonExplode),
        _ => None,
    }
}

/// 幻影技能名（`possess` / `possession` / `附体`，大小写不敏感）到分类引用。
pub fn phantom_skill_ref_from_name(name: &str) -> Option<ClassifiedSkillRef> {
    match normalize_skill_name(name).as_str() {
        "possess" | "possession" | "附体" => Some(ClassifiedSkillRef::PhantomPossess),
        _ => None,
    }
}

/// 去掉 skl / skill 前缀并小写化。
fn normalize_skill_name(name: &str) -> String {
    let lower = name.trim().to_ascii_lowercase();
    lower
        .strip_prefix("skl")
        .or_else(|| lower.strip_prefix("skill"))
        .unwrap_or(&lower)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::super::weapon::WeaponKind;
    use super::*;

    #[test]
    fn weapon_skills_append_to_the_legacy_lanes() {
        let mut loadout = SkillLoadoutSpec::default();
        loadout.append_weapon_skills(WeaponKind::S11, false);
        assert_eq!(loadout.entries.len(), 1);
        assert_eq!(loadout.entries[0].key, WEAPON_S11_SKILL_KEY);
        assert_eq!(loadout.entries[0].skill, BuiltinSkillRef::WeaponS11);
        assert_eq!(loadout.entries[0].level, 31);
        assert!(loadout.entries[0].boosted);
        assert_eq!(loadout.active_order, vec![WEAPON_S11_SKILL_KEY]);
        assert!(loadout.post_damage_order.is_empty());

        let mut loadout = SkillLoadoutSpec::default();
        loadout.append_weapon_skills(WeaponKind::DeathNote, false);
        assert_eq!(loadout.entries.len(), 1);
        assert_eq!(loadout.entries[0].key, WEAPON_DEATH_NOTE_SKILL_KEY);
        assert_eq!(loadout.entries[0].level, 1);
        assert!(loadout.entries[0].boosted);
        assert_eq!(loadout.active_order, vec![WEAPON_DEATH_NOTE_SKILL_KEY]);
        assert_eq!(loadout.post_damage_order, vec![WEAPON_DEATH_NOTE_SKILL_KEY]);

        let mut loadout = SkillLoadoutSpec::default();
        loadout.append_weapon_skills(WeaponKind::XuanYue, false);
        assert_eq!(loadout.entries.len(), 1);
        assert_eq!(loadout.entries[0].key, WEAPON_XUAN_YUE_COUNTER_SKILL_KEY);
        assert_eq!(loadout.entries[0].skill, BuiltinSkillRef::WeaponXuanYue);
        assert_eq!(loadout.entries[0].level, 1);
        assert!(!loadout.entries[0].boosted);
        assert!(loadout.active_order.is_empty());
        assert_eq!(loadout.post_damage_order, vec![WEAPON_XUAN_YUE_COUNTER_SKILL_KEY]);

        let mut loadout = SkillLoadoutSpec::default();
        loadout.append_weapon_skills(WeaponKind::Generic, false);
        assert!(loadout.entries.is_empty());
    }

    #[test]
    fn rinick_identity_appends_ice_age_and_boosted_yuri_control() {
        let mut loadout = SkillLoadoutSpec::default();
        loadout.append_weapon_skills(WeaponKind::RinickModifier, true);
        assert_eq!(loadout.entries.len(), 2);
        assert_eq!(loadout.entries[0].key, WEAPON_RINICK_ICE_AGE_SKILL_KEY);
        assert_eq!(loadout.entries[0].skill, BuiltinSkillRef::RinickIceAge);
        assert_eq!(loadout.entries[0].level, 20);
        assert!(!loadout.entries[0].boosted);
        assert_eq!(loadout.entries[1].key, WEAPON_RINICK_YURI_CONTROL_SKILL_KEY);
        assert_eq!(loadout.entries[1].skill, BuiltinSkillRef::RinickYuriControl);
        assert_eq!(loadout.entries[1].level, 20);
        assert!(loadout.entries[1].boosted);
        assert_eq!(loadout.entries[1].boost, Some(SkillBoost::LastBoost(10)));
        assert_eq!(
            loadout.active_order,
            vec![WEAPON_RINICK_ICE_AGE_SKILL_KEY, WEAPON_RINICK_YURI_CONTROL_SKILL_KEY]
        );
        assert!(loadout.post_damage_order.is_empty());

        // 非身份分支与普通武器一样不追加任何技能。
        let mut loadout = SkillLoadoutSpec::default();
        loadout.append_weapon_skills(WeaponKind::RinickModifier, false);
        assert!(loadout.entries.is_empty());
        assert!(loadout.active_order.is_empty());
    }
}

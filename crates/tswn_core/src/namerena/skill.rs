use crate::player::skill::SkillBoost;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinSkillRef {
    Normal(usize),
    SummonFire,
    SummonExplode,
    SummonShareDamage,
    Possess,
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
}

use std::{any::TypeId, sync::Arc};

use crate::{
    engine::{storage::Storage, update::RunUpdates},
    player::{
        OnDamageFunc, PlrPtr,
        skill::{Skill, SkillArgs, SkillTrait},
    },
    rc4::RC4,
};

use foldhash::{HashMap as FoldHashMap, HashMapExt, HashSet as FoldHashSet, HashSetExt};

#[derive(Debug, Clone)]
pub struct SkillStorage {
    pub store: FoldHashMap<TypeId, Skill>,
    pub skill: Vec<TypeId>,
    /// meta??
    pub meta: FoldHashSet<TypeId>,
    // 自己的状态 (usize: index)
    /// 更新状态时?
    pub update_states: Vec<TypeId>,
    /// step 之前
    pub pre_step: Vec<TypeId>,
    /// 动作之前
    pub pre_action: Vec<TypeId>,
    /// 动作之后
    pub post_action: Vec<TypeId>,
    /// 防御之前
    pub pre_defend: Vec<TypeId>,
    /// 防御之后
    pub post_defend: Vec<TypeId>,
    /// 伤害之后
    pub post_damage: Vec<TypeId>,
    /// 死亡之后
    pub post_death: Vec<TypeId>,
    /// 干掉目标之后
    pub post_kill: Vec<TypeId>,
    // 别的什么东西
    pub pending_clear_states: bool,
}

impl SkillStorage {
    pub fn new() -> Self {
        Self {
            store: FoldHashMap::new(),
            skill: Vec::new(),
            update_states: Vec::new(),
            meta: FoldHashSet::new(),
            pre_step: Vec::new(),
            pre_action: Vec::new(),
            post_action: Vec::new(),
            pre_defend: Vec::new(),
            post_defend: Vec::new(),
            post_damage: Vec::new(),
            post_death: Vec::new(),
            post_kill: Vec::new(),
            pending_clear_states: false,
        }
    }

    fn clear_proc(&mut self) {
        self.update_states.clear();
        self.meta.clear();
        self.pre_step.clear();
        self.pre_action.clear();
        self.post_action.clear();
        self.pre_defend.clear();
        self.post_defend.clear();
        self.post_damage.clear();
        self.post_death.clear();
        self.post_kill.clear();
    }

    pub fn update_proc(&mut self) {}

    /// 最后一个技能 boost
    pub fn boost_last(&mut self) {
        for skill in self.skill.iter().rev() {
            if self.store.get_mut(skill).expect("skill not found in store").boost_if_not() {
                break;
            }
        }
    }

    pub fn add_skill(&mut self, skill: Skill) {
        let id = TypeId::of::<Skill>();
        self.store.insert(id, skill.clone());
        self.skill.push(id);
    }

    pub fn skill_by_idx(&self, idx: usize) -> &Skill { self.store.get(&self.skill[idx]).expect("skill not found in store") }

    pub fn skill_by_idx_mut(&mut self, idx: usize) -> &mut Skill {
        self.store.get_mut(&self.skill[idx]).expect("skill not found in store")
    }

    pub fn skill_by_id(&self, id: TypeId) -> &Skill { self.store.get(&id).expect("skill not found in store") }

    pub fn skill_by_id_mut(&mut self, id: TypeId) -> &mut Skill { self.store.get_mut(&id).expect("skill not found in store") }

    // ==========
    // 以下是从 plr 里拆过来的部分, pre/post 之类的东西
    // ==========

    pub fn pre_step(&mut self, args: SkillArgs) {}

    pub fn pre_defend(&mut self, mut atp: f64, is_mag: bool, caster: PlrPtr, on_damage: OnDamageFunc, args: SkillArgs) -> f64 {
        for (idx, skill_type) in self.skill.iter().enumerate() {
            let skill = self.store.get_mut(skill_type).expect("skill not found in store");
            atp = skill.pre_defend(atp, is_mag, caster, &on_damage, (args.0, args.1, args.2, args.3));
        }

        atp
    }

    pub fn post_defend(&mut self, mut dmg: i32, caster: PlrPtr, on_damage: &OnDamageFunc, args: SkillArgs) -> i32 {
        for skill_type in self.post_defend.iter() {
            let skill = self.store.get_mut(skill_type).expect("skill not found in store");
            // dmg = skill.post_defend(caster, dmg, r, updates, s);
        }
        dmg
    }
}

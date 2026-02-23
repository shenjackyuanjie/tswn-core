use crate::{
    player::{
        OnDamageFunc, PlrId,
        skill::{ProcKind, Skill, SkillArgs},
    },
};

use foldhash::{HashMap as FoldHashMap, HashMapExt, HashSet as FoldHashSet, HashSetExt};

/// SkillStorage 内部使用的稳定技能键。
pub type SkillKey = usize;

#[derive(Debug, Clone)]
pub struct SkillStorage {
    pub store: FoldHashMap<SkillKey, Skill>,
    pub skill: Vec<SkillKey>,
    /// meta??
    pub meta: FoldHashSet<SkillKey>,
    // 自己的状态 (usize: index)
    /// 更新状态时?
    pub update_states: Vec<SkillKey>,
    /// step 之前
    pub pre_step: Vec<SkillKey>,
    /// 动作之前
    pub pre_action: Vec<SkillKey>,
    /// 动作之后
    pub post_action: Vec<SkillKey>,
    /// 防御之前
    pub pre_defend: Vec<SkillKey>,
    /// 防御之后
    pub post_defend: Vec<SkillKey>,
    /// 伤害之后
    pub post_damage: Vec<SkillKey>,
    /// 死亡之后
    pub post_death: Vec<SkillKey>,
    /// 干掉目标之后
    pub post_kill: Vec<SkillKey>,
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

    pub fn update_proc(&mut self) {
        self.clear_proc();
        let keys: Vec<SkillKey> = self.skill.clone();
        for key in keys {
            let skill = self.store.get(&key).expect("skill not found in store");
            if skill.level() == 0 {
                continue;
            }
            let kinds: Vec<ProcKind> = skill.proc_kinds().to_vec();
            for kind in kinds {
                match kind {
                    ProcKind::UpdateState => self.update_states.push(key),
                    ProcKind::PreStep => self.pre_step.push(key),
                    ProcKind::PreAction => self.pre_action.push(key),
                    ProcKind::PostAction => self.post_action.push(key),
                    ProcKind::PreDefend => self.pre_defend.push(key),
                    ProcKind::PostDefend => self.post_defend.push(key),
                    ProcKind::PostDamage => self.post_damage.push(key),
                    ProcKind::PostDeath => self.post_death.push(key),
                    ProcKind::PostKill => self.post_kill.push(key),
                }
            }
        }
    }

    /// 最后一个技能 boost
    pub fn boost_last(&mut self) {
        for skill in self.skill.iter().rev() {
            if self.store.get_mut(skill).expect("skill not found in store").boost_if_not() {
                break;
            }
        }
    }

    pub fn add_skill(&mut self, skill: Skill) {
        let id = self.skill.len();
        self.store.insert(id, skill);
        self.skill.push(id);
    }

    pub fn skill_by_idx(&self, idx: usize) -> &Skill { self.store.get(&self.skill[idx]).expect("skill not found in store") }

    pub fn skill_by_idx_mut(&mut self, idx: usize) -> &mut Skill {
        self.store.get_mut(&self.skill[idx]).expect("skill not found in store")
    }

    pub fn skill_by_id(&self, id: SkillKey) -> &Skill { self.store.get(&id).expect("skill not found in store") }

    pub fn skill_by_id_mut(&mut self, id: SkillKey) -> &mut Skill { self.store.get_mut(&id).expect("skill not found in store") }

    // ==========
    // 以下是从 plr 里拆过来的部分, pre/post 之类的东西
    // ==========

    pub fn update_state(&mut self, args: SkillArgs) {
        let keys: Vec<SkillKey> = self.update_states.clone();
        for skill_key in keys.iter() {
            let skill = self.store.get_mut(skill_key).expect("skill not found in store");
            skill.update_state((args.0, args.1, args.2, args.3));
        }
    }

    pub fn pre_step(&mut self, mut step: i32, args: SkillArgs) -> i32 {
        let keys: Vec<SkillKey> = self.pre_step.clone();
        for skill_key in keys.iter() {
            let skill = self.store.get_mut(skill_key).expect("skill not found in store");
            step = skill.pre_step(step, (args.0, args.1, args.2, args.3));
        }
        step
    }

    pub fn pre_action(&mut self, args: SkillArgs) {
        let keys: Vec<SkillKey> = self.pre_action.clone();
        for skill_key in keys.iter() {
            let skill = self.store.get_mut(skill_key).expect("skill not found in store");
            skill.pre_action((args.0, args.1, args.2, args.3));
        }
    }

    pub fn post_action(&mut self, args: SkillArgs) {
        let keys: Vec<SkillKey> = self.post_action.clone();
        for skill_key in keys.iter() {
            let skill = self.store.get_mut(skill_key).expect("skill not found in store");
            skill.post_action((args.0, args.1, args.2, args.3));
        }
    }

    pub fn pre_defend(&mut self, mut atp: f64, is_mag: bool, caster: PlrId, on_damage: OnDamageFunc, args: SkillArgs) -> f64 {
        let keys: Vec<SkillKey> = self.pre_defend.clone();
        for skill_key in keys.iter() {
            let skill = self.store.get_mut(skill_key).expect("skill not found in store");
            atp = skill.pre_defend(atp, is_mag, caster, &on_damage, (args.0, args.1, args.2, args.3));
            if atp == 0.0 {
                return 0.0;
            }
        }
        atp
    }

    pub fn post_defend(&mut self, mut dmg: i32, caster: PlrId, on_damage: &OnDamageFunc, args: SkillArgs) -> i32 {
        let keys: Vec<SkillKey> = self.post_defend.clone();
        for skill_key in keys.iter() {
            let skill = self.store.get_mut(skill_key).expect("skill not found in store");
            dmg = skill.post_defend(dmg, caster, on_damage, (args.0, args.1, args.2, args.3));
        }
        dmg
    }

    pub fn post_damage(&mut self, dmg: i32, caster: PlrId, args: SkillArgs) {
        let keys: Vec<SkillKey> = self.post_damage.clone();
        for skill_key in keys.iter() {
            let skill = self.store.get_mut(skill_key).expect("skill not found in store");
            skill.post_damage(dmg, caster, (args.0, args.1, args.2, args.3));
        }
    }

    pub fn die(&mut self, oldhp: i32, caster: PlrId, args: SkillArgs) {
        let keys: Vec<SkillKey> = self.post_death.clone();
        for skill_key in keys.iter() {
            let skill = self.store.get_mut(skill_key).expect("skill not found in store");
            if skill.die(oldhp, caster, (args.0, args.1, args.2, args.3)) {
                break;
            }
        }
    }

    pub fn kill(&mut self, target: PlrId, args: SkillArgs) {
        let keys: Vec<SkillKey> = self.post_kill.clone();
        for skill_key in keys.iter() {
            let skill = self.store.get_mut(skill_key).expect("skill not found in store");
            if skill.kill(target, (args.0, args.1, args.2, args.3)) {
                break;
            }
        }
    }
}

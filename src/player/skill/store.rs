use crate::engine::storage::PlrId;
use crate::player::OnDamageFunc;
use crate::player::skill::{Skill, SkillArgs};

#[derive(Debug, Clone)]
pub struct SkillStorage {
    /// 技能实例（每个玩家最多 ~20 个：用 Vec 直接存，cache 友好）
    pub skills: Vec<Skill>,

    /// 下列列表是“已注册到对应生命周期点”的技能索引（保序）
    pub pre_defend: Vec<usize>,
    pub post_defend: Vec<usize>,
    pub post_damage: Vec<usize>,

    pub pre_action: Vec<usize>,
    pub post_action: Vec<usize>,
}

impl SkillStorage {
    pub fn new() -> Self {
        Self {
            skills: Vec::new(),
            pre_defend: Vec::new(),
            post_defend: Vec::new(),
            post_damage: Vec::new(),
            pre_action: Vec::new(),
            post_action: Vec::new(),
        }
    }

    pub fn update_proc(&mut self) {
        // 目前先用最简单策略：所有技能都注册到所有钩子。
        // 等实现更多 skill/weapon 后，再细化为按需注册（对应 docs 的 Entry 列表）。
        let len = self.skills.len();
        self.pre_defend = (0..len).collect();
        self.post_defend = (0..len).collect();
        self.post_damage = (0..len).collect();
        self.pre_action = (0..len).collect();
        self.post_action = (0..len).collect();
    }

    /// 最后一个技能 boost
    pub fn boost_last(&mut self) {
        for skill in self.skills.iter_mut().rev() {
            if skill.boost_if_not() {
                break;
            }
        }
    }

    pub fn add_skill(&mut self, skill: Skill) {
        self.skills.push(skill);
    }

    pub fn len(&self) -> usize { self.skills.len() }

    pub fn skill_by_idx(&self, idx: usize) -> &Skill { &self.skills[idx] }
    pub fn skill_by_idx_mut(&mut self, idx: usize) -> &mut Skill { &mut self.skills[idx] }

    // ==========
    // 以下是从 plr 里拆过来的部分, pre/post 之类的东西
    // ==========

    pub fn pre_defend(
        &mut self,
        mut atp: f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        args: SkillArgs,
    ) -> f64 {
        let indices = self.pre_defend.clone();
        for idx in indices {
            let skill = self.skill_by_idx_mut(idx);
            atp = skill.pre_defend(atp, is_mag, caster, &on_damage, (args.0, args.1, args.2, args.3));
        }

        atp
    }

    pub fn post_defend(&mut self, mut dmg: i32, caster: PlrId, on_damage: &OnDamageFunc, args: SkillArgs) -> i32 {
        let indices = self.post_defend.clone();
        for idx in indices {
            let skill = self.skill_by_idx_mut(idx);
            dmg = skill.post_defend(dmg, caster, &on_damage, (args.0, args.1, args.2, args.3));
        }
        dmg
    }

    pub fn run_post_damage(&mut self, dmg: i32, caster: PlrId, args: SkillArgs) {
        let indices = self.post_damage.clone();
        for idx in indices {
            let skill = self.skill_by_idx_mut(idx);
            skill.post_damage(dmg, caster, (args.0, args.1, args.2, args.3));
        }
    }

    pub fn run_pre_action(&mut self, args: SkillArgs) {
        let indices = self.pre_action.clone();
        for idx in indices {
            let skill = self.skill_by_idx_mut(idx);
            skill.pre_action((args.0, args.1, args.2, args.3));
        }
    }

    pub fn run_post_action(&mut self, args: SkillArgs) {
        let indices = self.post_action.clone();
        for idx in indices {
            let skill = self.skill_by_idx_mut(idx);
            skill.post_action((args.0, args.1, args.2, args.3));
        }
    }
}

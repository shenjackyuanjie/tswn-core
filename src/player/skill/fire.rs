use crate::engine::storage::{PlrId, Storage};
use crate::engine::event::Event;
use crate::engine::update::RunUpdates;
use crate::player::skill::{SkillArgs, SkillExt, SkillTrait};
use crate::rc4::RC4;

#[derive(Debug, Clone)]
pub struct FireSkill {
    pub fire_mag: f64,
}

impl FireSkill {
    pub fn new() -> Self { Self { fire_mag: 0.0 } }

    /// OnDamage 函数
    pub fn on_fire(_caster: PlrId, target: PlrId, dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates, storage: &mut Storage) {
        // if dmg > 0 && 
    }
}

impl SkillExt for FireSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(FireSkill::new()) }
}

impl SkillTrait for FireSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn act(&mut self, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        let Some(&target_id) = targets.first() else {
            return;
        };

        // 事件化：不直接借用/修改 storage，交给 Runner 统一结算。
        // 目前先做最小行为：造成一个固定伤害（后续再把 atp/倍率/抗性接回去）。
        args.3.push(Event::DealDamage {
            caster: args.0,
            target: target_id,
            dmg: 10,
        });
    }
}

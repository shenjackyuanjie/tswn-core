//! 暴击主动技能实现。
//!
//! 攻击时有概率造成高倍率暴击伤害；维护 `CriticalState` 跟踪暴击状态。

use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct CriticalSkill;

impl CriticalSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for CriticalSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for CriticalSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn act(&mut self, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let owner = args.3.get_player(&args.0).expect("cannot get critical owner from storage");
        let atp0 = owner.get_at(false, args.1) * 1.149999976158142;
        let atp1 = owner.get_at(false, args.1) * 1.2000000476837158;
        let atp2 = owner.get_at(false, args.1) * 1.25;
        let atp = atp0.max(atp1).max(atp2);
        args.2.add(RunUpdate::new("[0]发动[会心一击]", args.0, target_id, 1));
        args.3
            .just_get_player_mut(target_id)
            .expect("cannot get critical target from storage")
            .attacked(atp, false, args.0, on_critical as OnDamageFunc, args.1, args.2, args.3);
    }
}

fn on_critical(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates, _storage: &Arc<Storage>) {}

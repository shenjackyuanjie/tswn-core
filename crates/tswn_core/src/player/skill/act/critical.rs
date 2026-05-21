use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId,
    skill::{Effect, InlineCtx, SkillArgs, SkillExt, SkillTrait},
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

    fn has_inline_act(&self) -> bool { true }

    fn act_inline(&mut self, _level: u32, targets: &[PlrId], _smart: bool, ctx: &mut InlineCtx) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let atp0 = ctx.owner.get_at(false, ctx.randomer) * 1.149999976158142;
        let atp1 = ctx.owner.get_at(false, ctx.randomer) * 1.2000000476837158;
        let atp2 = ctx.owner.get_at(false, ctx.randomer) * 1.25;
        let atp = atp0.max(atp1).max(atp2);
        ctx.updates.add(RunUpdate::new("[0]发动[会心一击]", ctx.ptr, target_id, 1));
        ctx.effects.push(Effect::Attack {
            target: target_id,
            atp,
            is_mag: false,
            on_damage: on_critical as OnDamageFunc,
        });
    }

    fn act(&mut self, targets: &[PlrId], _smart: bool, args: SkillArgs) {
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
        let core = {
            let target = args.3.just_get_player_mut(target_id).expect("cannot get critical target from storage");
            target.attacked_core(atp, false, args.0, on_critical as OnDamageFunc, args.1, args.2, args.3)
        };
        if core.hit {
            on_critical(args.0, core.target, core.dmg, args.1, args.2, args.3);
            let target = args.3.just_get_player_mut(core.target).expect("cannot get critical target from storage");
            target.finish_damage(core.dmg, core.old_hp, args.0, args.1, args.2, args.3);
        }
    }
}

fn on_critical(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates, _storage: &Arc<Storage>) {}

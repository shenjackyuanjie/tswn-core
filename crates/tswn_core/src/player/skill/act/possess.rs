use crate::engine::update::RunUpdate;
use crate::player::{
    Player, PlrId,
    skill::berserk::BerserkState,
    skill::{Effect, InlineCtx, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct PossessSkill;

impl PossessSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for PossessSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for PossessSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn has_inline_act(&self) -> bool { true }

    fn act_inline(&mut self, _level: u32, targets: Vec<PlrId>, _smart: bool, ctx: &mut InlineCtx) {
        let Some(&target_id) = targets.first() else {
            return;
        };
        ctx.updates.add(RunUpdate::new("[0]使用[附体]", ctx.ptr, target_id, 0));

        let dodged = if let Some(target) = ctx.storage.get_player(&target_id) {
            if target.check_immune("berserk", ctx.randomer) {
                true
            } else {
                target.alive()
                    && !target.get_status().frozed()
                    && Player::dodge(ctx.owner.status.magic, target.get_status().resistance, ctx.randomer)
            }
        } else {
            return;
        };
        if dodged {
            ctx.updates.add(RunUpdate::new("[0][回避]了攻击", target_id, ctx.ptr, 20));
            return;
        }

        {
            let target = ctx
                .storage
                .just_get_player_mut(target_id)
                .expect("cannot get possess target from storage");
            if let Some(state) = target.get_state_mut::<BerserkState>() {
                state.step += 4;
            } else {
                target.state.set(BerserkState { step: 4 });
            }
        }
        ctx.updates.add(RunUpdate::new("[1]进入[狂暴]状态", ctx.ptr, target_id, 0));

        let old_hp = ctx.owner.status.hp;
        ctx.owner.status.hp = 0;
        ctx.effects.push(Effect::OwnerDie { old_hp });
    }

    fn act_with_level(&mut self, _level: u32, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        let Some(&target_id) = targets.first() else {
            return;
        };
        args.2.add(RunUpdate::new("[0]使用[附体]", args.0, target_id, 0));

        let dodged = if let (Some(caster), Some(target)) = (args.3.get_player(&args.0), args.3.get_player(&target_id)) {
            if target.check_immune("berserk", args.1) {
                true
            } else {
                target.alive()
                    && !target.get_status().frozed()
                    && Player::dodge(caster.get_status().magic, target.get_status().resistance, args.1)
            }
        } else {
            return;
        };
        if dodged {
            args.2.add(RunUpdate::new("[0][回避]了攻击", target_id, args.0, 20));
            return;
        }

        {
            let target = args.3.just_get_player_mut(target_id).expect("cannot get possess target from storage");
            if let Some(state) = target.get_state_mut::<BerserkState>() {
                state.step += 4;
            } else {
                target.state.set(BerserkState { step: 4 });
            }
        }
        args.2.add(RunUpdate::new("[1]进入[狂暴]状态", args.0, target_id, 0));

        let old_hp = {
            let caster = args.3.just_get_player_mut(args.0).expect("cannot get possess caster from storage");
            let old_hp = caster.get_status().hp;
            caster.status.hp = 0;
            old_hp
        };
        args.3
            .just_get_player_mut(args.0)
            .expect("cannot get possess caster from storage")
            .on_die(old_hp, args.0, args.1, args.2, args.3);
    }
}

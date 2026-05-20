use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::{InlineCtx, PostActionPhase, ProcKind, SkillArgs, SkillExt, SkillTargetDomain, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct ChargeSkill {
    pub on_update_state: Option<()>,
    pub on_post_action: Option<()>,
    pub step: i32,
}

impl ChargeSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for ChargeSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ChargeSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn target_domain(&self) -> SkillTargetDomain { SkillTargetDomain::SelfOnly }

    fn select_target_count(&self, _smart: bool) -> usize { 1 }

    fn prob(&self, level: u32, smart: bool, args: SkillArgs) -> bool {
        // JS checks meta map (cleared by K()), not step. After interrupt, step > 0 but meta is gone.
        if self.on_update_state.is_some() {
            return false;
        }
        if smart {
            let owner = args.3.get_player(&args.0).expect("cannot get charge owner from storage");
            if owner.get_status().hp < 100 {
                return false;
            }
        }
        args.1.r127() < level
    }

    fn select_targets_with_level(&self, _level: u32, _candidates: &[PlrId], _smart: bool, args: SkillArgs) -> Vec<PlrId> {
        vec![args.0]
    }

    fn has_inline_act(&self) -> bool { true }

    fn act_inline(&mut self, _level: u32, _targets: Vec<PlrId>, _smart: bool, ctx: &mut InlineCtx) {
        self.step += 2;
        self.on_post_action = Some(());
        self.on_update_state = Some(());
        ctx.updates.add(RunUpdate::new("[0]开始[蓄力]", ctx.ptr, ctx.ptr, 1));
        ctx.mark_update_states();
        ctx.owner.set_magic_point(ctx.owner.magic_point() + 32);
    }

    fn act_with_level(&mut self, _level: u32, _targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        // JS: s.fy = s.fy + 2 — ADDS to step, does not reset.
        self.step += 2;
        self.on_post_action = Some(());
        self.on_update_state = Some(());
        args.2.add(RunUpdate::new("[0]开始[蓄力]", args.0, args.0, 1));
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get charge owner from storage");
        owner.update_states();
        owner.set_magic_point(owner.magic_point() + 32);
    }

    fn post_action(&mut self, args: SkillArgs) {
        // JS: fx is only in x2 when charge is active (added in v(), removed in K()).
        // In Rust, proc_kinds always includes PostAction, so guard with on_post_action flag.
        if self.on_post_action.is_none() {
            return;
        }
        self.step -= 1;
        if self.step <= 0 {
            self.on_post_action = None;
            self.on_update_state = None;
            args.3
                .just_get_player_mut(args.0)
                .expect("cannot get charge owner from storage")
                .update_states();
        }
    }

    fn has_inline_post_action(&self) -> bool { true }

    fn post_action_inline(&mut self, ctx: &mut InlineCtx) {
        if self.on_post_action.is_none() {
            return;
        }
        self.step -= 1;
        if self.step <= 0 {
            self.on_post_action = None;
            self.on_update_state = None;
            ctx.mark_update_states();
        }
    }

    fn post_action_phase(&self) -> PostActionPhase {
        // JS 把 Charge 的 PostActionImpl 以 ga4()=Infinity 追加到 x2 尾部，
        // 所以要晚于普通的 Poison/Protect/state post_action 收尾。
        PostActionPhase::Late
    }

    fn update_state(&mut self, args: SkillArgs) {
        if self.on_update_state.is_some() {
            args.3
                .just_get_player_mut(args.0)
                .expect("cannot get charge owner from storage")
                .mul_at_boost(3.0);
        }
    }

    fn update_state_inline(&mut self, _level: u32, status: &mut crate::player::PlayerStatus) {
        if self.on_update_state.is_some() {
            status.at_boost *= 3.0;
        }
    }

    fn clear_positive_runtime(&mut self, args: SkillArgs) -> Option<&'static str> {
        self.on_update_state.take()?;
        // JS K() does NOT reset fy (step). Leave step as-is so next activation accumulates.
        self.on_post_action = None;
        args.3
            .just_get_player_mut(args.0)
            .expect("cannot get charge owner from storage")
            .update_states();
        Some("[1]的[蓄力]被中止了")
    }

    fn clear_positive_runtime_inline(&mut self, ctx: &mut InlineCtx) -> Option<&'static str> {
        self.on_update_state.take()?;
        self.on_post_action = None;
        ctx.mark_update_states();
        Some("[1]的[蓄力]被中止了")
    }

    fn clear_positive_runtime_priority(&self) -> i32 { 200 }

    fn charge_runtime_active(&self) -> bool { self.on_update_state.is_some() }

    fn charge_step(&self) -> i32 { self.step }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostAction, ProcKind::UpdateState] }
}

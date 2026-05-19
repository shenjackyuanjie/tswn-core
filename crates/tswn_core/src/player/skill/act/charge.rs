use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::{PostActionPhase, ProcKind, SkillArgs, SkillExt, SkillTargetDomain, SkillTrait},
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
        // JS 检查的是 meta map（由 K() 清空），不是 step。
        // 因此技能被中断后即便 step > 0，只要 meta 消失也应视为未蓄力。
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

    fn act_with_level(&mut self, _level: u32, _targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        // JS: s.fy = s.fy + 2，会在现有 step 上累加，不会重置。
        self.step += 2;
        self.on_post_action = Some(());
        self.on_update_state = Some(());
        args.2.add(RunUpdate::new("[0]开始[蓄力]", args.0, args.0, 1));
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get charge owner from storage");
        owner.update_states();
        owner.set_magic_point(owner.magic_point() + 32);
    }

    fn post_action(&mut self, args: SkillArgs) {
        // JS: 只有蓄力生效时，fx 才会出现在 x2 中（在 v() 加入，在 K() 移除）。
        // Rust 的 proc_kinds 总是包含 PostAction，因此需要额外用 on_post_action 标记守卫。
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
        // JS 的 K() 不会重置 fy（step），这里也保持原值，让下次激活继续累加。
        self.on_post_action = None;
        args.3
            .just_get_player_mut(args.0)
            .expect("cannot get charge owner from storage")
            .update_states();
        Some("[1]的[蓄力]被中止了")
    }

    fn clear_positive_runtime_priority(&self) -> i32 { 200 }

    fn charge_runtime_active(&self) -> bool { self.on_update_state.is_some() }

    fn charge_step(&self) -> i32 { self.step }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostAction, ProcKind::UpdateState] }
}

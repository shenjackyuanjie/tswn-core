use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTargetDomain, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct IronSkill {
    pub sort_id: f64,
    pub on_post_defend: Option<()>,
    pub on_post_action: Option<()>,
    pub on_update_state: Option<()>,
    pub protect: i32,
    pub step: i32,
}

impl Default for IronSkill {
    fn default() -> Self {
        Self {
            sort_id: 4000.0,
            on_post_defend: None,
            on_post_action: None,
            on_update_state: None,
            protect: 0,
            step: 0,
        }
    }
}

impl IronSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for IronSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for IronSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn target_domain(&self) -> SkillTargetDomain { SkillTargetDomain::SelfOnly }

    fn select_target_count(&self, _smart: bool) -> usize { 1 }

    fn prob(&self, level: u32, _smart: bool, args: SkillArgs) -> bool {
        if self.step > 0 {
            return false;
        }
        args.1.r127() < level
    }

    fn select_targets_with_level(&self, _level: u32, _candidates: &[PlrId], _smart: bool, args: SkillArgs) -> Vec<PlrId> {
        vec![args.0]
    }

    fn act_with_level(&mut self, _level: u32, _targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        let owner_magic = args.3.get_player(&args.0).expect("cannot get iron owner from storage").get_status().magic;
        self.step = 3;
        self.protect = 110 + owner_magic;
        self.on_post_defend = Some(());
        self.on_post_action = Some(());
        self.on_update_state = Some(());
        args.2.add(RunUpdate::new("[0]发动[铁壁]", args.0, args.0, 60));
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get iron owner from storage");
        owner.set_move_point(owner.move_point() - 256);
        args.2.add(RunUpdate::new("[0]防御力大幅上升", args.0, args.0, 20));
    }

    fn post_defend(&mut self, mut dmg: i32, _caster: PlrId, _on_damage: &crate::player::OnDamageFunc, _args: SkillArgs) -> i32 {
        if self.step <= 0 || dmg <= 0 {
            return dmg.max(0);
        }
        if dmg <= self.protect {
            return 1;
        }
        dmg -= self.protect;
        self.protect = 0;
        self.step = 0;
        self.on_post_defend = None;
        self.on_post_action = None;
        self.on_update_state = None;
        dmg
    }

    fn post_action(&mut self, args: SkillArgs) {
        if self.step <= 0 {
            return;
        }
        self.step -= 1;
        if self.step == 0 {
            self.protect = 0;
            self.on_post_defend = None;
            self.on_post_action = None;
            self.on_update_state = None;
            let owner = args.3.just_get_player_mut(args.0).expect("cannot get iron owner from storage");
            owner.set_move_point(owner.move_point() - 128);
            args.2.add(RunUpdate::new("[0]从[铁壁]中解除", args.0, args.0, 20));
        }
    }

    fn update_state(&mut self, args: SkillArgs) {
        if self.step > 0 {
            args.3
                .just_get_player_mut(args.0)
                .expect("cannot get iron owner from storage")
                .mul_attract(1.12);
        }
    }

    fn update_state_inline(&mut self, _level: u32, status: &mut crate::player::PlayerStatus) {
        if self.step > 0 {
            status.attract *= 1.12;
        }
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostDefend, ProcKind::PostAction, ProcKind::UpdateState] }
}

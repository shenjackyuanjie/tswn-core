use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct HideSkill {
    pub on_pre_action: Option<()>,
    pub on_update_state: Option<()>,
}

impl Default for HideSkill {
    fn default() -> Self {
        Self {
            on_pre_action: None,
            on_update_state: None,
        }
    }
}

impl HideSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for HideSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for HideSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn post_damage_with_level(&mut self, level: u32, _dmg: i32, _caster: PlrId, args: SkillArgs) {
        if level == 0 || self.on_update_state.is_some() {
            return;
        }
        let owner_active = args.3.get_player(&args.0).map(|x| x.active()).unwrap_or(false);
        let alive_allies = if let Some(group) = args.3.group_containing(args.0) {
            group
                .iter()
                .filter(|id| args.3.get_player(id).map(|p| p.alive()).unwrap_or(false))
                .count()
        } else {
            let owner_clan = args
                .3
                .get_player(&args.0)
                .map(|p| p.clan_name())
                .unwrap_or_default();
            args.3
                .all_player_ids()
                .into_iter()
                .filter(|id| args.3.get_player(id).map(|p| p.alive() && p.clan_name() == owner_clan).unwrap_or(false))
                .count()
        };
        if owner_active && alive_allies > 1 && args.1.r63() < level {
            self.on_update_state = Some(());
            args.2.add(RunUpdate::new("[0]发动[隐匿]", args.0, args.0, 10));
        }
    }

    fn pre_action(&mut self, _args: SkillArgs) {
        if self.on_update_state.is_some() {
            self.on_update_state = None;
        }
    }

    fn update_state_with_level(&mut self, level: u32, args: SkillArgs) {
        if self.on_update_state.is_none() {
            return;
        }
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get hide owner from storage");
        owner.mul_attract(0.1);
        if level > 63 {
            let boost = (level - 63) as i32;
            owner.add_agility(boost);
            owner.add_defense(boost);
            owner.add_resistance(boost);
        }
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostDamage, ProcKind::PreAction, ProcKind::UpdateState] }
}

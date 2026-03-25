use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::act::charm::CharmState,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct HideSkill {
    pub on_pre_action: Option<()>,
    pub on_update_state: Option<()>,
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

    fn post_damage_with_level(&mut self, level: u32, dmg: i32, caster: PlrId, args: SkillArgs) {
        let debug_action = std::env::var("TSWN_DEBUG_ACTION").ok();
        let debug_this = debug_action
            .as_deref()
            .map(|name| args.3.get_player(&args.0).map(|p| p.id_name() == name).unwrap_or(false))
            .unwrap_or(false);
        if level == 0 || self.on_update_state.is_some() {
            if debug_this {
                let caster_name = args.3.get_player(&caster).map(|p| p.id_name()).unwrap_or_else(|| format!("#{}", caster));
                eprintln!(
                    "[hide_post_damage] owner={} skip level={} pending={} caster={} dmg={} rc4=({}, {})",
                    args.3.get_player(&args.0).map(|p| p.id_name()).unwrap_or_else(|| format!("#{}", args.0)),
                    level,
                    self.on_update_state.is_some(),
                    caster_name,
                    dmg,
                    args.1.i,
                    args.1.j,
                );
            }
            return;
        }
        let owner_active = args.3.get_player(&args.0).map(|x| x.active()).unwrap_or(false);
        let alive_allies = args
            .3
            .get_player(&args.0)
            .and_then(|owner| {
                owner
                    .get_state::<CharmState>()
                    .and_then(|charm| args.3.alive_group_at_team_of(charm.group_id).map(|group| group.len()))
                    .or_else(|| args.3.alive_group_at_team_of(args.0).map(|group| group.len()))
            })
            .unwrap_or_else(|| {
                let owner_clan = args.3.get_player(&args.0).map(|p| p.clan_name()).unwrap_or_default();
                args.3
                    .all_player_ids()
                    .into_iter()
                    .filter(|id| args.3.get_player(id).map(|p| p.alive() && p.clan_name() == owner_clan).unwrap_or(false))
                    .count()
            });
        if debug_this {
            let caster_name = args.3.get_player(&caster).map(|p| p.id_name()).unwrap_or_else(|| format!("#{}", caster));
            eprintln!(
                "[hide_post_damage] owner={} active={} alive_allies={} level={} caster={} dmg={} before_roll rc4=({}, {})",
                args.3.get_player(&args.0).map(|p| p.id_name()).unwrap_or_else(|| format!("#{}", args.0)),
                owner_active,
                alive_allies,
                level,
                caster_name,
                dmg,
                args.1.i,
                args.1.j,
            );
        }
        if owner_active && alive_allies > 1 && args.1.r63() < level {
            self.on_update_state = Some(());
            args.3
                .just_get_player_mut(args.0)
                .expect("cannot get hide owner from storage")
                .update_states();
            args.2.add(RunUpdate::new("[0]发动[隐匿]", args.0, args.0, 10));
            if debug_this {
                eprintln!(
                    "[hide_post_damage] owner={} triggered rc4=({}, {})",
                    args.3.get_player(&args.0).map(|p| p.id_name()).unwrap_or_else(|| format!("#{}", args.0)),
                    args.1.i,
                    args.1.j,
                );
            }
        } else if debug_this {
            eprintln!(
                "[hide_post_damage] owner={} not_triggered rc4=({}, {})",
                args.3.get_player(&args.0).map(|p| p.id_name()).unwrap_or_else(|| format!("#{}", args.0)),
                args.1.i,
                args.1.j,
            );
        }
    }

    fn pre_action(&mut self, args: SkillArgs) {
        if self.on_update_state.is_some() {
            self.on_update_state = None;
            args.3
                .just_get_player_mut(args.0)
                .expect("cannot get hide owner from storage")
                .update_states();
        }
    }

    fn pre_action_clear_forced(&mut self, _smart: bool, args: SkillArgs) -> bool {
        args.3
            .get_player(&args.0)
            .map(|owner| owner.has_state::<crate::player::skill::berserk::BerserkState>())
            .unwrap_or(false)
    }

    fn update_state_with_level(&mut self, level: u32, args: SkillArgs) {
        if self.on_update_state.is_none() {
            return;
        }
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get hide owner from storage");
        owner.mul_attract(0.10000000149011612);
        if level > 63 {
            let boost = (level - 63) as i32;
            owner.add_agility(boost);
            owner.add_defense(boost);
            owner.add_resistance(boost);
        }
    }

    fn update_state_inline(&mut self, level: u32, status: &mut crate::player::PlayerStatus) {
        if self.on_update_state.is_none() {
            return;
        }
        status.attract *= 0.10000000149011612;
        if level > 63 {
            let boost = (level - 63) as i32;
            status.agility += boost;
            status.defense += boost;
            status.resistance += boost;
        }
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostDamage, ProcKind::PreAction, ProcKind::UpdateState] }
}

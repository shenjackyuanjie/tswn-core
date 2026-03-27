use crate::engine::update::RunUpdate;
use crate::player::{
    Player, PlrId, StateTrait,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct CharmSkill;

impl CharmSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for CharmSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for CharmSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn valid_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> bool {
        if !smart {
            return true;
        }
        let Some(target_plr) = args.3.get_player(&target) else {
            return false;
        };
        if let Some(charm) = target_plr.get_state::<CharmState>() {
            charm.step <= 1
        } else {
            true
        }
    }

    fn score_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        let Some(target_plr) = args.3.get_player(&target) else {
            return f64::MIN;
        };
        let rate_hi_hp = |hp: i32| -> f64 {
            if hp < 20 {
                30.0
            } else if hp > 300 {
                300.0
            } else {
                hp as f64
            }
        };
        let mut score = if smart {
            let alive_group_count = args.3.alive_group_count();
            let target_alive_group_len = args.3.alive_group_containing(target).map(|group| group.len()).unwrap_or(0);
            let status = target_plr.get_status();
            if alive_group_count > 2 {
                rate_hi_hp(status.hp) * target_alive_group_len as f64 * status.attract
            } else {
                rate_hi_hp(status.hp) * status.attr_sum as f64 * status.attract
            }
        } else {
            args.1.rFFFF() as f64 + target_plr.get_status().attract
        };
        if target_plr.has_state::<CharmState>() || target_plr.has_state::<crate::player::skill::berserk::BerserkState>() {
            score /= 2.0;
        }
        score
    }

    fn act(&mut self, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        args.2.add(RunUpdate::new("[0]使用[魅惑]", args.0, target_id, 1));

        let (owner_magic, charge_active) = {
            let owner = args.3.get_player(&args.0).expect("cannot get charm owner from storage");
            (owner.get_status().magic, owner.get_status().at_boost >= 3.0)
        };
        // Dart compares owner.allyGroup (group object) vs charmState.grp (group object).
        // Rust keeps the charm source player id in group_id, but also needs the already-resolved
        // effective team so chained charm does not collapse back to the source player's original team.
        let caster_effective_team_idx = {
            let owner = args.3.get_player(&args.0).expect("cannot get charm owner from storage");
            if let Some(caster_charm) = owner.get_state::<CharmState>() {
                caster_charm.effective_team_idx.or_else(|| args.3.group_index_of(caster_charm.group_id))
            } else {
                args.3.group_index_of(args.0)
            }
        };
        let target = args.3.just_get_player_mut(target_id).expect("cannot get charm target from storage");
        if target.check_immune("charm", args.1)
            || (target.active()
                && Player::dodge(
                    owner_magic,
                    target.get_status().agility + target.get_status().resistance,
                    args.1,
                ))
        {
            args.2.add(RunUpdate::new("[0][回避]了攻击", target_id, args.0, 20));
            return;
        }

        if let Some(state) = target.get_state_mut::<CharmState>() {
            let existing_team_idx = state.effective_team_idx.or_else(|| args.3.group_index_of(state.group_id));
            if existing_team_idx == caster_effective_team_idx {
                state.step += 1;
            } else {
                state.group_id = args.0;
            }
            state.effective_team_idx = caster_effective_team_idx;
            if charge_active {
                state.step += 3;
            }
        } else {
            target.set_state(CharmState {
                group_id: args.0,
                effective_team_idx: caster_effective_team_idx,
                target: Some(target_id),
                on_post_action: None,
                step: if charge_active { 4 } else { 1 },
            });
        }
        args.2.add(RunUpdate::new("[1]被[魅惑]了", args.0, target_id, 120));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CharmState {
    pub group_id: usize,
    pub effective_team_idx: Option<usize>,
    pub target: Option<PlrId>,
    pub on_post_action: Option<()>,
    pub step: i32,
}

impl StateTrait for CharmState {
    fn meta_type(&self) -> i32 { -1 }

    fn post_action_priority(&self) -> i32 { 230 }

    fn on_post_action(
        &mut self,
        owner: PlrId,
        alive: bool,
        _randomer: &mut crate::rc4::RC4,
        updates: &mut crate::engine::update::RunUpdates,
        _storage: &std::sync::Arc<crate::engine::storage::Storage>,
    ) -> bool {
        self.step -= 1;
        if self.step > 0 {
            return false;
        }
        if alive {
            updates.emit(RunUpdate::new_newline);
            updates.emit(|| RunUpdate::new("[1]从[魅惑]中解除", owner, owner, 0));
        }
        true
    }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

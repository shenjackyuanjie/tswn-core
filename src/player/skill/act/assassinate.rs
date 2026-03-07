use std::cmp::Ordering;
use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId,
    skill::poison::PoisonState,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
    state_tag,
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct AssassinateSkill {
    pub on_pre_action: Option<()>,
    pub on_post_damage: Option<()>,
    pub target: Option<PlrId>,
}

impl AssassinateSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for AssassinateSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for AssassinateSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn uses_custom_target_selection(&self) -> bool { false }

    fn prob(&self, level: u32, smart: bool, args: SkillArgs) -> bool {
        if self.target.is_some() {
            return true;
        }
        if smart && args.3.get_player(&args.0).map(|p| p.has_state::<PoisonState>()).unwrap_or(false) {
            return false;
        }
        args.1.r127() < level
    }

    fn select_target_count(&self, smart: bool) -> usize { if smart { 3 } else { 2 } }

    fn valid_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> bool {
        if let Some(locked) = self.target {
            return locked == target;
        }
        if !smart {
            return true;
        }
        args.3.get_player(&target).map(|x| x.get_status().hp > 160).unwrap_or(false)
    }

    fn score_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        let Some(target_plr) = args.3.get_player(&target) else {
            return f64::MIN;
        };
        if !smart {
            return args.1.rFFFF() as f64 + target_plr.get_status().attract;
        }

        let rate_hi_hp = |hp: i32| -> f64 {
            if hp < 20 {
                30.0
            } else if hp > 300 {
                300.0
            } else {
                hp as f64
            }
        };
        let alive_group_count = args.3.alive_group_count();
        let target_alive_group_len = args.3.alive_group_containing(target).map(|group| group.len()).unwrap_or(0);
        let status = target_plr.get_status();
        if alive_group_count > 2 {
            rate_hi_hp(status.hp) * target_alive_group_len as f64 * status.attract
        } else {
            rate_hi_hp(status.hp) * status.attr_sum as f64 * status.attract
        }
    }

    fn select_targets_with_level(&self, level: u32, candidates: &[PlrId], smart: bool, args: SkillArgs) -> Vec<PlrId> {
        if let Some(target) = self.target {
            if args.3.get_player(&target).map(|x| x.alive()).unwrap_or(false) {
                return vec![target];
            }
            return Vec::new();
        }
        let select_count = self.select_target_count_with_level(level, smart);
        if select_count == 0 {
            return Vec::new();
        }
        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let Some(idx) = args.1.pick(candidates) else {
                return Vec::new();
            };
            let target = candidates[idx];
            if !self.valid_target_with_level(level, target, smart, (args.0, args.1, args.2, args.3)) {
                invalid += 1;
                continue;
            }
            if selected.contains(&target) {
                dup += 1;
                continue;
            }
            selected.push(target);
            if selected.len() >= select_count {
                break;
            }
        }
        if selected.is_empty() {
            return Vec::new();
        }
        let mut scored = selected
            .into_iter()
            .map(|target| {
                (
                    target,
                    self.score_target_with_level(level, target, smart, (args.0, args.1, args.2, args.3)),
                )
            })
            .collect::<Vec<(PlrId, f64)>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(Ordering::Equal));
        scored.into_iter().map(|x| x.0).collect()
    }

    fn act_with_level(&mut self, _level: u32, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if self.target.is_none() && targets.is_empty() {
            return;
        }
        if self.target.is_none() {
            let target_id = targets[0];
            self.target = Some(target_id);
            self.on_pre_action = Some(());
            let (current_move, owner_magic, charge_active) = args
                .3
                .get_player(&args.0)
                .map(|owner| (owner.move_point(), owner.get_status().magic, owner.get_status().at_boost >= 3.0))
                .expect("cannot get assassinate owner from storage");
            if !charge_active {
                self.on_post_damage = Some(());
            }
            args.3
                .just_get_player_mut(args.0)
                .expect("cannot get assassinate owner from storage")
                .set_move_point(current_move + owner_magic * 3 + if charge_active { 1600 } else { 0 });
            args.2.add(RunUpdate::new("[0][潜行]到[1]身后", args.0, target_id, 1));
            return;
        }

        let target_id = self.target.expect("assassinate target should exist");
        self.clear_pending();
        if !args.3.get_player(&target_id).map(|x| x.alive()).unwrap_or(false) {
            return;
        }
        args.2.add(RunUpdate::new("[0]发动[背刺]", args.0, target_id, 1));
        let owner = args.3.get_player(&args.0).expect("cannot get assassinate owner from storage");
        let atp = owner.get_at(true, args.1).max(owner.get_at(true, args.1)).max(owner.get_at(true, args.1)) * 4.0;
        let dodged = args
            .3
            .get_player(&target_id)
            .map(|target| target.check_immune(state_tag::<PoisonState>(), args.1))
            .unwrap_or(false);
        if dodged {
            args.2.add(RunUpdate::new("[0][回避]了攻击", target_id, args.0, 20));
            return;
        }
        args.3
            .just_get_player_mut(target_id)
            .expect("cannot get assassinate target from storage")
            .defned(atp, true, args.0, on_assassinate as OnDamageFunc, args.1, args.2, args.3);
    }

    fn pre_action(&mut self, _args: SkillArgs) {
        if self.target.is_some() {
            self.on_pre_action = Some(());
        }
    }

    fn pre_action_select(&mut self, _smart: bool, args: SkillArgs) -> bool {
        let Some(target) = self.target else {
            return false;
        };
        if args.3.get_player(&target).map(|x| x.alive()).unwrap_or(false) {
            true
        } else {
            self.clear_pending();
            false
        }
    }

    fn post_damage(&mut self, _dmg: i32, _caster: PlrId, args: SkillArgs) {
        if self.target.is_none() {
            return;
        }
        let target = self.target.expect("assassinate target should exist");
        args.2.add(RunUpdate::new_newline());
        args.2.add(RunUpdate::new("[0]的[潜行]被识破", args.0, target, 20));
        self.clear_pending();
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PreAction, ProcKind::PostDamage] }
}

impl AssassinateSkill {
    fn clear_pending(&mut self) {
        self.target = None;
        self.on_post_damage = None;
        self.on_pre_action = None;
    }
}

fn on_assassinate(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates, _storage: &Arc<Storage>) {}

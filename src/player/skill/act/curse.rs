use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId, StateTrait,
    skill::{SkillArgs, SkillExt, SkillTrait},
    state_tag,
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct CurseSkill;

impl CurseSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for CurseSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for CurseSkill {
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
        if target_plr.get_status().hp < 80 {
            return false;
        }
        if let Some(curse) = target_plr.get_state::<CurseState>()
            && curse.prob > 32
        {
            return false;
        }
        true
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
        let base = if smart {
            let alive_group_count = {
                let mut group_heads = Vec::new();
                for id in args.3.all_player_ids() {
                    let alive = args.3.get_player(&id).map(|plr| plr.alive()).unwrap_or(false);
                    if !alive {
                        continue;
                    }
                    let Some(group) = args.3.group_containing(id) else {
                        continue;
                    };
                    let Some(head) = group.first() else {
                        continue;
                    };
                    if !group_heads.contains(head) {
                        group_heads.push(*head);
                    }
                }
                group_heads.len()
            };
            let target_alive_group_len = args
                .3
                .group_containing(target)
                .map(|group| {
                    group
                        .iter()
                        .filter(|id| args.3.get_player(id).map(|plr| plr.alive()).unwrap_or(false))
                        .count()
                })
                .unwrap_or(0);
            let status = target_plr.get_status();
            if alive_group_count > 2 {
                rate_hi_hp(status.hp) * target_alive_group_len as f64 * status.attract
            } else {
                (1.0 / rate_hi_hp(status.hp)) * status.atk_sum as f64 * status.attract
            }
        } else {
            args.1.rFFFF() as f64 + target_plr.get_status().attract
        };
        if target_plr.get_state::<CurseState>().is_some() {
            base / 2.0
        } else {
            base
        }
    }

    fn act(&mut self, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let atp = args
            .3
            .get_player(&args.0)
            .expect("cannot get curse caster from storage")
            .get_at(true, args.1);
        args.2.add(RunUpdate::new("[0]使用[诅咒]", args.0, target_id, 1));
        let dmg = args
            .3
            .just_get_player_mut(target_id)
            .expect("cannot get curse target from storage")
            .attacked(atp, true, args.0, on_curse as OnDamageFunc, args.1, args.2, args.3);
        if dmg <= 0 {
            return;
        }
        let target = args.3.just_get_player_mut(target_id).expect("cannot get curse target from storage");
        if !target.alive() || target.check_immune(state_tag::<CurseState>(), args.1) {
            return;
        }
        if let Some(state) = target.get_state_mut::<CurseState>() {
            state.prob += 10;
            state.multiply += 1;
            state.owner = Some(args.0);
        } else {
            target.set_state(CurseState {
                owner: Some(args.0),
                target: Some(target_id),
                on_update_state: None,
                prob: 42,
                multiply: 2,
            });
        }
        args.2.add(RunUpdate::new("[1]被[诅咒]了", args.0, target_id, 60));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurseState {
    pub owner: Option<PlrId>,
    pub target: Option<PlrId>,
    pub on_update_state: Option<()>,
    pub prob: i32,
    pub multiply: i32,
}

impl Default for CurseState {
    fn default() -> Self {
        Self {
            owner: None,
            target: None,
            on_update_state: None,
            prob: 42,
            multiply: 2,
        }
    }
}

impl StateTrait for CurseState {
    fn meta_type(&self) -> i32 { -1 }

    fn update_state_priority(&self) -> i32 { 120 }

    fn apply_update_state(&self, status: &mut crate::player::PlayerStatus) { status.atk_sum *= 4; }

    fn post_defend_priority(&self) -> i32 { 110 }

    fn on_post_defend(&mut self, owner: PlrId, dmg: &mut i32, caster: PlrId, randomer: &mut RC4, updates: &mut RunUpdates) {
        if *dmg <= 0 {
            return;
        }
        if randomer.r63() < self.prob as u32 {
            updates.add(RunUpdate::new("[诅咒]使伤害加倍", caster, owner, 0));
            *dmg *= self.multiply;
        }
    }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

fn on_curse(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates) {}

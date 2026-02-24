use crate::engine::update::RunUpdate;
use crate::player::{
    Player, PlrId, StateTrait,
    skill::{SkillArgs, SkillExt, SkillTrait},
    state_tag,
};

#[derive(Debug, Clone, Default)]
pub struct SlowSkill;

impl SlowSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for SlowSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for SlowSkill {
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
        if let Some(slow) = target_plr.get_state::<SlowState>()
            && slow.step > 1
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
        let mut base = if smart {
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
                rate_hi_hp(status.hp) * status.attr_sum as f64 * status.attract
            }
        } else {
            args.1.rFFFF() as f64 + target_plr.get_status().attract
        };
        if target_plr.get_state::<SlowState>().is_some() {
            base /= 2.0;
        }
        base
    }

    fn act(&mut self, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        args.2.add(RunUpdate::new("[0]使用[减速术]", args.0, target_id, 1));

        let owner_magic = args.3.get_player(&args.0).expect("cannot get slow owner from storage").get_status().magic;
        let target = args.3.just_get_player_mut(target_id).expect("cannot get slow target from storage");
        if target.check_immune(state_tag::<SlowState>(), args.1)
            || (target.active() && Player::dodge(owner_magic, target.get_status().resistance, args.1))
        {
            args.2.add(RunUpdate::new("[0][回避]了攻击", target_id, args.0, 20));
            return;
        }

        let reduce_move_point = target.get_status().speed + 64;
        target.set_move_point(target.move_point() - reduce_move_point);
        if let Some(state) = target.get_state_mut::<SlowState>() {
            state.step += 2;
        } else {
            target.set_state(SlowState {
                owner: Some(args.0),
                target: Some(target_id),
                on_post_action: None,
                step: 2,
            });
        }
        args.2.add(RunUpdate::new("[1]进入[迟缓]状态", args.0, target_id, 60));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlowState {
    pub owner: Option<PlrId>,
    pub target: Option<PlrId>,
    pub on_post_action: Option<()>,
    pub step: i32,
}

impl Default for SlowState {
    fn default() -> Self {
        Self {
            owner: None,
            target: None,
            on_post_action: None,
            step: 2,
        }
    }
}

impl StateTrait for SlowState {
    fn meta_type(&self) -> i32 { -1 }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

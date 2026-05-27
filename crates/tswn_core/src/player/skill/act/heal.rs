//! 治疗主动技能实现。
//!
//! 为己方目标（或自身）恢复一定量生命值；在施放时同时驱散目标身上的诅咒/冰冻/魅惑状态。

use super::curse::CurseState;
use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::{
        SkillArgs, SkillExt, SkillTargetDomain, SkillTrait, berserk::BerserkState, charm::CharmState, ice::IceState,
        poison::PoisonState, slow::SlowState,
    },
};

#[derive(Debug, Clone, Default)]
pub struct HealSkill {
    pub allow_sneak: bool,
}

impl HealSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for HealSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for HealSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn post_act_level(&self, level: u32) -> u32 {
        // `治愈魔法` 只有在 9 级及以上时才会衰减；每次施放后 -1，8 级及以下不变。
        // 例如：`9 -> 8`、`10 -> 9`。
        // 这同样作用于战斗中的当前熟练度，所以 clone build 后必须 clamp。
        if level > 8 { level - 1 } else { level }
    }

    fn target_domain_with_level(&self, _level: u32) -> SkillTargetDomain { SkillTargetDomain::AllyAlive }

    fn valid_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> bool {
        let Some(target_plr) = args.3.get_player(&target) else {
            return false;
        };
        let status = target_plr.get_status();
        if smart {
            status.hp + 80 < status.max_hp
        } else {
            status.hp < status.max_hp
        }
    }

    fn score_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        let Some(target_plr) = args.3.get_player(&target) else {
            return f64::MIN;
        };
        if smart {
            let status = target_plr.get_status();
            let mut damaged = (status.max_hp - status.hp).max(0);
            damaged += (target_plr.negative_state_count() as i32) * 64;
            (damaged as f64) * (target_plr.attr_sum().max(1) as f64)
        } else {
            args.1.rFFFF() as f64
        }
    }

    fn act_with_level(&mut self, _level: u32, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let atp = args
            .3
            .get_player(&args.0)
            .expect("cannot get heal owner from storage")
            .get_at(true, args.1);
        let missing_hp = {
            let target = args.3.get_player(&target_id).expect("cannot get heal target from storage");
            (target.get_status().max_hp - target.get_status().hp).max(0)
        };
        if missing_hp <= 0 {
            return;
        }
        let heal = ((atp / 60.0).ceil() as i32).clamp(1, missing_hp);
        if crate::debug::debug_heal() {
            let owner = args.3.get_player(&args.0).expect("cannot get heal owner from storage");
            let target = args.3.get_player(&target_id).expect("cannot get heal target from storage");
            eprintln!(
                "[heal] owner={} target={} owner_hp={}/{} target_hp={}/{} at_boost={} atp={:.2} missing={} heal={} rc4=({}, {})",
                owner.id_name(),
                target.id_name(),
                owner.get_status().hp,
                owner.get_status().max_hp,
                target.get_status().hp,
                target.get_status().max_hp,
                owner.get_status().at_boost,
                atp,
                missing_hp,
                heal,
                args.1.i,
                args.1.j
            );
        }
        args.2.add(RunUpdate::new("[0]使用[治愈魔法]", args.0, target_id, heal as u32));
        let (had_slow, had_poison, had_ice, had_berserk, had_charm, had_curse) = {
            let target = args.3.just_get_player_mut(target_id).expect("cannot get heal target from storage");
            let had_slow = target.has_state::<SlowState>();
            let had_poison = target.has_state::<PoisonState>();
            let had_ice = target.has_state::<IceState>();
            let had_berserk = target.has_state::<BerserkState>();
            let had_charm = target.has_state::<CharmState>();
            let had_curse = target.has_state::<CurseState>();
            target.damage(-heal, args.0, on_heal, args.1, args.2, args.3);
            target.clear_negative_states();
            (had_slow, had_poison, had_ice, had_berserk, had_charm, had_curse)
        };
        // Dart 的 clearStates 会按字母序遍历 meta.keys：
        // berserk → charm → curse → ice → poison → slow
        if had_berserk {
            args.2.add(RunUpdate::new_newline());
            args.2.add(RunUpdate::new("[1]从[狂暴]中解除", args.0, target_id, 0));
        }
        if had_charm {
            args.2.add(RunUpdate::new_newline());
            args.2.add(RunUpdate::new("[1]从[魅惑]中解除", args.0, target_id, 0));
        }
        if had_curse {
            args.2.add(RunUpdate::new_newline());
            args.2.add(RunUpdate::new("[1]从[诅咒]中解除", args.0, target_id, 0));
        }
        if had_ice {
            args.2.add(RunUpdate::new_newline());
            args.2.add(RunUpdate::new("[1]从[冰冻]中解除", args.0, target_id, 0));
        }
        if had_poison {
            args.2.add(RunUpdate::new_newline());
            args.2.add(RunUpdate::new("[1]从[中毒]中解除", args.0, target_id, 0));
        }
        if had_slow {
            args.2.add(RunUpdate::new_newline());
            args.2.add(RunUpdate::new("[1]从[迟缓]中解除", args.0, target_id, 0));
        }
    }
}

fn on_heal(
    _caster: PlrId,
    _target: PlrId,
    _dmg: i32,
    _r: &mut crate::rc4::RC4,
    _updates: &mut crate::engine::update::RunUpdates,
    _storage: &std::sync::Arc<crate::engine::storage::Storage>,
) {
}

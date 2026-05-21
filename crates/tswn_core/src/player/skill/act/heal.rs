use super::curse::CurseState;
use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::{
        InlineCtx, SkillArgs, SkillExt, SkillTargetDomain, SkillTrait, berserk::BerserkState, charm::CharmState, ice::IceState,
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

    fn has_inline_act(&self) -> bool { true }

    fn has_action_impl(&self) -> bool { true }

    fn post_act_level(&self, level: u32) -> u32 { if level > 8 { level - 1 } else { level } }

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

    fn act_inline(&mut self, _level: u32, targets: Vec<PlrId>, _smart: bool, ctx: &mut InlineCtx) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let atp = ctx.owner.get_at(true, ctx.randomer);
        let missing_hp = if target_id == ctx.ptr {
            (ctx.owner.get_status().max_hp - ctx.owner.get_status().hp).max(0)
        } else {
            let target = ctx.storage.get_player(&target_id).expect("cannot get heal target from storage");
            (target.get_status().max_hp - target.get_status().hp).max(0)
        };
        if missing_hp <= 0 {
            return;
        }
        let heal = ((atp / 60.0).ceil() as i32).clamp(1, missing_hp);
        if crate::debug::debug_heal() {
            let target_name = if target_id == ctx.ptr {
                ctx.owner.id_name()
            } else {
                ctx.storage.get_player(&target_id).expect("cannot get heal target from storage").id_name()
            };
            eprintln!(
                "[heal] owner={} target={} owner_hp={}/{} target_hp={}/{} at_boost={} atp={:.2} missing={} heal={} rc4=({}, {})",
                ctx.owner.id_name(),
                target_name,
                ctx.owner.get_status().hp,
                ctx.owner.get_status().max_hp,
                if target_id == ctx.ptr {
                    ctx.owner.get_status().hp
                } else {
                    ctx.storage
                        .get_player(&target_id)
                        .expect("cannot get heal target from storage")
                        .get_status()
                        .hp
                },
                if target_id == ctx.ptr {
                    ctx.owner.get_status().max_hp
                } else {
                    ctx.storage
                        .get_player(&target_id)
                        .expect("cannot get heal target from storage")
                        .get_status()
                        .max_hp
                },
                ctx.owner.get_status().at_boost,
                atp,
                missing_hp,
                heal,
                ctx.randomer.i,
                ctx.randomer.j
            );
        }
        ctx.updates.add(RunUpdate::new("[0]使用[治愈魔法]", ctx.ptr, target_id, heal as u32));
        let (had_slow, had_poison, had_ice, had_berserk, had_charm, had_curse) = if target_id == ctx.ptr {
            let had_slow = ctx.owner.has_state::<SlowState>();
            let had_poison = ctx.owner.has_state::<PoisonState>();
            let had_ice = ctx.owner.has_state::<IceState>();
            let had_berserk = ctx.owner.has_state::<BerserkState>();
            let had_charm = ctx.owner.has_state::<CharmState>();
            let had_curse = ctx.owner.has_state::<CurseState>();
            ctx.owner.damage(-heal, ctx.ptr, on_heal, ctx.randomer, ctx.updates, ctx.storage);
            ctx.owner.clear_negative_states();
            (had_slow, had_poison, had_ice, had_berserk, had_charm, had_curse)
        } else {
            let target = ctx.storage.just_get_player_mut(target_id).expect("cannot get heal target from storage");
            let had_slow = target.has_state::<SlowState>();
            let had_poison = target.has_state::<PoisonState>();
            let had_ice = target.has_state::<IceState>();
            let had_berserk = target.has_state::<BerserkState>();
            let had_charm = target.has_state::<CharmState>();
            let had_curse = target.has_state::<CurseState>();
            target.damage(-heal, ctx.ptr, on_heal, ctx.randomer, ctx.updates, ctx.storage);
            target.clear_negative_states();
            (had_slow, had_poison, had_ice, had_berserk, had_charm, had_curse)
        };
        if had_berserk {
            ctx.updates.add(RunUpdate::new_newline());
            ctx.updates.add(RunUpdate::new("[1]从[狂暴]中解除", ctx.ptr, target_id, 0));
        }
        if had_charm {
            ctx.updates.add(RunUpdate::new_newline());
            ctx.updates.add(RunUpdate::new("[1]从[魅惑]中解除", ctx.ptr, target_id, 0));
        }
        if had_curse {
            ctx.updates.add(RunUpdate::new_newline());
            ctx.updates.add(RunUpdate::new("[1]从[诅咒]中解除", ctx.ptr, target_id, 0));
        }
        if had_ice {
            ctx.updates.add(RunUpdate::new_newline());
            ctx.updates.add(RunUpdate::new("[1]从[冰冻]中解除", ctx.ptr, target_id, 0));
        }
        if had_poison {
            ctx.updates.add(RunUpdate::new_newline());
            ctx.updates.add(RunUpdate::new("[1]从[中毒]中解除", ctx.ptr, target_id, 0));
        }
        if had_slow {
            ctx.updates.add(RunUpdate::new_newline());
            ctx.updates.add(RunUpdate::new("[1]从[迟缓]中解除", ctx.ptr, target_id, 0));
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
        // Dart clearStates iterates meta.keys sorted alphabetically:
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

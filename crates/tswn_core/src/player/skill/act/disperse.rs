use std::sync::Arc;

use smallvec::SmallVec;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId,
    skill::act::minion::is_combat_minion,
    skill::{InlineCtx, SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct DisperseSkill;

impl DisperseSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for DisperseSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for DisperseSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_inline_act(&self) -> bool { true }

    fn has_action_impl(&self) -> bool { true }

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
            let target_alive_group_len = args.3.alive_group_len_containing(target);
            let status = target_plr.get_status();
            if alive_group_count > 2 {
                rate_hi_hp(status.hp) * target_alive_group_len as f64 * status.attract
            } else {
                (1.0 / rate_hi_hp(status.hp)) * status.atk_sum as f64 * status.attract
            }
        } else {
            args.1.rFFFF() as f64 + target_plr.get_status().attract
        };
        if smart && is_combat_minion(target_plr) && target_plr.get_status().hp > 100 {
            score *= 2.0;
        }
        score
    }

    fn act_with_level(&mut self, _level: u32, targets: &[PlrId], _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let atp = args
            .3
            .get_player(&args.0)
            .expect("cannot get disperse owner from storage")
            .get_at(true, args.1);
        let target_is_minion = args.3.get_player(&target_id).map(is_combat_minion).unwrap_or(false);
        args.2.add(RunUpdate::new("[0]使用[净化]", args.0, target_id, 20));

        // Note: Dart source has 'Dt.shield'/'Dt.iron' (string literal) instead of Dt.shield/Dt.iron (variable)
        // so these checks NEVER match in Dart/JS. Shield/Iron are NOT pre-cleared before damage.

        let core = {
            let target = args.3.just_get_player_mut(target_id).expect("cannot get disperse target from storage");
            let (_, core) = target.defned_core(
                if target_is_minion { atp * 2.0 } else { atp },
                true,
                args.0,
                on_disperse as OnDamageFunc,
                args.1,
                args.2,
                args.3,
            );
            core
        };
        if !core.is_heal && !core.is_zero {
            on_disperse(args.0, target_id, core.actual_dmg, args.1, args.2, args.3);
            let target = args.3.just_get_player_mut(target_id).expect("cannot get disperse target from storage");
            target.finish_damage(core.actual_dmg, core.old_hp, args.0, args.1, args.2, args.3);
        }
    }

    fn act_inline(&mut self, _level: u32, targets: &[PlrId], _smart: bool, ctx: &mut InlineCtx) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let atp = ctx.owner.get_at(true, ctx.randomer);
        let target_is_minion = ctx.storage.get_player(&target_id).map(is_combat_minion).unwrap_or(false);
        ctx.updates.add(RunUpdate::new("[0]使用[净化]", ctx.ptr, target_id, 20));

        let core = {
            let target = ctx
                .storage
                .just_get_player_mut(target_id)
                .expect("cannot get disperse target from storage");
            let (_, core) = target.defned_core(
                if target_is_minion { atp * 2.0 } else { atp },
                true,
                ctx.ptr,
                on_disperse as OnDamageFunc,
                ctx.randomer,
                ctx.updates,
                ctx.storage,
            );
            core
        };
        if !core.is_heal && !core.is_zero {
            on_disperse(ctx.ptr, target_id, core.actual_dmg, ctx.randomer, ctx.updates, ctx.storage);
            let target = ctx
                .storage
                .just_get_player_mut(target_id)
                .expect("cannot get disperse target from storage");
            target.finish_damage(core.actual_dmg, core.old_hp, ctx.ptr, ctx.randomer, ctx.updates, ctx.storage);
        }
    }
}

fn on_disperse(caster: PlrId, target_id: PlrId, dmg: i32, r: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
    if dmg <= 0 {
        return;
    }
    // JS/Dart clears positive meta in sorted meta-key order.
    // Rust stores positive skill/runtime meta and state meta separately, so we collect both with
    // their stable type-name tags, then sort once before emitting cancel messages.
    let target = storage.just_get_player_mut(target_id).expect("cannot get disperse target from storage");
    let target_ptr: *mut crate::player::Player = target;
    let mut clear_messages = {
        let mut ctx = InlineCtx {
            ptr: target_id,
            owner: unsafe { &mut *target_ptr },
            randomer: r,
            updates,
            storage,
            post_damage: None,
            effects: SmallVec::new(),
            needs_update_states: false,
        };
        let messages = unsafe { &mut (*target_ptr).skills }.clear_positive_runtime_with_order_inline(&mut ctx);
        if ctx.needs_update_states {
            unsafe { &mut *target_ptr }.update_states();
        }
        messages
    };
    let target = unsafe { &mut *target_ptr };
    clear_messages.extend(target.clear_positive_states_with_ordered_messages());
    clear_messages.sort_unstable_by_key(|(priority, _)| *priority);
    let mp = target.get_status().magic_point;
    if mp > 64 {
        target.set_magic_point(mp - 64);
    } else if mp > 32 {
        target.set_magic_point(0);
    } else {
        target.set_magic_point(mp - 32);
    }
    for (_, message) in clear_messages {
        updates.emit(RunUpdate::new_newline);
        updates.emit(|| RunUpdate::new(message, caster, target_id, 0));
    }
}

//! 驱散主动技能实现。
//!
//! 清除目标身上的所有负面（或正面）状态效果，使其恢复原始状态。

use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId,
    skill::act::minion::is_combat_minion,
    skill::{SkillArgs, SkillExt, SkillTrait},
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

    fn act_with_level(&mut self, _level: u32, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
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

        // 注意：Dart 源码写的是 'Dt.shield'/'Dt.iron'（字符串字面量），
        // 不是 Dt.shield/Dt.iron（变量），所以这些检查在 Dart/JS 中永远不会命中。
        // 也就是说 Shield/Iron 不会在伤害前被提前清掉。

        args.3
            .just_get_player_mut(target_id)
            .expect("cannot get disperse target from storage")
            .defned(
                if target_is_minion { atp * 2.0 } else { atp },
                true,
                args.0,
                on_disperse as OnDamageFunc,
                args.1,
                args.2,
                args.3,
            );
    }
}

fn on_disperse(caster: PlrId, target_id: PlrId, dmg: i32, r: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
    if dmg <= 0 {
        return;
    }
    // JS/Dart 会按排好序的 meta-key 顺序清除正面 meta。
    // Rust 把正面技能/运行时 meta 与 state meta 分开存储，所以这里先统一收集
    // 它们稳定的类型名标签，再排序一次后输出取消消息。
    let target = storage.just_get_player_mut(target_id).expect("cannot get disperse target from storage");
    let mut clear_messages = target.skills.clear_positive_runtime_with_order((target_id, r, updates, storage));
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
        updates.add_newline();
        updates.emit(|| RunUpdate::new(message, caster, target_id, 0));
    }
}

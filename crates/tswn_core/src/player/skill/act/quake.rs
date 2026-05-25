use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone)]
pub struct QuakeSkill {
    pub sel_count: usize,
    pub sel_count_smart: usize,
}

impl Default for QuakeSkill {
    fn default() -> Self {
        Self {
            sel_count: 5,
            sel_count_smart: 6,
        }
    }
}

impl QuakeSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for QuakeSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for QuakeSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn select_target_count(&self, smart: bool) -> usize { if smart { self.sel_count_smart } else { self.sel_count } }

    fn act_with_level(&mut self, _level: u32, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let round = if args.1.c50() { 5 } else { 4 };
        let mut picked = targets;
        picked.truncate(round.min(picked.len()));
        if picked.is_empty() {
            return;
        }
        args.2.add(RunUpdate::new("[0]使用[地裂术]", args.0, picked[0], 1));
        let divisor = picked.len() as f64 + 0.6000000238418579;
        for target_id in picked {
            // JS 会在检查 hp > 0 之前先调用 getAt，因此即使目标已死也会消耗 RC4。
            let owner = args.3.get_player(&args.0).expect("cannot get quake owner from storage");
            // 这里必须保留反编译出来的 JS 浮点字面量；2.44 会让部分 case 跨过 ceil() 边界。
            let atp = owner.get_at(true, args.1) * 2.440000057220459 / divisor;
            let target_alive = args.3.get_player(&target_id).map(|p| p.get_status().hp > 0).unwrap_or(false);
            if !target_alive {
                continue;
            }
            args.2.add(RunUpdate::new_newline());
            args.3
                .just_get_player_mut(target_id)
                .expect("cannot get quake target from storage")
                .attacked(atp, true, args.0, on_quake as OnDamageFunc, args.1, args.2, args.3);
            if quake_should_stop(args.3) {
                break;
            }
        }
    }
}

fn quake_should_stop(storage: &Arc<Storage>) -> bool { quake_battle_over(storage) }

fn quake_battle_over(storage: &Arc<Storage>) -> bool {
    let mut first_group: Option<usize> = None;
    for id in storage.iter_player_ids() {
        if storage.get_player(&id).map(|player| player.alive()).unwrap_or(false)
            && let Some(group_idx) = storage.group_index_of(id)
        {
            match first_group {
                None => first_group = Some(group_idx),
                Some(existing) if existing != group_idx => return false,
                _ => {}
            }
        }
    }

    for pending in storage.iter_pending_spawns() {
        if pending.player.alive()
            && let Some(group_idx) = storage.group_index_of(pending.owner)
        {
            match first_group {
                None => first_group = Some(group_idx),
                Some(existing) if existing != group_idx => return false,
                _ => {}
            }
        }
    }

    true
}

fn on_quake(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates, _storage: &Arc<Storage>) {}

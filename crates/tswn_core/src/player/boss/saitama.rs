//! 埼玉 Boss（SaitamaBoss）实现。
//!
//! 维护 `SaitamaState`，模拟"一拳超人"机制：存活若干轮后必定一击秒杀当前目标。

use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{ActionTargets, Player, PlrId, StateTrait, noop_on_damage};
use crate::rc4::RC4;

#[derive(Clone, Debug)]
pub struct SaitamaState {
    pub turns: i32,
    pub damages: i32,
    pub hitters: std::collections::HashSet<PlrId>,
    pub minions: std::collections::HashSet<PlrId>,
}

impl StateTrait for SaitamaState {
    fn meta_type(&self) -> i32 { 0 }

    fn post_defend_priority(&self) -> i32 { i32::MAX }
    fn on_post_defend(
        &mut self,
        _owner: PlrId,
        dmg: &mut i32,
        caster: PlrId,
        _randomer: &mut RC4,
        _updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> bool {
        self.damages += *dmg;
        if let Some(caster_plr) = storage.get_player(&caster) {
            if let Some(minion_state) = caster_plr.get_state::<crate::player::skill::act::minion::MinionRuntimeState>() {
                if let Some(owner_id) = minion_state.owner {
                    self.hitters.insert(owner_id);
                    self.minions.insert(caster);
                } else {
                    self.hitters.insert(caster);
                }
            } else {
                self.hitters.insert(caster);
            }
        } else {
            self.hitters.insert(caster);
        }
        *dmg /= 100;
        false
    }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(self.clone()) }
}

pub fn saitama_boss_action(
    player: &mut Player,
    smart: bool,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
    targets: &ActionTargets,
) {
    let boss_id = player.as_ptr();

    let selected_target = player.select_default_attack_target(smart, randomer, storage, targets);

    let (damages, hitters_len, minions_len) = player
        .get_state::<SaitamaState>()
        .map(|s| (s.damages, s.hitters.len() as i32, s.minions.len() as i32))
        .unwrap_or((0, 0, 0));

    let hunger_denominator = hitters_len + minions_len / 3 + 1;
    if damages / hunger_denominator.max(1) > 255 {
        let boss_display = player.display_name();
        let mut hungry_update = RunUpdate::new(format!("{boss_display}觉得有点饿"), boss_id, boss_id, 0);
        hungry_update.delay1 = 2000;
        updates.add(hungry_update);
        updates.add(RunUpdate::new_newline());
        updates.add(RunUpdate::new(format!(" {boss_display}离开了战场"), boss_id, boss_id, 0));
        let old_hp = player.get_status().hp;
        player.apply_raw_damage(old_hp);
        player.status.set_alive(false);
        storage.record_death(boss_id);
        return;
    }

    let turns = player.get_state::<SaitamaState>().map(|s| s.turns).unwrap_or(0);

    if turns < 10 {
        if let Some(state) = player.get_state_mut::<SaitamaState>() {
            state.turns += 1;
        }
        return;
    }

    let Some(target_id) = selected_target else {
        return;
    };
    let atp = player.get_at(false, randomer) * 12.0;
    updates.add(RunUpdate::new("[0]发起攻击", boss_id, target_id, 0));
    storage.just_get_player_mut(target_id).expect("saitama attack target").attacked(
        atp,
        false,
        boss_id,
        noop_on_damage,
        randomer,
        updates,
        storage,
    );

    if let Some(boss_group) = storage.group_containing(boss_id) {
        let group = boss_group.clone();
        for &member_id in &group {
            if let Some(member) = storage.just_get_player_mut(member_id) {
                member.set_move_point(0);
            }
        }
    }
    player.set_move_point(1700);
}

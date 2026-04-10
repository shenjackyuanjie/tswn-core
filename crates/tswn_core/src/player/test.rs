//! # 玩家测试 (test)
//!
//! 拆分后的 player 测试入口。

use super::*;
use crate::engine::update::UpdateType;

fn noop_on_damage(_: PlrId, _: PlrId, _: i32, _: &mut RC4, _: &mut RunUpdates, _: &std::sync::Arc<Storage>) {}

#[derive(Debug, Clone, Default)]
struct ObservePreDefendByteSkill;

impl crate::player::skill::SkillTrait for ObservePreDefendByteSkill {
    fn destroy(&self, _plr: PlrId, _args: crate::player::skill::SkillArgs) {}

    fn clone_box(&self) -> Box<dyn crate::player::skill::SkillTrait> { Box::new(self.clone()) }

    fn pre_defend(
        &mut self,
        atp: f64,
        _caster: PlrId,
        _is_mag: bool,
        _on_damage: &OnDamageFunc,
        args: crate::player::skill::SkillArgs,
    ) -> f64 {
        let byte = args.1.next_u8();
        args.2.add(crate::engine::update::RunUpdate::new(
            format!("pre_defend_skill_byte={byte}"),
            args.0,
            args.0,
            1,
        ));
        atp
    }

    fn proc_kinds(&self) -> &[crate::player::skill::ProcKind] { &[crate::player::skill::ProcKind::PreDefend] }
}

#[derive(Debug, Clone, Default)]
struct ObserveChargeBoostState;

impl crate::player::state::StateTrait for ObserveChargeBoostState {
    fn clone_box(&self) -> Box<dyn crate::player::state::StateTrait> { Box::new(self.clone()) }

    fn on_post_action(
        &mut self,
        owner: PlrId,
        _alive: bool,
        _randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &std::sync::Arc<Storage>,
    ) -> bool {
        let boosted = storage
            .get_player(&owner)
            .map(|player| player.get_status().at_boost > 1.0)
            .unwrap_or(false);
        updates.add(crate::engine::update::RunUpdate::new(
            if boosted {
                "observe_charge_boost=boosted"
            } else {
                "observe_charge_boost=normal"
            },
            owner,
            owner,
            1,
        ));
        false
    }
}

mod basic;
mod boss;
mod minions;
mod shadow_sync;
mod skill_store;
mod skills;
mod states;
mod weapons;

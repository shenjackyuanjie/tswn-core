use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId, StateTrait,
    skill::act::minion::is_combat_minion,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtectLink {
    pub owner: PlrId,
    pub level: u32,
}

#[derive(Debug, Clone, Default)]
pub struct ProtectState {
    pub target: Option<PlrId>,
    pub protect_from: Vec<ProtectLink>,
}

impl StateTrait for ProtectState {
    fn meta_type(&self) -> i32 { 0 }

    fn pre_defend_priority(&self) -> i32 { 100 }

    #[allow(clippy::too_many_arguments)]
    fn on_pre_defend(
        &mut self,
        owner: PlrId,
        atp: &mut f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<crate::engine::storage::Storage>,
    ) -> bool {
        let links = self.protect_from.clone();
        if links.is_empty() {
            return false;
        }
        let mut stale_owners = Vec::new();
        for link in links {
            let protector_alive = storage.get_player(&link.owner).map(|p| p.alive()).unwrap_or(false);
            if !protector_alive {
                stale_owners.push(link.owner);
                continue;
            }
            if randomer.r127() >= link.level {
                continue;
            }
            let protector_ready = {
                let protector = storage.just_get_player_mut(link.owner).expect("cannot get protect owner from storage");
                protector.mp_ready(randomer)
            };
            if !protector_ready {
                continue;
            }
            updates.add(RunUpdate::new("[0][守护][1]", link.owner, owner, 40));
            let redirected_atp = {
                let protector = storage.just_get_player_mut(link.owner).expect("cannot get protect owner from storage");
                protector.pre_defend(*atp, is_mag, caster, on_damage, randomer, updates, storage)
            };
            if redirected_atp == 0.0 {
                *atp = 0.0;
                return false;
            }
            let mut redirected_dmg = {
                let protector = storage.get_player(&link.owner).expect("cannot get protect owner from storage");
                (redirected_atp * 0.5 / protector.get_df(is_mag) as f64).floor() as i32
            };
            redirected_dmg = {
                let protector = storage.just_get_player_mut(link.owner).expect("cannot get protect owner from storage");
                protector.post_defend(redirected_dmg, caster, on_damage, randomer, updates, storage)
            };
            storage
                .just_get_player_mut(link.owner)
                .expect("cannot get protect owner from storage")
                .damage(redirected_dmg, caster, on_damage, randomer, updates, storage);
            *atp = 0.0;
            return false;
        }

        if !stale_owners.is_empty() {
            self.protect_from.retain(|entry| !stale_owners.contains(&entry.owner));
        }
        self.protect_from.is_empty()
    }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(self.clone()) }
}

#[derive(Debug, Clone, Default)]
pub struct ProtectSkill {
    pub allow_sneak: bool,
    pub protect_to: Option<PlrId>,
}

impl ProtectSkill {
    pub fn new() -> Self { Self::default() }

    fn pick_target(&mut self, level: u32, args: SkillArgs) -> Option<PlrId> {
        let group = if let Some(group) = args.3.group_containing(args.0) {
            group.clone()
        } else {
            let owner_clan = args.3.get_player(&args.0).expect("cannot get protect owner from storage").clan_name();
            args.3
                .all_player_ids()
                .into_iter()
                .filter(|id| args.3.get_player(id).map(|p| p.clan_name() == owner_clan).unwrap_or(false))
                .collect::<Vec<PlrId>>()
        };
        let owner_wisdom = args
            .3
            .get_player(&args.0)
            .expect("cannot get protect owner from storage")
            .get_status()
            .wisdom
            .clamp(0, 127) as u32;
        let smart = args.1.r127() < owner_wisdom;

        let mut best_target = None;
        let mut best_score = f64::MIN;
        for candidate in group {
            if candidate == args.0 {
                continue;
            }
            let Some(target) = args.3.get_player(&candidate) else {
                continue;
            };
            if !target.alive() {
                continue;
            }
            if is_combat_minion(target) {
                continue;
            }
            let score = if smart {
                let hp = target.get_status().hp.max(0) as f64;
                let max_hp = target.get_status().max_hp.max(1) as f64;
                let protect_len =
                    target.get_state::<ProtectState>().map(|state| state.protect_from.len() + 1).unwrap_or(1) as f64;
                ((max_hp - hp) / max_hp) * target.attr_sum().max(1) as f64 / protect_len
            } else {
                args.1.rFFFF() as f64
            };
            if score > best_score {
                best_score = score;
                best_target = Some(candidate);
            }
        }

        if best_target.is_none() && level > 0 {
            best_target = self.protect_to;
        }
        best_target
    }

    fn unregister_owner(&self, owner: PlrId, target_id: PlrId, args: SkillArgs) {
        let mut clear_state = false;
        if let Some(target) = args.3.just_get_player_mut(target_id)
            && let Some(state) = target.get_state_mut::<ProtectState>()
        {
            state.protect_from.retain(|entry| entry.owner != owner);
            clear_state = state.protect_from.is_empty();
        }
        if clear_state && let Some(target) = args.3.just_get_player_mut(target_id) {
            target.clear_state::<ProtectState>();
        }
    }

    fn register_owner(&self, owner: PlrId, level: u32, target_id: PlrId, args: SkillArgs) {
        let target = args.3.just_get_player_mut(target_id).expect("cannot get protect target from storage");
        if let Some(state) = target.get_state_mut::<ProtectState>() {
            if let Some(entry) = state.protect_from.iter_mut().find(|entry| entry.owner == owner) {
                entry.level = level;
            } else {
                state.protect_from.push(ProtectLink { owner, level });
            }
            state.target = Some(target_id);
            return;
        }
        target.set_state(ProtectState {
            target: Some(target_id),
            protect_from: vec![ProtectLink { owner, level }],
        });
    }
}

impl SkillExt for ProtectSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ProtectSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn post_action_with_level(&mut self, level: u32, args: SkillArgs) {
        let next_target = self.pick_target(level, (args.0, args.1, args.2, args.3));
        if self.protect_to == next_target {
            return;
        }
        if let Some(old_target) = self.protect_to {
            self.unregister_owner(args.0, old_target, (args.0, args.1, args.2, args.3));
        }
        self.protect_to = next_target;
        if let Some(target_id) = next_target {
            self.register_owner(args.0, level, target_id, (args.0, args.1, args.2, args.3));
        }
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostAction] }
}

use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId, StateTrait,
    skill::act::minion::MinionRuntimeState,
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
        let debug_target = std::env::var("TSWN_DEBUG_PROTECT").ok();
        let debug_this = debug_target
            .as_deref()
            .map(|name| storage.get_player(&owner).map(|p| p.id_name() == name).unwrap_or(false))
            .unwrap_or(false);
        if debug_this {
            eprintln!(
                "[protect_pre_defend] owner={} links={} rc4=({}, {})",
                storage.get_player(&owner).map(|p| p.id_name()).unwrap_or_else(|| format!("#{owner}")),
                links.len(),
                randomer.i,
                randomer.j
            );
        }
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
            let roll = randomer.r127();
            if debug_this {
                eprintln!(
                    "[protect_pre_defend] link_owner={} level={} roll={} rc4=({}, {})",
                    storage
                        .get_player(&link.owner)
                        .map(|p| p.id_name())
                        .unwrap_or_else(|| format!("#{}", link.owner)),
                    link.level,
                    roll,
                    randomer.i,
                    randomer.j
                );
            }
            if roll >= link.level {
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

    fn pick_target(&mut self, _level: u32, args: SkillArgs) -> Option<PlrId> {
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
        let alive_group = group
            .into_iter()
            .filter(|id| args.3.get_player(id).map(|p| p.alive()).unwrap_or(false))
            .collect::<Vec<PlrId>>();
        if alive_group.is_empty() {
            return None;
        }
        let pending_spawns = args.3.pending_spawn_count_for_owner(args.0);
        let mut candidates = alive_group.iter().copied().map(Some).collect::<Vec<Option<PlrId>>>();
        if pending_spawns > 0 {
            candidates.extend(std::iter::repeat(None).take(pending_spawns));
        }
        if candidates.is_empty() {
            return None;
        }
        let owner_wisdom = args
            .3
            .get_player(&args.0)
            .expect("cannot get protect owner from storage")
            .get_status()
            .wisdom
            .clamp(0, 127) as u32;
        let smart = args.1.r127() < owner_wisdom;
        let owner_pos = candidates.iter().position(|entry| *entry == Some(args.0));

        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let next_idx = if let Some(pos) = owner_pos {
                args.1.pick_skip(&candidates, pos)
            } else {
                args.1.pick(&candidates)
            };
            let Some(idx) = next_idx else {
                return None;
            };
            let Some(target_id) = candidates[idx] else {
                invalid += 1;
                continue;
            };
            let valid = args
                .3
                .get_player(&target_id)
                .map(|target| !target.has_state::<MinionRuntimeState>())
                .unwrap_or(false);
            if !valid {
                invalid += 1;
                continue;
            }
            if selected.contains(&target_id) {
                dup += 1;
                continue;
            }
            selected.push(target_id);
            if selected.len() >= select_count {
                break;
            }
        }
        if selected.is_empty() {
            return None;
        }

        let mut scored = selected
            .into_iter()
            .map(|target_id| {
                let score = args
                    .3
                    .get_player(&target_id)
                    .map(|target| {
                        if smart {
                            let hp = target.get_status().hp;
                            let rate_hi_hp = if hp < 20 {
                                30.0
                            } else if hp > 300 {
                                300.0
                            } else {
                                hp as f64
                            };
                            let protect_len =
                                target.get_state::<ProtectState>().map(|state| state.protect_from.len() + 1).unwrap_or(1) as f64;
                            (1.0 / rate_hi_hp) * target.get_status().atk_sum as f64 / protect_len
                        } else {
                            args.1.rFFFF() as f64
                        }
                    })
                    .unwrap_or(f64::MIN);
                (target_id, score)
            })
            .collect::<Vec<(PlrId, f64)>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.first().map(|x| x.0)
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

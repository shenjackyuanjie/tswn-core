use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId, StateTrait,
    skill::act::charm::CharmState,
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

fn effective_group(storage: &Arc<crate::engine::storage::Storage>, plr: PlrId) -> Option<Vec<PlrId>> {
    storage.get_player(&plr).and_then(|player| {
        player
            .get_state::<CharmState>()
            .and_then(|charm| {
                charm
                    .effective_team_idx
                    .and_then(|team_idx| storage.get_group(team_idx).cloned())
                    .or_else(|| storage.group_containing(charm.group_id).cloned())
            })
            .or_else(|| storage.group_containing(plr).cloned())
    })
}

/// 返回玩家所在队伍的 alive 列表（保持 alive 顺序，匹配 Dart 的 Grp.alives）。
/// 与 effective_group 不同：effective_group 返回 roster（含死亡成员），
/// 此函数直接返回 alive 列表，复活成员在队尾（与 Dart 一致）。
fn effective_alive_group(storage: &Arc<crate::engine::storage::Storage>, plr: PlrId) -> Option<Vec<PlrId>> {
    storage.get_player(&plr).and_then(|player| {
        player
            .get_state::<CharmState>()
            .and_then(|charm| {
                charm
                    .effective_team_idx
                    .and_then(|team_idx| storage.alive_group_at(team_idx).cloned())
                    .or_else(|| storage.alive_group_at_team_of(charm.group_id).cloned())
            })
            .or_else(|| storage.alive_group_at_team_of(plr).cloned())
    })
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
        let debug_action = std::env::var("TSWN_DEBUG_ACTION").ok();
        let debug_this = debug_action
            .as_deref()
            .map(|name| storage.get_player(&owner).map(|p| p.id_name() == name).unwrap_or(false))
            .unwrap_or(false);
        if debug_this {
            eprintln!(
                "[protect_pre_defend] owner={} links={} atp={} rc4=({}, {})",
                storage.get_player(&owner).map(|p| p.id_name()).unwrap_or_else(|| format!("#{}", owner)),
                self.protect_from.len(),
                *atp,
                randomer.i,
                randomer.j,
            );
        }
        while !self.protect_from.is_empty() {
            let Some(idx) = randomer.pick(&self.protect_from) else {
                break;
            };
            let link = self.protect_from[idx];
            // Dart: pskl.owner.allyGroup == target.group
            // protector uses allyGroup (affected by charm), target uses original group
            let protector_group = effective_group(storage, link.owner);
            let target_group = storage.group_containing(owner).cloned();
            let same_group = protector_group == target_group;

            let trigger_ok = same_group && randomer.r127() < link.level;
            let protector_ready = if trigger_ok {
                storage
                    .just_get_player_mut(link.owner)
                    .map(|protector| protector.mp_ready(randomer))
                    .unwrap_or(false)
            } else {
                false
            };

            if debug_this {
                eprintln!(
                    "[protect_pre_defend] owner={} picked_link_owner={} same_group={} trigger_ok={} protector_ready={} rc4=({}, {})",
                    storage.get_player(&owner).map(|p| p.id_name()).unwrap_or_else(|| format!("#{}", owner)),
                    storage
                        .get_player(&link.owner)
                        .map(|p| p.id_name())
                        .unwrap_or_else(|| format!("#{}", link.owner)),
                    same_group,
                    trigger_ok,
                    protector_ready,
                    randomer.i,
                    randomer.j,
                );
            }

            if trigger_ok && protector_ready {
                {
                    let protector = storage.just_get_player_mut(link.owner).expect("cannot get protect owner from storage");
                    if let Some(protect_skill) = protector.skills.store.get_mut(&26)
                        && protect_skill.level() > 0
                    {
                        protect_skill.post_action((link.owner, randomer, updates, storage));
                    }
                }
                updates.emit(|| RunUpdate::new("[0][守护][1]", link.owner, owner, 40));
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

            self.protect_from.remove(idx);
            // JS: p.Q = null — protector 的 protectTo 在保护失败时被清除
            if let Some(protector) = storage.just_get_player_mut(link.owner)
                && let Some(protect_skill) = protector.skills.store.get_mut(&26)
            {
                protect_skill.clear_protect_to();
            }
        }
        if debug_this {
            eprintln!(
                "[protect_pre_defend] owner={} end links={} atp={} rc4=({}, {})",
                storage.get_player(&owner).map(|p| p.id_name()).unwrap_or_else(|| format!("#{}", owner)),
                self.protect_from.len(),
                *atp,
                randomer.i,
                randomer.j,
            );
        }
        self.protect_from.is_empty()
    }

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
        let debug_action = std::env::var("TSWN_DEBUG_ACTION").ok();
        let debug_this = debug_action
            .as_deref()
            .map(|name| args.3.get_player(&args.0).map(|p| p.id_name() == name).unwrap_or(false))
            .unwrap_or(false);
        let group = if let Some(group) = effective_alive_group(args.3, args.0) {
            group
        } else if let Some(group) = args.3.alive_group_containing(args.0) {
            group.clone()
        } else {
            let owner_clan = args.3.get_player(&args.0).expect("cannot get protect owner from storage").clan_name();
            args.3
                .all_player_ids()
                .into_iter()
                .filter(|id| args.3.get_player(id).map(|p| p.clan_name() == owner_clan && p.alive()).unwrap_or(false))
                .collect::<Vec<PlrId>>()
        };
        // 基于 team.alive 顺序，但过滤掉本轮死亡尚未 sync 的成员
        let alive_group: Vec<PlrId> = group
            .iter()
            .copied()
            .filter(|id| args.3.get_player(id).map(|p| p.alive()).unwrap_or(false))
            .collect();
        let mut candidates = alive_group;
        // 追加刚复活但尚未 sync 到 team.alive 的成员（Dart 立刻加入 alives 末尾）
        let roster = effective_group(args.3, args.0)
            .or_else(|| args.3.group_containing(args.0).cloned())
            .unwrap_or_default();
        for id in &roster {
            if !candidates.contains(id) && args.3.get_player(id).map(|p| p.alive()).unwrap_or(false) {
                candidates.push(*id);
            }
        }
        // 追加整个队伍的 pending spawn（Dart 中 addNew 立刻加入 alives）
        candidates.extend(args.3.pending_spawn_ids_for_group(&roster));
        let owner_wisdom = args
            .3
            .get_player(&args.0)
            .expect("cannot get protect owner from storage")
            .get_status()
            .wisdom
            .max(0) as u32;
        // JS `SklProtect.cI()` 先消耗 smart roll，再让 `aa()` 处理空队伍/null 结果。
        // 所以 charm 指向的队伍 alive 列表为空时，Protect 仍会前进 1 个 RC4 字节。
        let smart = args.1.r127() < owner_wisdom;
        if candidates.is_empty() {
            return None;
        }
        let owner_pos = candidates.iter().position(|entry| *entry == args.0);
        if debug_this {
            let candidate_names = candidates
                .iter()
                .map(|id| {
                    args.3
                        .get_player_or_pending(id)
                        .map(|target| target.id_name())
                        .unwrap_or_else(|| format!("#{id}"))
                })
                .collect::<Vec<_>>();
            eprintln!(
                "[protect_pick] owner={} smart={} wisdom={} candidates={:?} rc4=({}, {})",
                args.3.get_player(&args.0).map(|p| p.id_name()).unwrap_or_else(|| format!("#{}", args.0)),
                smart,
                owner_wisdom,
                candidate_names,
                args.1.i,
                args.1.j
            );
        }

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
            let idx = next_idx?;
            let target_id = candidates[idx];
            let valid = args
                .3
                .get_player_or_pending(&target_id)
                .map(|target| !is_combat_minion(target))
                .unwrap_or(false);
            if debug_this {
                let picked_name = args
                    .3
                    .get_player_or_pending(&target_id)
                    .map(|target| target.id_name())
                    .unwrap_or_else(|| format!("#{target_id}"));
                eprintln!(
                    "[protect_pick] picked={} valid={} selected={:?} rc4=({}, {})",
                    picked_name,
                    valid,
                    selected
                        .iter()
                        .map(|id| args
                            .3
                            .get_player_or_pending(id)
                            .map(|target| target.id_name())
                            .unwrap_or_else(|| format!("#{id}")))
                        .collect::<Vec<_>>(),
                    args.1.i,
                    args.1.j
                );
            }
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
                    .get_player_or_pending(&target_id)
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
                if debug_this {
                    let target_name = args
                        .3
                        .get_player_or_pending(&target_id)
                        .map(|target| target.id_name())
                        .unwrap_or_else(|| format!("#{target_id}"));
                    eprintln!(
                        "[protect_pick] score target={} score={} rc4=({}, {})",
                        target_name, score, args.1.i, args.1.j
                    );
                }
                (target_id, score)
            })
            .collect::<Vec<(PlrId, f64)>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        if debug_this {
            let chosen = scored
                .first()
                .and_then(|(target_id, _)| args.3.get_player_or_pending(target_id).map(|target| target.id_name()));
            eprintln!("[protect_pick] chosen={:?} rc4=({}, {})", chosen, args.1.i, args.1.j);
        }
        scored.first().map(|x| x.0)
    }

    fn unregister_owner(&self, owner: PlrId, target_id: PlrId, args: SkillArgs) {
        let mut clear_state = false;
        if let Some(target) = args.3.just_get_player_or_pending_mut(target_id)
            && let Some(state) = target.get_state_mut::<ProtectState>()
        {
            state.protect_from.retain(|entry| entry.owner != owner);
            clear_state = state.protect_from.is_empty();
        }
        if clear_state && let Some(target) = args.3.just_get_player_or_pending_mut(target_id) {
            target.clear_state::<ProtectState>();
        }
    }

    fn register_owner(&self, owner: PlrId, level: u32, target_id: PlrId, args: SkillArgs) {
        let target = args
            .3
            .just_get_player_or_pending_mut(target_id)
            .expect("cannot get protect target from storage or pending spawn");
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

    fn link_registered(owner: PlrId, target_id: PlrId, args: SkillArgs) -> bool {
        args.3
            .get_player_or_pending(&target_id)
            .and_then(|target| target.get_state::<ProtectState>())
            .map(|state| state.protect_from.iter().any(|entry| entry.owner == owner))
            .unwrap_or(false)
    }
}

impl SkillExt for ProtectSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ProtectSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn clear_protect_to(&mut self) { self.protect_to = None; }

    fn post_action_with_level(&mut self, level: u32, args: SkillArgs) {
        let next_target = self.pick_target(level, (args.0, args.1, args.2, args.3));
        if let Ok(probe_owner) = std::env::var("TSWN_PROBE_PROTECT") {
            let owner_name = args.3.get_player(&args.0).map(|p| p.id_name()).unwrap_or_default();
            if owner_name == probe_owner {
                let old_name = self.protect_to.and_then(|id| args.3.get_player_or_pending(&id).map(|p| p.id_name()));
                let next_name = next_target.and_then(|id| args.3.get_player_or_pending(&id).map(|p| p.id_name()));
                eprintln!(
                    "[protect_probe] owner={} level={} old={:?} new={:?} rc4=({}, {})",
                    owner_name, level, old_name, next_name, args.1.i, args.1.j
                );
            }
        }
        let debug_action = std::env::var("TSWN_DEBUG_ACTION").ok();
        let debug_this = debug_action
            .as_deref()
            .map(|name| args.3.get_player(&args.0).map(|p| p.id_name() == name).unwrap_or(false))
            .unwrap_or(false);
        if debug_this {
            let owner_name = args.3.get_player(&args.0).map(|p| p.id_name()).unwrap_or_else(|| format!("#{}", args.0));
            let next_name = next_target.and_then(|id| args.3.get_player_or_pending(&id).map(|p| p.id_name()));
            eprintln!(
                "[protect_post_action] owner={} current={:?} next={:?} rc4=({}, {})",
                owner_name, self.protect_to, next_name, args.1.i, args.1.j
            );
        }
        if self.protect_to == next_target {
            if let Some(target_id) = next_target {
                if Self::link_registered(args.0, target_id, (args.0, args.1, args.2, args.3)) {
                    return;
                }
            } else {
                return;
            }
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

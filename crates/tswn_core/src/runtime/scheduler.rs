//! 行动选择与技能/状态 hook 计划；效果结算由 CombatRuntime 接续处理。
//!
//! [`ActionSchedulerMode::LegacyStep`] 对齐名字竞技场的速度行动条：每次 tick 从战斗
//! RNG 取步长，行动点**严格大于** [`MOVE_POINT_THRESHOLD`] 才行动，再减去阈值。
//! [`ActionSchedulerMode::Minimal`] 用于低层最小模板，适合单独验证阶段机制。
//!
//! 技能计划按 `(priority, active_order, registration_order)` 排序，并保留
//! `fixed_lane` 供 handler 查询当前等级。状态计划按 `(priority, registration_order)`
//! 稳定排序，同时携带运行期注册游标。数字较小的 priority 先执行。
//! POST_ACTION 按 early、状态间 deferred、late 分区执行。
//! 计划是派生数据，技能或状态变动后按 generation 刷新。

use crate::runtime::entity::{EntityArena, EntityIdx, SkillLoadout};
use crate::runtime::extension::{
    ExtensionRegistry, ProcMask, RegistrationOrder, SkillId, SkillPostActionPhase, SkillPriority, StateId, TargetPolicy,
};
use crate::runtime::world::WorldArena;
use crate::{rc4::RC4, runtime::MOVE_POINT_THRESHOLD};
use smallvec::SmallVec;

/// 调度器选出的一次行动：行动者、目标与行动量。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionPlan {
    pub actor: EntityIdx,
    pub target: EntityIdx,
    pub amount: i32,
}

/// 技能 hook 计划条目：owner、技能、目标策略、优先级与固定 lane。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillHookPlanEntry {
    pub owner: EntityIdx,
    pub skill_id: SkillId,
    pub target_policy: TargetPolicy,
    pub priority: SkillPriority,
    pub active_order: usize,
    pub fixed_lane: usize,
    pub registration_order: RegistrationOrder,
}

/// 一个 owner 在某个 hook 下的技能执行计划。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillHookPlan {
    pub owner: EntityIdx,
    pub hook: ProcMask,
    pub loadout_len: usize,
    pub entries: SmallVec<[SkillHookPlanEntry; 4]>,
}

#[derive(Debug)]
pub struct SkillPostActionPlans {
    pub generation: u32,
    /// 行动后早期技能通常只有少量条目，私有计划内联两项覆盖常见阵容。
    pub early: SmallVec<[SkillHookPlanEntry; 2]>,
    pub deferred: SmallVec<[(u64, SkillHookPlanEntry); 4]>,
    /// 行动后末尾技能与早期段分开内联，超过两项时仍可按需扩容。
    pub late: SmallVec<[SkillHookPlanEntry; 2]>,
}

/// 状态 hook 计划条目；同时携带编译期与运行期两种注册序。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateHookPlanEntry {
    pub owner: EntityIdx,
    pub state_id: Option<StateId>,
    pub legacy_order_key: u32,
    pub priority: SkillPriority,
    pub registration_order: RegistrationOrder,
    pub runtime_registration_order: u64,
}

/// 一个 owner 在某个 hook 下的状态执行计划。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateHookPlan {
    pub hook: ProcMask,
    pub store_generation: u32,
    pub entries: SmallVec<[StateHookPlanEntry; 2]>,
}

/// 调度模式：最小模板轮转，或对齐原版的速度行动条。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ActionSchedulerMode {
    #[default]
    Minimal,
    LegacyStep,
}

/// 阶段调度器：推进行动条选出行动者，并构造技能/状态 hook 计划。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseScheduler {
    action_mode: ActionSchedulerMode,
    ice_release_events: SmallVec<[EntityIdx; 4]>,
}

impl Default for PhaseScheduler {
    /// 默认调度器：最小回合模式，无待处理冰冻解除事件。
    fn default() -> Self {
        Self {
            action_mode: ActionSchedulerMode::Minimal,
            ice_release_events: SmallVec::new(),
        }
    }
}

impl PhaseScheduler {
    pub fn model_ice_events(&self) -> Vec<EntityIdx> { self.ice_release_events.to_vec() }

    /// 从实体推断调度模式并构造调度器。
    pub fn from_entities(entities: &EntityArena) -> Self {
        Self {
            action_mode: Self::infer_action_mode(entities),
            ice_release_events: SmallVec::new(),
        }
    }

    /// 当前调度模式。
    pub const fn action_mode(&self) -> ActionSchedulerMode { self.action_mode }

    /// 手动设置调度模式。
    pub const fn set_action_mode(&mut self, action_mode: ActionSchedulerMode) { self.action_mode = action_mode; }

    /// 按当前实体重新推断调度模式。
    pub fn reset_action_mode_from_entities(&mut self, entities: &EntityArena) {
        self.action_mode = Self::infer_action_mode(entities);
    }

    /// 是否使用对齐原版的速度行动条。
    pub const fn uses_legacy_step_scheduler(&self) -> bool { matches!(self.action_mode, ActionSchedulerMode::LegacyStep) }

    /// 取走本次选行动期间记录的冰冻解除事件。
    pub fn take_ice_release_events(&mut self) -> SmallVec<[EntityIdx; 4]> { std::mem::take(&mut self.ice_release_events) }

    /// 由实体推断调度模式：出现非默认类型且非 Boss/加成的实体时走最小模式，否则按是否存在正速度选 legacy 速度条。
    fn infer_action_mode(entities: &EntityArena) -> ActionSchedulerMode {
        let mut has_speed = false;
        for (_, entity) in entities.iter() {
            if entity.template.kind != crate::runtime::PlayerTemplate::DEFAULT_KIND
                && !entity
                    .runtime
                    .flags
                    .intersects(crate::runtime::PlayerKindFlags::BOSS | crate::runtime::PlayerKindFlags::BOOST)
            {
                return ActionSchedulerMode::Minimal;
            }
            has_speed |= entity.runtime.speed > 0;
        }
        if has_speed {
            ActionSchedulerMode::LegacyStep
        } else {
            ActionSchedulerMode::Minimal
        }
    }

    /// 推进行动条选出本回合行动者与目标；无人达到阈值时返回 None。
    pub fn select_action(
        &mut self,
        world: &mut WorldArena,
        entities: &mut EntityArena,
        randomer: &mut RC4,
    ) -> Option<ActionPlan> {
        self.ice_release_events.clear();
        if !self.uses_legacy_step_scheduler() {
            return self.select_minimal_action(world, entities);
        }

        // legacy 按世界 roster（包含已死亡实体）限制单帧 tick 数；EntityArena 中为 spawn
        // 保留的 ID 空洞不属于 roster，若把它们计入会让新生成的使魔或亡灵提前一帧行动。
        let max_ticks = world.roster_entity_count().max(1) * 4;
        #[cfg(feature = "no_debug")]
        for _ in 0..max_ticks {
            let (actor, should_act, ice_released) = world.next_round_actor_with(|actor| {
                let entity = entities.get_mut(actor).filter(|entity| entity.runtime.alive)?;
                let step_roll = (randomer.next_u8() & 3) as i32;
                let speed = entity.effective_speed();
                let move_points = entity.runtime.move_state.speed_points;
                let (step, ice_released) = entity.states.apply_ice_pre_step(speed * step_roll, move_points);
                if ice_released {
                    // 原版先在解冻钩子内重算状态，再累计本步行动点。
                    entity.apply_runtime_stats_from_template(true);
                }
                entity.runtime.move_state.speed_points += step;
                let should_act = entity.runtime.move_state.speed_points > MOVE_POINT_THRESHOLD;
                if should_act {
                    entity.runtime.move_state.speed_points -= MOVE_POINT_THRESHOLD;
                }
                Some((actor, should_act, ice_released))
            })?;
            if ice_released {
                self.ice_release_events.push(actor);
                if !should_act {
                    return None;
                }
            }
            if should_act {
                let target = world.first_alive_enemy(actor, entities)?;
                let amount = entities.get(actor).map_or(0, |entity| entity.template.attack);
                return Some(ActionPlan { actor, target, amount });
            }
        }
        #[cfg(not(feature = "no_debug"))]
        for _ in 0..max_ticks {
            let actor = world.next_actor(entities)?;
            let step_byte = randomer.next_u8();
            let step_roll = (step_byte & 3) as i32;
            #[cfg(not(feature = "no_debug"))]
            let probe_step = crate::debug::probe_step().is_some_and(|needle| {
                entities.get(actor).is_some_and(|entity| {
                    needle == "*"
                        || needle.strip_prefix("idx:").is_some_and(|idx| idx == actor.0.to_string())
                        || needle.strip_prefix("id:").is_some_and(|id| id == entity.template.id.to_string())
                        || entity.template.name.contains(needle)
                        || entity.template.display_name.contains(needle)
                })
            });
            let (should_act, ice_released) = {
                let actor = entities
                    .get_mut(actor)
                    .unwrap_or_else(|| panic!("runtime scheduler selected unknown actor: {}", actor.0));
                let effective_speed = actor.effective_speed();
                let move_points_before = actor.runtime.move_state.speed_points;
                let (step, ice_released) = actor.states.apply_ice_pre_step(effective_speed * step_roll, move_points_before);
                if ice_released {
                    actor.apply_runtime_stats_from_template(true);
                }
                actor.runtime.move_state.speed_points += step;
                if actor.runtime.move_state.speed_points > MOVE_POINT_THRESHOLD {
                    actor.runtime.move_state.speed_points -= MOVE_POINT_THRESHOLD;
                    #[cfg(not(feature = "no_debug"))]
                    if probe_step {
                        eprintln!(
                            "[step_probe:runtime] actor={} name={} byte={} roll={} speed={} effective_speed={} \
                             step={} move_before={} move_after={} acted=true",
                            actor.template.id,
                            actor.template.name,
                            step_byte,
                            step_roll,
                            actor.runtime.speed,
                            effective_speed,
                            step,
                            move_points_before,
                            actor.runtime.move_state.speed_points,
                        );
                    }
                    (true, ice_released)
                } else {
                    #[cfg(not(feature = "no_debug"))]
                    if probe_step {
                        eprintln!(
                            "[step_probe:runtime] actor={} name={} byte={} roll={} speed={} effective_speed={} \
                             step={} move_before={} move_after={} acted=false",
                            actor.template.id,
                            actor.template.name,
                            step_byte,
                            step_roll,
                            actor.runtime.speed,
                            effective_speed,
                            step,
                            move_points_before,
                            actor.runtime.move_state.speed_points,
                        );
                    }
                    (false, ice_released)
                }
            };
            if ice_released {
                self.ice_release_events.push(actor);
                if !should_act {
                    return None;
                }
            }
            if !should_act {
                continue;
            }

            let target = world.first_alive_enemy(actor, entities)?;
            let amount = entities.get(actor).map_or(0, |entity| entity.template.attack);
            return Some(ActionPlan { actor, target, amount });
        }
        None
    }

    /// 最小模式选行动：下一行动者攻击其首个存活敌人。
    pub fn select_minimal_action(&mut self, world: &mut WorldArena, entities: &EntityArena) -> Option<ActionPlan> {
        let actor = world.next_actor(entities)?;
        let target = world.first_alive_enemy(actor, entities)?;
        let amount = entities.get(actor).map_or(0, |entity| entity.template.attack);
        Some(ActionPlan { actor, target, amount })
    }

    /// 构造 owner 在指定 hook 下的技能执行计划。
    pub fn skill_hook_plan(
        &self,
        entities: &EntityArena,
        registry: &ExtensionRegistry,
        owner: EntityIdx,
        hook: ProcMask,
    ) -> SkillHookPlan {
        let entity = entities
            .get(owner)
            .unwrap_or_else(|| panic!("unknown runtime skill owner entity: {}", owner.0));
        self.skill_hook_plan_for_loadout(&entity.template.skills, registry, owner, hook)
    }

    pub fn skill_hook_plan_for_loadout(
        &self,
        skills: &SkillLoadout,
        registry: &ExtensionRegistry,
        owner: EntityIdx,
        hook: ProcMask,
    ) -> SkillHookPlan {
        let mut entries = SmallVec::<[SkillHookPlanEntry; 4]>::new();
        if let Some(cached_entries) = skills.cached_hook_entries(hook) {
            for entry in cached_entries {
                if skills.level_at(entry.fixed_lane) != Some(0) {
                    entries.push(SkillHookPlanEntry {
                        owner,
                        skill_id: entry.skill_id,
                        target_policy: entry.target_policy,
                        priority: entry.priority,
                        active_order: entry.active_order,
                        fixed_lane: entry.fixed_lane,
                        registration_order: entry.registration_order,
                    });
                }
            }
        } else {
            for (active_order, &fixed_lane) in skills.active_order().iter().enumerate() {
                if skills.level_at(fixed_lane) == Some(0) {
                    continue;
                }
                let skill_id = skills
                    .skills()
                    .get(fixed_lane)
                    .unwrap_or_else(|| panic!("runtime skill active order references missing lane: {fixed_lane}"));
                let spec = registry
                    .skill(*skill_id)
                    .unwrap_or_else(|| panic!("unknown runtime skill id in loadout: {}", skill_id.0));
                if spec.hook_mask.intersects(hook) {
                    entries.push(SkillHookPlanEntry {
                        owner,
                        skill_id: spec.id,
                        target_policy: spec.target_policy,
                        priority: spec.priority,
                        active_order,
                        fixed_lane,
                        registration_order: spec.registration_order,
                    });
                }
            }
            entries.sort_by_key(|entry| (entry.priority, entry.active_order, entry.registration_order));
        }
        SkillHookPlan {
            owner,
            hook,
            loadout_len: skills.len(),
            entries,
        }
    }

    /// 按 POST_ACTION 的 early / late 阶段过滤技能计划。
    pub fn skill_post_action_hook_plan(
        &self,
        entities: &EntityArena,
        registry: &ExtensionRegistry,
        owner: EntityIdx,
        phase: SkillPostActionPhase,
    ) -> SkillHookPlan {
        let mut plan = self.skill_hook_plan(entities, registry, owner, ProcMask::POST_ACTION);
        plan.entries.retain(|entry| {
            registry
                .skill(entry.skill_id)
                .map(|spec| spec.post_action_phase == phase)
                .unwrap_or(false)
        });
        if phase == SkillPostActionPhase::Early {
            let deferred_lanes = entities
                .get(owner)
                .unwrap_or_else(|| panic!("unknown runtime post-action skill owner entity: {}", owner.0))
                .template
                .skills
                .post_action_after_states();
            plan.entries
                .retain(|entry| !deferred_lanes.iter().any(|(_, fixed_lane)| *fixed_lane == entry.fixed_lane));
        }
        plan
    }

    /// 一次扫描 POST_ACTION 缓存并分成即时、状态间穿插和末尾三段。
    /// 调用方用 generation 判断执行期间是否发生技能写入，再按需重建。
    pub fn skill_post_action_plans(
        &self,
        skills: &SkillLoadout,
        registry: &ExtensionRegistry,
        owner: EntityIdx,
    ) -> SkillPostActionPlans {
        let deferred_lanes = skills.post_action_after_states();
        let mut deferred = SmallVec::<[(u64, SkillHookPlanEntry); 4]>::new();
        let mut early_entries = SmallVec::<[SkillHookPlanEntry; 2]>::new();
        let mut late_entries = SmallVec::<[SkillHookPlanEntry; 2]>::new();

        if let Some(cached_entries) = skills.cached_hook_entries(ProcMask::POST_ACTION) {
            for cached in cached_entries {
                if skills.level_at(cached.fixed_lane) == Some(0) {
                    continue;
                }
                let entry = SkillHookPlanEntry {
                    owner,
                    skill_id: cached.skill_id,
                    target_policy: cached.target_policy,
                    priority: cached.priority,
                    active_order: cached.active_order,
                    fixed_lane: cached.fixed_lane,
                    registration_order: cached.registration_order,
                };
                match cached.post_action_phase {
                    SkillPostActionPhase::Early => {
                        if let Some((cursor, _)) = deferred_lanes.iter().find(|(_, lane)| *lane == cached.fixed_lane) {
                            deferred.push((*cursor, entry));
                        } else {
                            early_entries.push(entry);
                        }
                    }
                    SkillPostActionPhase::Late => late_entries.push(entry),
                }
            }
        } else {
            let plan = self.skill_hook_plan_for_loadout(skills, registry, owner, ProcMask::POST_ACTION);
            for (cursor, fixed_lane) in deferred_lanes {
                let Some(entry) = plan.entries.iter().find(|entry| entry.fixed_lane == *fixed_lane).copied() else {
                    continue;
                };
                let spec = registry
                    .skill(entry.skill_id)
                    .unwrap_or_else(|| panic!("unknown runtime post-action skill id: {}", entry.skill_id.0));
                if spec.post_action_phase == SkillPostActionPhase::Early {
                    deferred.push((*cursor, entry));
                }
            }
            for entry in plan.entries {
                let phase = registry
                    .skill(entry.skill_id)
                    .map(|spec| spec.post_action_phase)
                    .unwrap_or(SkillPostActionPhase::Early);
                match phase {
                    SkillPostActionPhase::Early
                        if !deferred_lanes.iter().any(|(_, fixed_lane)| *fixed_lane == entry.fixed_lane) =>
                    {
                        early_entries.push(entry);
                    }
                    SkillPostActionPhase::Early => {}
                    SkillPostActionPhase::Late => late_entries.push(entry),
                }
            }
        }

        SkillPostActionPlans {
            generation: skills.hook_generation(),
            early: early_entries,
            deferred,
            late: late_entries,
        }
    }

    /// 取延后到状态之后执行的 POST_ACTION 条目，按状态游标排序。
    pub fn deferred_skill_post_action_entries(
        &self,
        entities: &EntityArena,
        registry: &ExtensionRegistry,
        owner: EntityIdx,
    ) -> SmallVec<[(u64, SkillHookPlanEntry); 4]> {
        let entity = entities
            .get(owner)
            .unwrap_or_else(|| panic!("unknown runtime deferred post-action skill owner entity: {}", owner.0));
        let plan = self.skill_hook_plan(entities, registry, owner, ProcMask::POST_ACTION);
        entity
            .template
            .skills
            .post_action_after_states()
            .iter()
            .filter_map(|(cursor, fixed_lane)| {
                plan.entries
                    .iter()
                    .find(|entry| {
                        entry.fixed_lane == *fixed_lane
                            && registry
                                .skill(entry.skill_id)
                                .is_some_and(|spec| spec.post_action_phase == SkillPostActionPhase::Early)
                    })
                    .copied()
                    .map(|entry| (*cursor, entry))
            })
            .collect()
    }

    /// 构造 owner 在指定 hook 下的状态执行计划。
    pub fn state_hook_plan(&self, entities: &EntityArena, owner: EntityIdx, hook: ProcMask) -> StateHookPlan {
        let entity = entities
            .get(owner)
            .unwrap_or_else(|| panic!("unknown runtime hook owner entity: {}", owner.0));
        if !entity.states.hook_mask().intersects(hook) {
            return StateHookPlan {
                hook,
                store_generation: entity.states.generation(),
                entries: SmallVec::new(),
            };
        }
        let mut entries = SmallVec::<[StateHookPlanEntry; 2]>::new();
        // 运行期注册顺序与 entries 同下标，直接 zip 取值；
        // 逐条按 legacy_order_key 反查会让整个 plan 构造退化成 O(n^2)。
        let runtime_orders = entity.states.runtime_registration_orders();
        debug_assert_eq!(runtime_orders.len(), entity.states.entries().len());
        for (entry, &runtime_registration_order) in entity.states.entries().iter().zip(runtime_orders) {
            if !entry.hook_mask.intersects(hook) {
                continue;
            }
            let planned = StateHookPlanEntry {
                owner,
                state_id: entry.extension_state_id,
                legacy_order_key: entry.legacy_order_key,
                priority: entry.plan_priority_for_hook(hook, entities),
                registration_order: entry.registration_order,
                runtime_registration_order,
            };
            if matches!(hook, ProcMask::PRE_ACTION | ProcMask::POST_ACTION)
                && entry.is_boss_infection(entities)
                && let crate::runtime::StatePayload::CovidInfection { entries: strains, .. } = &entry.payload
            {
                entries.extend(strains.iter().map(|strain| StateHookPlanEntry {
                    runtime_registration_order: strain.registration_order.unwrap_or(runtime_registration_order),
                    ..planned
                }));
            } else {
                entries.push(planned);
            }
        }
        entries.sort_by_key(|entry| (entry.priority, entry.registration_order, entry.runtime_registration_order));
        StateHookPlan {
            hook,
            store_generation: entity.states.generation(),
            entries,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{
        ExtensionRegistryBuilder, PlayerTemplate, PreparedCombatTemplate, StateEntry, StatePayload, TargetPolicy,
    };

    #[test]
    fn scheduler_selects_next_actor_first_alive_enemy_and_amount() {
        let entities = EntityArena::from_templates(PreparedCombatTemplate::minimal_1v1(10, 10, 4).players);
        let mut world = WorldArena::from_entities(&entities);
        let mut scheduler = PhaseScheduler::default();

        assert_eq!(
            scheduler.select_minimal_action(&mut world, &entities),
            Some(ActionPlan {
                actor: EntityIdx(0),
                target: EntityIdx(1),
                amount: 4,
            })
        );
    }

    #[test]
    fn scheduler_skips_dead_actor_in_round_order() {
        let mut entities = EntityArena::from_templates(vec![
            PlayerTemplate::new(1, "dead-left", 0, 10, 4),
            PlayerTemplate::new(2, "right", 1, 10, 4),
            PlayerTemplate::new(3, "alive-left", 0, 10, 4),
        ]);
        entities.get_mut(EntityIdx(0)).unwrap().runtime.alive = false;
        let mut world = WorldArena::from_entities(&entities);
        let mut scheduler = PhaseScheduler::default();

        assert_eq!(
            scheduler.select_minimal_action(&mut world, &entities),
            Some(ActionPlan {
                actor: EntityIdx(1),
                target: EntityIdx(2),
                amount: 4,
            })
        );
    }

    #[test]
    fn scheduler_returns_none_without_alive_enemy() {
        let mut entities = EntityArena::from_templates(PreparedCombatTemplate::minimal_1v1(10, 10, 4).players);
        entities.get_mut(EntityIdx(1)).unwrap().runtime.alive = false;
        let mut world = WorldArena::from_entities(&entities);
        let mut scheduler = PhaseScheduler::default();

        assert_eq!(scheduler.select_minimal_action(&mut world, &entities), None);
    }

    #[test]
    fn scheduler_advances_legacy_speed_points_until_an_actor_is_ready() {
        let mut entities = EntityArena::from_templates(vec![
            PlayerTemplate::new(1, "left", 0, 10, 4).with_speed(1),
            PlayerTemplate::new(2, "right", 1, 10, 5)
                .with_speed(1)
                .with_speed_points(MOVE_POINT_THRESHOLD + 1),
        ]);
        let mut world = WorldArena::from_entities(&entities);
        let mut scheduler = PhaseScheduler::from_entities(&entities);
        let mut randomer = RC4::new(b"runtime-scheduler", 1);

        assert_eq!(
            scheduler.select_action(&mut world, &mut entities, &mut randomer),
            Some(ActionPlan {
                actor: EntityIdx(1),
                target: EntityIdx(0),
                amount: 5,
            })
        );
        assert!(entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points <= 3);
        assert!(entities.get(EntityIdx(1)).unwrap().runtime.move_state.speed_points <= 4);
    }

    #[test]
    fn scheduler_builds_skill_hook_plan_from_loadout_and_registry_order() {
        let mut builder = ExtensionRegistryBuilder::default();
        let late = builder
            .register_skill_with_hooks(
                "custom",
                "late",
                "custom.late",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(10),
            )
            .expect("late skill should register");
        let early = builder
            .register_skill_with_hooks(
                "custom",
                "early",
                "custom.early",
                ProcMask::PRE_ACTION | ProcMask::POST_ACTION,
                TargetPolicy::Ally,
                SkillPriority(1),
            )
            .expect("early skill should register");
        let unrelated = builder
            .register_skill_with_hooks(
                "custom",
                "unrelated",
                "custom.unrelated",
                ProcMask::POST_DAMAGE,
                TargetPolicy::Any,
                SkillPriority(0),
            )
            .expect("unrelated skill should register");
        let registry = builder.build();
        let entities = EntityArena::from_templates_with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 4).with_skills([late, unrelated, early])],
            &registry,
        );
        let scheduler = PhaseScheduler::default();

        let plan = scheduler.skill_hook_plan(&entities, &registry, EntityIdx(0), ProcMask::PRE_ACTION);

        assert_eq!(plan.owner, EntityIdx(0));
        assert_eq!(plan.hook, ProcMask::PRE_ACTION);
        assert_eq!(plan.loadout_len, 3);
        assert_eq!(
            plan.entries.as_slice(),
            &[
                SkillHookPlanEntry {
                    owner: EntityIdx(0),
                    skill_id: early,
                    target_policy: TargetPolicy::Ally,
                    priority: SkillPriority(1),
                    active_order: 2,
                    fixed_lane: 2,
                    registration_order: RegistrationOrder(1),
                },
                SkillHookPlanEntry {
                    owner: EntityIdx(0),
                    skill_id: late,
                    target_policy: TargetPolicy::Enemy,
                    priority: SkillPriority(10),
                    active_order: 0,
                    fixed_lane: 0,
                    registration_order: RegistrationOrder(0),
                },
            ]
        );
    }

    #[test]
    fn scheduler_uses_active_order_for_same_priority_skill_hooks() {
        let mut builder = ExtensionRegistryBuilder::default();
        let first_lane = builder
            .register_skill_with_hooks(
                "custom",
                "first-lane",
                "custom.first_lane",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(1),
            )
            .expect("first lane skill should register");
        let second_lane = builder
            .register_skill_with_hooks(
                "custom",
                "second-lane",
                "custom.second_lane",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(1),
            )
            .expect("second lane skill should register");
        let third_lane = builder
            .register_skill_with_hooks(
                "custom",
                "third-lane",
                "custom.third_lane",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(1),
            )
            .expect("third lane skill should register");
        let registry = builder.build();
        let loadout =
            crate::runtime::SkillLoadout::from_skills([first_lane, second_lane, third_lane]).with_active_order([2, 0, 1]);
        let entities = EntityArena::from_templates_with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 4).with_skill_loadout(loadout)],
            &registry,
        );
        let scheduler = PhaseScheduler::default();

        let plan = scheduler.skill_hook_plan(&entities, &registry, EntityIdx(0), ProcMask::PRE_ACTION);

        assert_eq!(
            plan.entries.iter().map(|entry| entry.skill_id).collect::<Vec<_>>(),
            vec![third_lane, first_lane, second_lane]
        );
        assert_eq!(
            plan.entries.iter().map(|entry| entry.fixed_lane).collect::<Vec<_>>(),
            vec![2, 0, 1]
        );
    }

    #[test]
    fn scheduler_builds_state_hook_plan_from_current_store_generation() {
        let mut entities = EntityArena::from_templates(vec![PlayerTemplate::new(1, "left", 0, 10, 4)]);
        let late = StateEntry {
            legacy_order_key: 11,
            extension_state_id: Some(StateId(1)),
            hook_mask: ProcMask::PRE_ACTION,
            priority: SkillPriority(10),
            registration_order: RegistrationOrder(1),
            payload: StatePayload::None,
        };
        let early = StateEntry {
            legacy_order_key: 22,
            extension_state_id: Some(StateId(2)),
            hook_mask: ProcMask::PRE_ACTION | ProcMask::POST_ACTION,
            priority: SkillPriority(1),
            registration_order: RegistrationOrder(2),
            payload: StatePayload::None,
        };
        let unrelated = StateEntry {
            legacy_order_key: 33,
            extension_state_id: Some(StateId(3)),
            hook_mask: ProcMask::POST_DAMAGE,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        };
        let owner = entities.get_mut(EntityIdx(0)).unwrap();
        owner.states.add_entry(late);
        owner.states.add_entry(early);
        owner.states.add_entry(unrelated);
        let scheduler = PhaseScheduler::default();

        let plan = scheduler.state_hook_plan(&entities, EntityIdx(0), ProcMask::PRE_ACTION);

        assert_eq!(plan.hook, ProcMask::PRE_ACTION);
        assert_eq!(plan.store_generation, 3);
        assert_eq!(
            plan.entries,
            SmallVec::<[StateHookPlanEntry; 2]>::from_slice(&[
                StateHookPlanEntry {
                    owner: EntityIdx(0),
                    state_id: Some(StateId(2)),
                    legacy_order_key: 22,
                    priority: SkillPriority(1),
                    registration_order: RegistrationOrder(2),
                    runtime_registration_order: 1,
                },
                StateHookPlanEntry {
                    owner: EntityIdx(0),
                    state_id: Some(StateId(1)),
                    legacy_order_key: 11,
                    priority: SkillPriority(10),
                    registration_order: RegistrationOrder(1),
                    runtime_registration_order: 0,
                },
            ])
        );
    }

    #[test]
    fn scheduler_rebuilds_state_hook_plan_after_state_changes() {
        let mut entities = EntityArena::from_templates(vec![PlayerTemplate::new(1, "left", 0, 10, 4)]);
        let first = StateEntry {
            legacy_order_key: 11,
            extension_state_id: Some(StateId(1)),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(10),
            registration_order: RegistrationOrder(1),
            payload: StatePayload::None,
        };
        let second = StateEntry {
            legacy_order_key: 22,
            extension_state_id: Some(StateId(2)),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(1),
            registration_order: RegistrationOrder(2),
            payload: StatePayload::None,
        };
        let scheduler = PhaseScheduler::default();
        entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(first);

        let before = scheduler.state_hook_plan(&entities, EntityIdx(0), ProcMask::POST_ACTION);
        entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(second);
        entities.get_mut(EntityIdx(0)).unwrap().states.clear_legacy_key(11);
        let after = scheduler.state_hook_plan(&entities, EntityIdx(0), ProcMask::POST_ACTION);

        assert_eq!(before.store_generation, 1);
        assert_eq!(
            before.entries.iter().map(|entry| entry.legacy_order_key).collect::<Vec<_>>(),
            vec![11]
        );
        assert_eq!(after.store_generation, 3);
        assert_eq!(
            after.entries.iter().map(|entry| entry.legacy_order_key).collect::<Vec<_>>(),
            vec![22]
        );
    }
}

use crate::runtime_v2::entity::{EntityArena, EntityIdx};
use crate::runtime_v2::extension::{
    ExtensionRegistry, ProcMask, RegistrationOrder, SkillId, SkillPriority, StateId, TargetPolicy,
};
use crate::runtime_v2::world::WorldArena;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionPlan {
    pub actor: EntityIdx,
    pub target: EntityIdx,
    pub amount: i32,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillHookPlan {
    pub owner: EntityIdx,
    pub hook: ProcMask,
    pub loadout_len: usize,
    pub entries: Vec<SkillHookPlanEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateHookPlanEntry {
    pub owner: EntityIdx,
    pub state_id: Option<StateId>,
    pub legacy_order_key: u32,
    pub priority: SkillPriority,
    pub registration_order: RegistrationOrder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateHookPlan {
    pub hook: ProcMask,
    pub store_generation: u32,
    pub entries: Vec<StateHookPlanEntry>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PhaseScheduler;

impl PhaseScheduler {
    pub fn select_minimal_action(&mut self, world: &mut WorldArena, entities: &EntityArena) -> Option<ActionPlan> {
        let actor = world.next_actor(entities)?;
        let target = world.first_alive_enemy(actor, entities)?;
        let amount = entities.get(actor).map_or(0, |entity| entity.template.attack);
        Some(ActionPlan { actor, target, amount })
    }

    pub fn skill_hook_plan(
        &self,
        entities: &EntityArena,
        registry: &ExtensionRegistry,
        owner: EntityIdx,
        hook: ProcMask,
    ) -> SkillHookPlan {
        let entity = entities
            .get(owner)
            .unwrap_or_else(|| panic!("unknown runtime_v2 skill owner entity: {}", owner.0));
        let mut entries = entity
            .template
            .skills
            .active_order()
            .iter()
            .enumerate()
            .filter_map(|lane| {
                let (active_order, lane) = lane;
                let skill_id = entity
                    .template
                    .skills
                    .skills()
                    .get(*lane)
                    .unwrap_or_else(|| panic!("runtime_v2 skill active order references missing lane: {lane}"));
                let spec = registry
                    .skill(*skill_id)
                    .unwrap_or_else(|| panic!("unknown runtime_v2 skill id in loadout: {}", skill_id.0));
                spec.hook_mask.intersects(hook).then_some(SkillHookPlanEntry {
                    owner,
                    skill_id: spec.id,
                    target_policy: spec.target_policy,
                    priority: spec.priority,
                    active_order,
                    fixed_lane: *lane,
                    registration_order: spec.registration_order,
                })
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| (entry.priority, entry.active_order, entry.registration_order));
        SkillHookPlan {
            owner,
            hook,
            loadout_len: entity.template.skills.len(),
            entries,
        }
    }

    pub fn state_hook_plan(&self, entities: &EntityArena, owner: EntityIdx, hook: ProcMask) -> StateHookPlan {
        let entity = entities
            .get(owner)
            .unwrap_or_else(|| panic!("unknown runtime_v2 hook owner entity: {}", owner.0));
        let entries = entity
            .states
            .entries_in_hook_order_for(hook)
            .into_iter()
            .map(|entry| StateHookPlanEntry {
                owner,
                state_id: entry.extension_state_id,
                legacy_order_key: entry.legacy_order_key,
                priority: entry.priority_for_hook(hook),
                registration_order: entry.registration_order,
            })
            .collect();
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
    use crate::runtime_v2::{
        ExtensionRegistryBuilder, PlayerTemplate, PreparedCombatTemplate, StateEntry, StatePayload, TargetPolicy,
    };

    #[test]
    fn scheduler_selects_next_actor_first_alive_enemy_and_amount() {
        let entities = EntityArena::from_templates(PreparedCombatTemplate::minimal_1v1(10, 10, 4).players);
        let mut world = WorldArena::from_entities(&entities);
        let mut scheduler = PhaseScheduler;

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
        let mut scheduler = PhaseScheduler;

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
        let mut scheduler = PhaseScheduler;

        assert_eq!(scheduler.select_minimal_action(&mut world, &entities), None);
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
        let scheduler = PhaseScheduler;

        let plan = scheduler.skill_hook_plan(&entities, &registry, EntityIdx(0), ProcMask::PRE_ACTION);

        assert_eq!(plan.owner, EntityIdx(0));
        assert_eq!(plan.hook, ProcMask::PRE_ACTION);
        assert_eq!(plan.loadout_len, 3);
        assert_eq!(
            plan.entries,
            vec![
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
            crate::runtime_v2::SkillLoadout::from_skills([first_lane, second_lane, third_lane]).with_active_order([2, 0, 1]);
        let entities = EntityArena::from_templates_with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 4).with_skill_loadout(loadout)],
            &registry,
        );
        let scheduler = PhaseScheduler;

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
        let scheduler = PhaseScheduler;

        let plan = scheduler.state_hook_plan(&entities, EntityIdx(0), ProcMask::PRE_ACTION);

        assert_eq!(plan.hook, ProcMask::PRE_ACTION);
        assert_eq!(plan.store_generation, 3);
        assert_eq!(
            plan.entries,
            vec![
                StateHookPlanEntry {
                    owner: EntityIdx(0),
                    state_id: Some(StateId(2)),
                    legacy_order_key: 22,
                    priority: SkillPriority(1),
                    registration_order: RegistrationOrder(2),
                },
                StateHookPlanEntry {
                    owner: EntityIdx(0),
                    state_id: Some(StateId(1)),
                    legacy_order_key: 11,
                    priority: SkillPriority(10),
                    registration_order: RegistrationOrder(1),
                },
            ]
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
        let scheduler = PhaseScheduler;
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

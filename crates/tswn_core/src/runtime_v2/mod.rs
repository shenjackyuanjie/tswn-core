pub mod effect;
pub mod entity;
pub mod extension;
pub mod oracle;
pub mod scheduler;
pub mod scratch;
pub mod slot;
#[cfg(not(feature = "no_debug"))]
pub mod trace;
pub mod world;

use crate::engine::update::RunUpdates;
use crate::player::PlrId;
use crate::rc4::RC4;

pub use effect::{
    CoreReplayEvent, CoreShowEvent, CustomEffect, CustomEffectPayload, EffectContext, EffectContextError, EffectHandlerFn,
    EffectHandlers, EffectQueue, QueuedEffect, RenderedReplay, RenderedShow, ReplayRendererFn, ReplayRenderers, RuntimeFrame,
    ShowRendererFn, ShowRenderers, SkillContext, SkillHandlerFn, SkillHandlers, StateContext, StateHandlerFn, StateHandlers,
};
pub use entity::{
    EntityArena, EntityIdx, EntityRecord, MoveState, PlayerRuntime, PlayerTemplate, SkillLoadout, StateEntry, StateStore,
};
pub use extension::{
    BattleSlotId, BattleSlotSpec, DamageSharePolicy, EffectHandlerId, EffectHandlerSpec, EntitySlotId, EntitySlotSpec,
    ExtensionCapability, ExtensionError, ExtensionRegistry, ExtensionRegistryBuilder, ExtensionVersion, InstalledExtensionSpec,
    MergePolicy, OwnerResolutionPolicy, PlayerKindFlags, PlayerKindId, PlayerKindPolicies, PlayerKindSpec, ProcMask,
    RegistrationOrder, ReplayRendererId, ReplayRendererSpec, ShowRendererId, ShowRendererSpec, SkillId, SkillPriority, SkillSpec,
    StateId, StateSpec, TargetPolicy, TemplateSlotId, TemplateSlotSpec, TswnExtension,
};
pub use oracle::{NormalizedOutcome, NormalizedUpdateFrame, StrictDiff, strict_diff};
pub use scheduler::{ActionPlan, PhaseScheduler, SkillHookPlan, SkillHookPlanEntry, StateHookPlan, StateHookPlanEntry};
pub use scratch::BattleScratch;
pub use slot::{BattleSlotStorage, EntitySlotStorage, SlotError, SlotValue, TemplateSlotStorage};
#[cfg(not(feature = "no_debug"))]
pub use trace::{RngCheckpoint, RuntimeTrace, TraceAction, TraceFrame};
pub use world::WorldArena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCombatTemplate {
    pub players: Vec<PlayerTemplate>,
    pub registry: ExtensionRegistry,
    pub slots: TemplateSlotStorage,
}

impl PreparedCombatTemplate {
    pub fn new(players: Vec<PlayerTemplate>) -> Self { Self::with_registry(players, ExtensionRegistry::default()) }

    pub fn with_registry(players: Vec<PlayerTemplate>, registry: ExtensionRegistry) -> Self {
        let slots = TemplateSlotStorage::from_registry(&registry);
        Self {
            players,
            registry,
            slots,
        }
    }

    pub fn minimal_1v1(left_hp: i32, right_hp: i32, attack: i32) -> Self {
        Self::new(vec![
            PlayerTemplate::new(1, "left", 0, left_hp, attack),
            PlayerTemplate::new(2, "right", 1, right_hp, attack),
        ])
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeV2Runner {
    runtime: CombatRuntime,
}

impl RuntimeV2Runner {
    pub fn from_template(template: PreparedCombatTemplate) -> Self {
        Self {
            runtime: CombatRuntime::from_template(template),
        }
    }

    pub fn from_bed2_roster(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
    ) -> Result<Self, CustomBed2RosterImportError> {
        let template = CustomBed2Import::roster_into_prepared_template(raw_groups, registry, kind, summon_skill)?;
        Ok(Self::from_template(template))
    }

    pub fn from_mixed_roster(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
    ) -> Result<Self, CustomMixedRosterImportError> {
        let template = CustomBed2Import::mixed_roster_into_prepared_template(raw_groups, registry, bed2_kind, bed2_summon_skill)?;
        Ok(Self::from_template(template))
    }

    pub fn runtime(&self) -> &CombatRuntime { &self.runtime }

    pub fn runtime_mut(&mut self) -> &mut CombatRuntime { &mut self.runtime }

    pub fn run_round(&mut self) -> RoundOutcome { self.runtime.run_minimal_round() }

    pub fn run_round_normalized(&mut self) -> NormalizedOutcome {
        let outcome = self.run_round();
        NormalizedOutcome::from_runtime(&self.runtime, &outcome)
    }
}

pub const DEFAULT_BED2_HP: i32 = 3000;
pub const DEFAULT_BED2_DEFENSE: i32 = 99;
pub const DEFAULT_BED2_RESISTANCE: i32 = 99;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomBed2Import {
    pub name: String,
    pub team: Option<String>,
    pub hp: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomBed2RosterImportError {
    pub team_index: usize,
    pub player_index: usize,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomMixedRosterImportError {
    pub team_index: usize,
    pub player_index: usize,
    pub raw: String,
    pub message: String,
}

impl CustomBed2Import {
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }

        let (name, team, plus_rest, team_marker_hp) = if let Some((name, team_and_rest)) = raw.split_once('@') {
            let (team_part, plus_rest) = team_and_rest.split_once('+').unwrap_or((team_and_rest, ""));
            let (team, hp) = Self::split_bed2_team_marker(team_part.trim());
            (name.trim(), team, plus_rest, hp)
        } else if let Some((name, plus_rest)) = raw.split_once('+') {
            (name.trim(), None, plus_rest, None)
        } else {
            return None;
        };

        let hp = Self::parse_bed2_plus_segments(plus_rest).or(team_marker_hp)?;
        Some(Self {
            name: name.to_owned(),
            team,
            hp,
        })
    }

    pub fn parse_player_facade_raw(raw: &str) -> Option<Self> {
        let marker_import = Self::parse(raw)?;
        let id_name = crate::player::Player::raw_namerena_to_idname(raw.trim());
        let (name, team, facade_hp) = Self::parse_facade_id_name(&id_name);
        Some(Self {
            name,
            team,
            hp: facade_hp.unwrap_or(marker_import.hp),
        })
    }

    pub fn into_player_template(self, id: PlrId, kind: PlayerKindId, team: usize, summon_skill: SkillId) -> PlayerTemplate {
        PlayerTemplate::with_kind(id, self.name, kind, team, self.hp, 0)
            .with_def_res(DEFAULT_BED2_DEFENSE, DEFAULT_BED2_RESISTANCE)
            .with_skills([summon_skill])
    }

    pub fn roster_into_prepared_template(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
    ) -> Result<PreparedCombatTemplate, CustomBed2RosterImportError> {
        let players = Self::roster_into_player_templates(raw_groups, kind, summon_skill)?;
        Ok(PreparedCombatTemplate::with_registry(players, registry))
    }

    pub fn roster_into_player_templates(
        raw_groups: &[Vec<String>],
        kind: PlayerKindId,
        summon_skill: SkillId,
    ) -> Result<Vec<PlayerTemplate>, CustomBed2RosterImportError> {
        let mut players = Vec::new();
        let mut next_id = 1;
        for (team_index, group) in raw_groups.iter().enumerate() {
            for (player_index, raw) in group.iter().enumerate() {
                if crate::player::Player::check_is_seed(raw.trim()) {
                    continue;
                }
                let Some(import) = Self::parse_player_facade_raw(raw) else {
                    return Err(CustomBed2RosterImportError {
                        team_index,
                        player_index,
                        raw: raw.clone(),
                    });
                };
                players.push(import.into_player_template(next_id, kind, team_index, summon_skill));
                next_id += 1;
            }
        }
        Ok(players)
    }

    pub fn mixed_roster_into_prepared_template(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
    ) -> Result<PreparedCombatTemplate, CustomMixedRosterImportError> {
        let players = Self::mixed_roster_into_player_templates(raw_groups, bed2_kind, bed2_summon_skill)?;
        Ok(PreparedCombatTemplate::with_registry(players, registry))
    }

    pub fn mixed_roster_into_player_templates(
        raw_groups: &[Vec<String>],
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
    ) -> Result<Vec<PlayerTemplate>, CustomMixedRosterImportError> {
        let storage = crate::engine::storage::Storage::new_arc();
        let mut players = Vec::new();
        let mut next_id = 1;
        for (team_index, group) in raw_groups.iter().enumerate() {
            for (player_index, raw) in group.iter().enumerate() {
                let raw_trimmed = raw.trim();
                if crate::player::Player::check_is_seed(raw_trimmed) {
                    continue;
                }

                let template = if let Some(import) = Self::parse_player_facade_raw(raw_trimmed) {
                    import.into_player_template(next_id, bed2_kind, team_index, bed2_summon_skill)
                } else {
                    let mut player =
                        crate::player::Player::new_from_namerena_raw(raw.clone(), storage.clone()).map_err(|error| {
                            CustomMixedRosterImportError {
                                team_index,
                                player_index,
                                raw: raw.clone(),
                                message: format!("{error:?}"),
                            }
                        })?;
                    player.build();
                    let status = player.get_status();
                    if status.max_hp <= 0 || status.attack < 0 || status.defense < 0 || status.resistance < 0 {
                        return Err(CustomMixedRosterImportError {
                            team_index,
                            player_index,
                            raw: raw.clone(),
                            message: format!(
                                "legacy player facade produced unsupported status max_hp={} attack={} defense={} resistance={}",
                                status.max_hp, status.attack, status.defense, status.resistance
                            ),
                        });
                    }
                    PlayerTemplate::new(next_id, player.id_name(), team_index, status.max_hp, status.attack)
                        .with_def_res(status.defense, status.resistance)
                };
                players.push(template);
                next_id += 1;
            }
        }
        Ok(players)
    }

    fn parse_facade_id_name(id_name: &str) -> (String, Option<String>, Option<i32>) {
        let (base, plus_rest) = id_name.split_once('+').unwrap_or((id_name, ""));
        let (name, team, team_marker_hp) = Self::split_facade_name_team(base);
        let plus_marker_hp = Self::parse_bed2_plus_segments(plus_rest);
        (name, team, plus_marker_hp.or(team_marker_hp))
    }

    fn split_facade_name_team(raw: &str) -> (String, Option<String>, Option<i32>) {
        if let Some((name, team)) = raw.split_once('@') {
            let (team, hp) = Self::split_bed2_team_marker(team.trim());
            (name.trim().to_owned(), team, hp)
        } else {
            (raw.trim().to_owned(), None, None)
        }
    }

    fn split_bed2_team_marker(team: &str) -> (Option<String>, Option<i32>) {
        if team == "bed2" {
            return (None, Some(DEFAULT_BED2_HP));
        }
        match team.rsplit_once('@') {
            Some((team, "bed2")) if !team.is_empty() => (Some(team.to_owned()), Some(DEFAULT_BED2_HP)),
            _ if team.is_empty() => (None, None),
            _ => (Some(team.to_owned()), None),
        }
    }

    fn parse_bed2_plus_segments(raw: &str) -> Option<i32> { raw.split('+').filter_map(Self::parse_bed2_plus_marker).last() }

    fn parse_bed2_plus_marker(segment: &str) -> Option<i32> {
        let rest = segment.trim().strip_prefix("bed2[")?;
        let hp = rest.strip_suffix(']')?.trim().parse::<i32>().ok()?;
        (hp > 0).then_some(hp)
    }
}

#[derive(Debug, Clone)]
pub struct RoundOutcome {
    pub action: Option<ActionPlan>,
    pub frame: Option<RuntimeFrame>,
    pub winner_team: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct CombatRuntime {
    pub entities: EntityArena,
    pub world: WorldArena,
    pub scheduler: PhaseScheduler,
    pub effects: EffectQueue,
    pub effect_handlers: EffectHandlers,
    pub skill_handlers: SkillHandlers,
    pub state_handlers: StateHandlers,
    pub replay_renderers: ReplayRenderers,
    pub show_renderers: ShowRenderers,
    pub scratch: BattleScratch,
    pub template_slots: TemplateSlotStorage,
    pub slots: BattleSlotStorage,
    pub registry: ExtensionRegistry,
    pub rng: RC4,
    #[cfg(not(feature = "no_debug"))]
    pub trace: Option<RuntimeTrace>,
    pub round: u64,
}

impl CombatRuntime {
    pub fn from_template(template: PreparedCombatTemplate) -> Self {
        let PreparedCombatTemplate {
            players,
            registry,
            slots: template_slots,
        } = template;
        let entities = EntityArena::from_templates_with_registry(players, &registry);
        let world = WorldArena::from_entities(&entities);
        let slots = BattleSlotStorage::from_registry(&registry);
        let effect_handlers = EffectHandlers::from_registry(&registry);
        let skill_handlers = SkillHandlers::from_registry(&registry);
        let state_handlers = StateHandlers::from_registry(&registry);
        let replay_renderers = ReplayRenderers::from_registry(&registry);
        let show_renderers = ShowRenderers::from_registry(&registry);
        Self {
            entities,
            world,
            scheduler: PhaseScheduler,
            effects: EffectQueue::default(),
            effect_handlers,
            skill_handlers,
            state_handlers,
            replay_renderers,
            show_renderers,
            scratch: BattleScratch::default(),
            template_slots,
            slots,
            registry,
            rng: RC4::default(),
            #[cfg(not(feature = "no_debug"))]
            trace: None,
            round: 0,
        }
    }

    #[cfg(not(feature = "no_debug"))]
    pub fn enable_trace(&mut self) { self.trace = Some(RuntimeTrace::default()); }

    #[cfg(not(feature = "no_debug"))]
    pub fn trace(&self) -> Option<&RuntimeTrace> { self.trace.as_ref() }

    pub fn set_effect_handler(&mut self, id: EffectHandlerId, handler: EffectHandlerFn) { self.effect_handlers.set(id, handler); }

    pub fn set_effect_handler_with_capabilities(
        &mut self,
        id: EffectHandlerId,
        handler: EffectHandlerFn,
        capabilities: &[ExtensionCapability],
    ) {
        self.effect_handlers.set_with_capabilities(id, handler, capabilities);
    }

    pub fn set_skill_handler(&mut self, id: SkillId, handler: SkillHandlerFn) { self.skill_handlers.set(id, handler); }

    pub fn set_skill_handler_with_capabilities(
        &mut self,
        id: SkillId,
        handler: SkillHandlerFn,
        capabilities: &[ExtensionCapability],
    ) {
        self.skill_handlers.set_with_capabilities(id, handler, capabilities);
    }

    pub fn set_state_handler(&mut self, id: StateId, handler: StateHandlerFn) { self.state_handlers.set(id, handler); }

    pub fn set_state_handler_with_capabilities(
        &mut self,
        id: StateId,
        handler: StateHandlerFn,
        capabilities: &[ExtensionCapability],
    ) {
        self.state_handlers.set_with_capabilities(id, handler, capabilities);
    }

    pub fn set_replay_renderer(&mut self, id: ReplayRendererId, renderer: ReplayRendererFn) {
        self.replay_renderers.set(id, renderer);
    }

    pub fn set_show_renderer(&mut self, id: ShowRendererId, renderer: ShowRendererFn) { self.show_renderers.set(id, renderer); }

    pub fn render_replay_frame(&self, frame: &RuntimeFrame) -> Vec<RenderedReplay> {
        self.registry
            .replay_renderers_in_order()
            .into_iter()
            .filter_map(|spec| {
                let Some(renderer) = self.replay_renderers.get(spec.id) else {
                    panic!("missing runtime_v2 replay renderer implementation: {}", spec.id.0);
                };
                renderer(frame)
            })
            .collect()
    }

    pub fn render_show_frame(&self, frame: &RuntimeFrame) -> Vec<RenderedShow> {
        self.registry
            .show_renderers_in_order()
            .into_iter()
            .filter_map(|spec| {
                let Some(renderer) = self.show_renderers.get(spec.id) else {
                    panic!("missing runtime_v2 show renderer implementation: {}", spec.id.0);
                };
                renderer(frame)
            })
            .collect()
    }

    pub fn run_skill_hooks(&mut self, owner: EntityIdx, hook: ProcMask) -> Option<RuntimeFrame> {
        let plan = self.scheduler.skill_hook_plan(&self.entities, &self.registry, owner, hook);
        self.flush_skill_hook_plan(&plan)
    }

    fn flush_skill_hook_plan(&mut self, plan: &SkillHookPlan) -> Option<RuntimeFrame> {
        let mut updates = RunUpdates::new();
        self.drain_skill_hook_plan_into(plan, &mut updates);
        updates.had_updates().then_some(RuntimeFrame { updates })
    }

    fn drain_skill_hook_plan_into(&mut self, plan: &SkillHookPlan, updates: &mut RunUpdates) {
        for entry in &plan.entries {
            let Some(handler) = self.skill_handlers.get(entry.skill_id) else {
                panic!("missing runtime_v2 skill handler implementation: {}", entry.skill_id.0);
            };
            {
                let capabilities = self.skill_handlers.capabilities(entry.skill_id).unwrap_or(&[]);
                let mut context = SkillContext::new(
                    &mut self.entities,
                    &mut self.world,
                    &self.template_slots,
                    &mut self.slots,
                    &mut self.effects,
                    updates,
                    &mut self.rng,
                    *entry,
                    capabilities,
                );
                handler(&mut context, entry);
            }
            self.drain_effects_into(updates);
        }
    }

    pub fn run_state_hooks(&mut self, owner: EntityIdx, hook: ProcMask) -> Option<RuntimeFrame> {
        let plan = self.scheduler.state_hook_plan(&self.entities, owner, hook);
        self.flush_state_hook_plan(&plan)
    }

    fn flush_state_hook_plan(&mut self, plan: &StateHookPlan) -> Option<RuntimeFrame> {
        let mut updates = RunUpdates::new();
        self.drain_state_hook_plan_into(plan, &mut updates);
        updates.had_updates().then_some(RuntimeFrame { updates })
    }

    fn drain_state_hook_plan_into(&mut self, plan: &StateHookPlan, updates: &mut RunUpdates) {
        for entry in &plan.entries {
            let Some(state_id) = entry.state_id else {
                continue;
            };
            let Some(handler) = self.state_handlers.get(state_id) else {
                panic!("missing runtime_v2 state handler implementation: {}", state_id.0);
            };
            {
                let capabilities = self.state_handlers.capabilities(state_id).unwrap_or(&[]);
                let mut context = StateContext::new(
                    &mut self.entities,
                    &mut self.world,
                    &self.template_slots,
                    &mut self.slots,
                    &mut self.effects,
                    updates,
                    &mut self.rng,
                    *entry,
                    capabilities,
                );
                handler(&mut context, entry);
            }
            self.drain_effects_into(updates);
        }
    }

    pub fn run_minimal_round(&mut self) -> RoundOutcome {
        if let Some(winner_team) = self.world.sync_winner(&self.entities) {
            return RoundOutcome {
                action: None,
                frame: None,
                winner_team: Some(winner_team),
            };
        }

        let Some(action) = self.scheduler.select_minimal_action(&mut self.world, &self.entities) else {
            return RoundOutcome {
                action: None,
                frame: None,
                winner_team: None,
            };
        };
        self.scratch.selected_actor_round = self.round;
        #[cfg(not(feature = "no_debug"))]
        if let Some(trace) = &mut self.trace {
            trace.record_action(TraceAction {
                round: self.round + 1,
                actor: action.actor,
                target: action.target,
                amount: action.amount,
                rng_before: Some(RngCheckpoint::from_rc4(&self.rng)),
                rng_after: Some(RngCheckpoint::from_rc4(&self.rng)),
            });
        }

        let mut updates = RunUpdates::new();
        let skill_plan = self
            .scheduler
            .skill_hook_plan(&self.entities, &self.registry, action.actor, ProcMask::PRE_ACTION);
        self.drain_skill_hook_plan_into(&skill_plan, &mut updates);
        let pre_damage_skill_plan =
            self.scheduler
                .skill_hook_plan(&self.entities, &self.registry, action.actor, ProcMask::PRE_DAMAGE);
        self.drain_skill_hook_plan_into(&pre_damage_skill_plan, &mut updates);
        let pre_damage_state_plan = self.scheduler.state_hook_plan(&self.entities, action.actor, ProcMask::PRE_DAMAGE);
        self.drain_state_hook_plan_into(&pre_damage_state_plan, &mut updates);
        self.effects.push(QueuedEffect::Damage {
            caster: action.actor,
            target: action.target,
            amount: action.amount,
        });
        self.drain_effects_into(&mut updates);
        let post_damage_skill_plan =
            self.scheduler
                .skill_hook_plan(&self.entities, &self.registry, action.actor, ProcMask::POST_DAMAGE);
        self.drain_skill_hook_plan_into(&post_damage_skill_plan, &mut updates);
        let post_damage_state_plan = self.scheduler.state_hook_plan(&self.entities, action.actor, ProcMask::POST_DAMAGE);
        self.drain_state_hook_plan_into(&post_damage_state_plan, &mut updates);
        let post_action_skill_plan =
            self.scheduler
                .skill_hook_plan(&self.entities, &self.registry, action.actor, ProcMask::POST_ACTION);
        self.drain_skill_hook_plan_into(&post_action_skill_plan, &mut updates);
        let state_plan = self.scheduler.state_hook_plan(&self.entities, action.actor, ProcMask::POST_ACTION);
        self.drain_state_hook_plan_into(&state_plan, &mut updates);
        let frame = updates.had_updates().then_some(RuntimeFrame { updates });
        self.round += 1;
        let winner_team = self.world.sync_winner(&self.entities);
        #[cfg(not(feature = "no_debug"))]
        if let (Some(trace), Some(frame)) = (&mut self.trace, &frame) {
            trace.record_frame(self.round, frame, winner_team, Some(RngCheckpoint::from_rc4(&self.rng)));
        }
        RoundOutcome {
            action: Some(action),
            frame,
            winner_team,
        }
    }

    #[cfg(test)]
    fn flush_effects(&mut self) -> Option<RuntimeFrame> {
        let mut updates = RunUpdates::new();
        self.drain_effects_into(&mut updates);
        updates.had_updates().then_some(RuntimeFrame { updates })
    }

    fn drain_effects_into(&mut self, updates: &mut RunUpdates) {
        while let Some(effect) = self.effects.pop_next() {
            match effect {
                QueuedEffect::Damage { caster, target, amount } => {
                    self.ensure_effect_entity("damage", "caster", caster);
                    self.ensure_effect_entity("damage", "target", target);
                    let resolved_target = self.resolve_damage_target(target);
                    self.ensure_effect_entity("damage", "resolved target", resolved_target);
                    let share_targets = self.resolve_damage_share_targets(target, resolved_target);
                    if self.apply_damage_into(caster, resolved_target, amount, updates) {
                        self.drain_lethal_damage_hooks_into(caster, resolved_target, updates);
                    }
                    for share_target in share_targets {
                        self.ensure_effect_entity("damage", "share target", share_target);
                        if self.apply_damage_into(caster, share_target, amount, updates) {
                            self.drain_lethal_damage_hooks_into(caster, share_target, updates);
                        }
                    }
                }
                QueuedEffect::Heal { caster, target, amount } => {
                    self.ensure_effect_entity("heal", "caster", caster);
                    self.ensure_effect_entity("heal", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime_v2 heal target entity: {}", target.0);
                    };
                    let was_alive = target_entity.runtime.alive;
                    target_entity.runtime.hp = (target_entity.runtime.hp + amount.max(0)).min(target_entity.template.max_hp);
                    if target_entity.runtime.hp > 0 {
                        target_entity.runtime.alive = true;
                    }
                    let team = target_entity.runtime.team;
                    updates.add(RuntimeFrame::heal_update(caster.0 as usize, target.0 as usize, amount));
                    if !was_alive && target_entity.runtime.alive {
                        self.world.revive_round_actor(target);
                        self.world.revive_alive(target, team);
                    }
                }
                QueuedEffect::Spawn { caster, template } => {
                    self.ensure_effect_entity("spawn", "caster", caster);
                    let root_owner = self.entities.get(caster).unwrap().runtime.root_owner;
                    let spawned =
                        self.entities
                            .spawn_from_template_with_owner(template, &self.registry, Some(caster), Some(root_owner));
                    let team = self.entities.get(spawned).unwrap().runtime.team;
                    self.world.add_spawned_alive(spawned, team);
                    updates.add(RuntimeFrame::spawn_update(caster.0 as usize, spawned.0 as usize));
                }
                QueuedEffect::AddState { target, state } => {
                    self.ensure_effect_entity("add-state", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime_v2 add-state target entity: {}", target.0);
                    };
                    if target_entity.states.add_entry(state) {
                        updates.add(RuntimeFrame::add_state_update(target.0 as usize));
                    }
                }
                QueuedEffect::ClearState {
                    target,
                    legacy_order_key,
                } => {
                    self.ensure_effect_entity("clear-state", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime_v2 clear-state target entity: {}", target.0);
                    };
                    if target_entity.states.clear_legacy_key(legacy_order_key) {
                        updates.add(RuntimeFrame::clear_state_update(target.0 as usize));
                    }
                }
                QueuedEffect::Revive { caster, target, hp } => {
                    self.ensure_effect_entity("revive", "caster", caster);
                    self.ensure_effect_entity("revive", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime_v2 revive target entity: {}", target.0);
                    };
                    target_entity.runtime.hp = hp.max(1).min(target_entity.template.max_hp);
                    target_entity.runtime.alive = true;
                    let team = target_entity.runtime.team;
                    self.world.revive_round_actor(target);
                    self.world.revive_alive(target, team);
                    updates.add(RuntimeFrame::revive_update(caster.0 as usize, target.0 as usize, hp));
                }
                QueuedEffect::Remove { caster, target } => {
                    self.ensure_effect_entity("remove", "caster", caster);
                    self.ensure_effect_entity("remove", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime_v2 remove target entity: {}", target.0);
                    };
                    target_entity.runtime.hp = 0;
                    target_entity.runtime.alive = false;
                    let team = target_entity.runtime.team;
                    self.world.remove_round_actor(target);
                    self.world.remove_alive(target, team);
                    updates.add(RuntimeFrame::remove_update(caster.0 as usize, target.0 as usize));
                    self.cleanup_linked_minions_for_owner(target, updates);
                }
                QueuedEffect::Merge { caster, target } => {
                    self.ensure_effect_entity("merge", "caster", caster);
                    self.ensure_effect_entity("merge", "target", target);
                    let target_skills = self.entities.get(target).unwrap().template.skills.clone();
                    let policy = self.entities.get(caster).unwrap().runtime.policies.merge;
                    let Some(caster_entity) = self.entities.get_mut(caster) else {
                        panic!("unknown runtime_v2 merge caster entity: {}", caster.0);
                    };
                    if caster_entity.template.skills.merge_fixed_lanes_from(&target_skills, policy) {
                        updates.add(crate::engine::update::RunUpdate::new(
                            "[0][吞噬]了[1]",
                            caster.0 as usize,
                            target.0 as usize,
                            60,
                        ));
                        updates.add(crate::engine::update::RunUpdate::new(
                            "[0]属性上升",
                            caster.0 as usize,
                            target.0 as usize,
                            0,
                        ));
                    }
                }
                QueuedEffect::Replay {
                    caster,
                    target,
                    message,
                    score,
                } => {
                    self.ensure_effect_entity("replay", "caster", caster);
                    self.ensure_effect_entity("replay", "target", target);
                    updates.add(RuntimeFrame::replay_update(
                        caster.0 as usize,
                        target.0 as usize,
                        message,
                        score,
                    ));
                }
                QueuedEffect::Custom(custom) => {
                    self.ensure_effect_entity("custom", "caster", custom.caster);
                    if let Some(target) = custom.target {
                        self.ensure_effect_entity("custom", "target", target);
                    }
                    let Some(handler) = self.effect_handlers.get(custom.handler) else {
                        panic!("missing runtime_v2 effect handler implementation: {}", custom.handler.0);
                    };
                    let capabilities = self.effect_handlers.capabilities(custom.handler).unwrap_or(&[]);
                    let mut context = EffectContext::new(
                        &mut self.entities,
                        &mut self.world,
                        &self.template_slots,
                        &mut self.slots,
                        &mut self.effects,
                        updates,
                        &mut self.rng,
                        &custom,
                        capabilities,
                    );
                    handler(&mut context, &custom);
                }
            }
        }
    }

    fn drain_lethal_damage_hooks_into(&mut self, caster: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let die_skill_plan = self.scheduler.skill_hook_plan(&self.entities, &self.registry, target, ProcMask::DIE);
        self.drain_skill_hook_plan_into(&die_skill_plan, updates);
        let die_state_plan = self.scheduler.state_hook_plan(&self.entities, target, ProcMask::DIE);
        self.drain_state_hook_plan_into(&die_state_plan, updates);
        let kill_skill_plan = self.scheduler.skill_hook_plan(&self.entities, &self.registry, caster, ProcMask::KILL);
        self.drain_skill_hook_plan_into(&kill_skill_plan, updates);
        let kill_state_plan = self.scheduler.state_hook_plan(&self.entities, caster, ProcMask::KILL);
        self.drain_state_hook_plan_into(&kill_state_plan, updates);
    }

    fn apply_damage_into(&mut self, caster: EntityIdx, target: EntityIdx, amount: i32, updates: &mut RunUpdates) -> bool {
        let Some(target_entity) = self.entities.get_mut(target) else {
            panic!("unknown runtime_v2 damage target entity: {}", target.0);
        };
        target_entity.runtime.hp = (target_entity.runtime.hp - amount).max(0);
        let killed = target_entity.runtime.hp == 0 && target_entity.runtime.alive;
        if killed {
            target_entity.runtime.alive = false;
        }
        let team = target_entity.runtime.team;
        updates.add(RuntimeFrame::damage_update(caster.0 as usize, target.0 as usize, amount));
        if killed {
            self.world.remove_alive(target, team);
            self.cleanup_linked_minions_for_owner(target, updates);
        }
        killed
    }

    fn cleanup_linked_minions_for_owner(&mut self, owner: EntityIdx, updates: &mut RunUpdates) {
        let linked_minions = self
            .entities
            .iter()
            .filter_map(|(idx, entity)| {
                (idx != owner
                    && entity.runtime.alive
                    && entity.runtime.owner == owner
                    && entity.runtime.flags.contains(PlayerKindFlags::MINION))
                .then_some(idx)
            })
            .collect::<Vec<_>>();

        for minion in linked_minions {
            let Some(minion_entity) = self.entities.get_mut(minion) else {
                panic!("unknown runtime_v2 linked minion entity: {}", minion.0);
            };
            minion_entity.runtime.hp = 0;
            minion_entity.runtime.alive = false;
            let team = minion_entity.runtime.team;
            self.world.remove_round_actor(minion);
            self.world.remove_alive(minion, team);
            updates.add(RuntimeFrame::remove_update(owner.0 as usize, minion.0 as usize));
        }
    }

    fn resolve_damage_target(&self, target: EntityIdx) -> EntityIdx {
        let target_entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 damage target entity: {}", target.0));
        match target_entity.runtime.policies.owner_resolution {
            OwnerResolutionPolicy::SelfEntity => target,
            OwnerResolutionPolicy::RootOwner => target_entity.runtime.root_owner,
        }
    }

    fn resolve_damage_share_targets(&self, target: EntityIdx, resolved_target: EntityIdx) -> Vec<EntityIdx> {
        let target_entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 damage target entity: {}", target.0));
        match target_entity.runtime.policies.damage_share {
            DamageSharePolicy::None => Vec::new(),
            DamageSharePolicy::ShareToOwner => {
                let owner = target_entity.runtime.owner;
                (owner != resolved_target).then_some(owner).into_iter().collect()
            }
            DamageSharePolicy::ShareToSummons => {
                if resolved_target != target {
                    return Vec::new();
                }
                self.entities
                    .iter()
                    .filter_map(|(idx, entity)| {
                        (idx != target && entity.runtime.alive && entity.runtime.owner == target).then_some(idx)
                    })
                    .collect()
            }
        }
    }

    fn ensure_effect_entity(&self, effect: &'static str, role: &'static str, entity: EntityIdx) {
        if self.entities.get(entity).is_none() {
            panic!("unknown runtime_v2 {effect} {role} entity: {}", entity.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_1v1_template_builds_runtime() {
        let runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));

        assert_eq!(runtime.entities.len(), 2);
        assert_eq!(runtime.world.winner_team(), None);
        assert!(runtime.effects.is_empty());
        assert!(runtime.slots.is_empty());
    }

    #[test]
    fn runtime_from_template_reserves_registered_slot_storage() {
        let mut builder = ExtensionRegistryBuilder::default();
        let template_slot = builder
            .reserve_template_slot("custom", "template", "custom.template")
            .expect("template slot should reserve");
        let battle_slot = builder
            .reserve_battle_slot("custom", "battle", "custom.battle")
            .expect("battle slot should reserve");
        let entity_slot = builder
            .reserve_entity_slot("custom", "entity", "custom.entity")
            .expect("entity slot should reserve");
        let registry = builder.build();
        let mut template = PreparedCombatTemplate::with_registry(vec![PlayerTemplate::new(1, "left", 0, 10, 3)], registry);
        template
            .slots
            .set(template_slot, SlotValue::Text("seed".to_owned()))
            .expect("template slot should write");

        let mut runtime = CombatRuntime::from_template(template);
        runtime.slots.set(battle_slot, SlotValue::U64(1)).expect("battle slot should write");
        runtime
            .entities
            .get_mut(EntityIdx(0))
            .unwrap()
            .slots
            .set(entity_slot, SlotValue::Bool(true))
            .expect("entity slot should write");

        assert_eq!(
            runtime.template_slots.get(template_slot),
            Some(&SlotValue::Text("seed".to_owned()))
        );
        assert_eq!(runtime.slots.get(battle_slot), Some(&SlotValue::U64(1)));
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(entity_slot),
            Some(&SlotValue::Bool(true))
        );
    }

    #[test]
    fn custom_bed2_fixture_maps_kind_skill_and_marker_slots() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill_with_hooks(
                "custom",
                "summon",
                "custom.summon",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("summon skill should register");
        let fire = builder
            .register_skill(
                "custom",
                "summon-fire",
                "custom.summon.fire",
                TargetPolicy::Enemy,
                SkillPriority(1),
            )
            .expect("summon fire skill should register");
        let explode = builder
            .register_skill(
                "custom",
                "summon-explode",
                "custom.summon.explode",
                TargetPolicy::Enemy,
                SkillPriority(2),
            )
            .expect("summon explode skill should register");
        let summon_template = builder
            .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
            .expect("bed2 summon template slot should reserve");
        let hp_marker = builder
            .reserve_entity_slot("custom", "hp-marker", "custom.hp_marker")
            .expect("hp marker slot should reserve");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-summon",
                "custom.bed2.summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: true,
                },
            )
            .expect("bed2 summon kind should register");
        let registry = builder.build();
        let mut template = PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "bed2", bed2, 0, 3000, 0)
                    .with_def_res(DEFAULT_BED2_DEFENSE, DEFAULT_BED2_RESISTANCE)
                    .with_skills([summon]),
            ],
            registry,
        );
        let bed2_summon_template = PlayerTemplate::with_kind(2, "bed2?0", summon_kind, 0, 1000, 1)
            .with_def_res(99, 99)
            .with_skills([fire, explode]);
        template
            .slots
            .set(
                summon_template,
                SlotValue::PlayerTemplate(Box::new(bed2_summon_template.clone())),
            )
            .expect("bed2 summon template slot should write");

        let mut runtime = CombatRuntime::from_template(template);
        runtime.set_skill_handler_with_capabilities(
            summon,
            skill_bed2_template_slot_summon_handler,
            &[ExtensionCapability::ReadTemplateSlots],
        );
        runtime
            .entities
            .get_mut(EntityIdx(0))
            .unwrap()
            .slots
            .set(hp_marker, SlotValue::Bool(true))
            .expect("hp marker slot should write");
        let entity = runtime.entities.get(EntityIdx(0)).expect("bed2 entity should exist");

        assert_eq!(entity.template.max_hp, 3000);
        assert_eq!(entity.template.skills.skills(), &[summon]);
        assert!(entity.runtime.flags.contains(PlayerKindFlags::BED2));
        assert_eq!(entity.runtime.policies.owner_resolution, OwnerResolutionPolicy::RootOwner);
        assert_eq!(entity.runtime.policies.damage_share, DamageSharePolicy::ShareToOwner);
        assert_eq!(entity.runtime.policies.merge, MergePolicy::FixedLane);
        assert_eq!(entity.slots.get(hp_marker), Some(&SlotValue::Bool(true)));
        assert_eq!(
            runtime.template_slots.get(summon_template),
            Some(&SlotValue::PlayerTemplate(Box::new(bed2_summon_template.clone())))
        );
        let SlotValue::PlayerTemplate(stored_template) =
            runtime.template_slots.get(summon_template).expect("bed2 summon template should persist")
        else {
            panic!("bed2 summon template slot should hold a PlayerTemplate payload");
        };
        assert_eq!(stored_template.kind, summon_kind);
        assert_eq!(stored_template.max_hp, 1000);
        assert_eq!(stored_template.defense, DEFAULT_BED2_DEFENSE);
        assert_eq!(stored_template.resistance, DEFAULT_BED2_RESISTANCE);
        assert_eq!(stored_template.skills.skills(), &[fire, explode]);

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("bed2 summon handler should spawn template payload");

        assert_eq!(runtime.entities.len(), 2);
        assert_eq!(frame.updates.updates.len(), 1);
        assert_eq!(frame.updates.updates[0].message, "出现一个新的[1]");
        assert_eq!(frame.updates.updates[0].target, 1);
        let summoned = runtime.entities.get(EntityIdx(1)).expect("bed2 summon should spawn from template slot");
        assert_eq!(summoned.template.kind, summon_kind);
        assert_eq!(summoned.template.max_hp, 1000);
        assert_eq!(summoned.template.skills.skills(), &[fire, explode]);
        assert_eq!(summoned.runtime.owner, EntityIdx(0));
        assert_eq!(summoned.runtime.root_owner, EntityIdx(0));
        assert_eq!(summoned.runtime.defense, DEFAULT_BED2_DEFENSE);
        assert_eq!(summoned.runtime.resistance, DEFAULT_BED2_RESISTANCE);
    }

    #[test]
    fn custom_bed2_import_fixture_parses_markers_into_v2_template() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let registry = builder.build();
        let plus = CustomBed2Import::parse("alpha@red+bed2[4500]+ol:{\"skills\":{\"sklsummon\":255}}")
            .expect("bed2 plus marker should parse");
        let legacy_team = CustomBed2Import::parse("beta@blue@bed2").expect("legacy bed2 team marker should parse");
        let bare = CustomBed2Import::parse("gamma+bed2[2500]").expect("bare bed2 marker should parse");

        assert_eq!(plus.name, "alpha");
        assert_eq!(plus.team.as_deref(), Some("red"));
        assert_eq!(plus.hp, 4500);
        assert_eq!(legacy_team.name, "beta");
        assert_eq!(legacy_team.team.as_deref(), Some("blue"));
        assert_eq!(legacy_team.hp, DEFAULT_BED2_HP);
        assert_eq!(bare.name, "gamma");
        assert_eq!(bare.team, None);
        assert_eq!(bare.hp, 2500);
        assert_eq!(CustomBed2Import::parse("alpha@red+bed2[0]"), None);

        let facade_bridge =
            CustomBed2Import::parse_player_facade_raw("alpha@red+weapon+bed2[4500]+ol:{\"skills\":{\"sklsummon\":255}}")
                .expect("bed2 raw should bridge through player facade id name");
        assert_eq!(
            crate::player::Player::raw_namerena_to_idname("alpha@red+weapon+bed2[4500]+ol:{\"skills\":{\"sklsummon\":255}}"),
            "alpha@red"
        );
        assert_eq!(facade_bridge.name, "alpha");
        assert_eq!(facade_bridge.team.as_deref(), Some("red"));
        assert_eq!(facade_bridge.hp, 4500);

        let same_team_bridge = CustomBed2Import::parse_player_facade_raw("same@same+bed2[1800]")
            .expect("same-team bed2 raw should bridge through normalized player facade id name");
        assert_eq!(crate::player::Player::raw_namerena_to_idname("same@same+bed2[1800]"), "same");
        assert_eq!(same_team_bridge.name, "same");
        assert_eq!(same_team_bridge.team, None);
        assert_eq!(same_team_bridge.hp, 1800);

        let template = plus.into_player_template(1, bed2, 0, summon);
        let runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(vec![template], registry));
        let entity = runtime.entities.get(EntityIdx(0)).expect("bed2 entity should import");

        assert_eq!(entity.template.name, "alpha");
        assert_eq!(entity.template.max_hp, 4500);
        assert_eq!(entity.template.attack, 0);
        assert_eq!(entity.template.defense, DEFAULT_BED2_DEFENSE);
        assert_eq!(entity.template.resistance, DEFAULT_BED2_RESISTANCE);
        assert_eq!(entity.template.skills.skills(), &[summon]);
        assert!(entity.runtime.flags.contains(PlayerKindFlags::BED2));
    }

    #[test]
    fn custom_bed2_roster_import_builds_prepared_template_from_grouped_raw_players() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let registry = builder.build();
        let raw_groups = vec![
            vec![
                "alpha@red+weapon+bed2[4500]+ol:{\"skills\":{\"sklsummon\":255}}".to_owned(),
                "seed:custom-seed@!".to_owned(),
            ],
            vec!["beta@blue@bed2".to_owned(), "same@same+bed2[1800]".to_owned()],
        ];

        let template = CustomBed2Import::roster_into_prepared_template(&raw_groups, registry, bed2, summon)
            .expect("grouped bed2 raw roster should build a prepared template");

        assert_eq!(template.players.len(), 3);
        assert_eq!(template.players[0].id, 1);
        assert_eq!(template.players[0].name, "alpha");
        assert_eq!(template.players[0].team, 0);
        assert_eq!(template.players[0].max_hp, 4500);
        assert_eq!(template.players[0].skills.skills(), &[summon]);
        assert_eq!(template.players[1].id, 2);
        assert_eq!(template.players[1].name, "beta");
        assert_eq!(template.players[1].team, 1);
        assert_eq!(template.players[1].max_hp, DEFAULT_BED2_HP);
        assert_eq!(template.players[2].id, 3);
        assert_eq!(template.players[2].name, "same");
        assert_eq!(template.players[2].team, 1);
        assert_eq!(template.players[2].max_hp, 1800);
        assert!(template.players.iter().all(|player| player.kind == bed2
            && player.attack == 0
            && player.defense == DEFAULT_BED2_DEFENSE
            && player.resistance == DEFAULT_BED2_RESISTANCE));

        let runtime = CombatRuntime::from_template(template);
        assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0)].as_slice()));
        assert_eq!(runtime.world.team_alive(1), Some([EntityIdx(1), EntityIdx(2)].as_slice()));
        assert!(
            runtime
                .entities
                .iter()
                .all(|(_, entity)| entity.runtime.flags.contains(PlayerKindFlags::BED2))
        );
    }

    #[test]
    fn custom_bed2_roster_import_rejects_non_bed2_players() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let registry = builder.build();
        let raw_groups = vec![vec!["alpha+bed2[4500]".to_owned()], vec!["plain".to_owned()]];

        let err = CustomBed2Import::roster_into_prepared_template(&raw_groups, registry, bed2, summon)
            .expect_err("non-bed2 raw players should be rejected by the bed2 roster importer");

        assert_eq!(
            err,
            CustomBed2RosterImportError {
                team_index: 1,
                player_index: 0,
                raw: "plain".to_owned(),
            }
        );
    }

    #[test]
    fn custom_mixed_roster_import_bridges_bed2_and_legacy_player_templates() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let registry = builder.build();
        let raw_groups = vec![
            vec!["plain@red".to_owned(), "alpha@red+bed2[4500]".to_owned()],
            vec!["seed:custom-seed@!".to_owned(), "beta@blue@bed2".to_owned()],
        ];

        let template = CustomBed2Import::mixed_roster_into_prepared_template(&raw_groups, registry, bed2, summon)
            .expect("mixed legacy/bed2 raw roster should build a prepared template");
        let legacy_storage = crate::engine::storage::Storage::new_arc();
        let mut legacy_plain = crate::player::Player::new_from_namerena_raw("plain@red".to_owned(), legacy_storage)
            .expect("legacy player facade should parse plain player");
        legacy_plain.build();
        let legacy_status = legacy_plain.get_status();

        assert_eq!(template.players.len(), 3);
        assert_eq!(template.players[0].id, 1);
        assert_eq!(template.players[0].name, legacy_plain.id_name());
        assert_eq!(template.players[0].kind, PlayerTemplate::DEFAULT_KIND);
        assert_eq!(template.players[0].team, 0);
        assert_eq!(template.players[0].max_hp, legacy_status.max_hp);
        assert_eq!(template.players[0].attack, legacy_status.attack);
        assert_eq!(template.players[0].defense, legacy_status.defense);
        assert_eq!(template.players[0].resistance, legacy_status.resistance);
        assert_eq!(template.players[1].id, 2);
        assert_eq!(template.players[1].kind, bed2);
        assert_eq!(template.players[1].name, "alpha");
        assert_eq!(template.players[1].team, 0);
        assert_eq!(template.players[1].max_hp, 4500);
        assert_eq!(template.players[1].skills.skills(), &[summon]);
        assert_eq!(template.players[2].id, 3);
        assert_eq!(template.players[2].kind, bed2);
        assert_eq!(template.players[2].name, "beta");
        assert_eq!(template.players[2].team, 1);
        assert_eq!(template.players[2].max_hp, DEFAULT_BED2_HP);

        let runtime = CombatRuntime::from_template(template);
        assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.flags.contains(PlayerKindFlags::BED2));
        assert!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.flags.contains(PlayerKindFlags::BED2));
        assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.flags.contains(PlayerKindFlags::BED2));
        assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0), EntityIdx(1)].as_slice()));
        assert_eq!(runtime.world.team_alive(1), Some([EntityIdx(2)].as_slice()));
    }

    #[test]
    fn runtime_v2_runner_constructs_and_runs_mixed_roster() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let registry = builder.build();
        let raw_groups = vec![
            vec!["plain@red".to_owned(), "alpha@red+bed2[9]".to_owned()],
            vec!["seed:custom-seed@!".to_owned(), "beta@blue@bed2".to_owned()],
        ];

        let mut runner = RuntimeV2Runner::from_mixed_roster(&raw_groups, registry, bed2, summon)
            .expect("mixed roster should construct a runtime v2 runner");

        assert_eq!(
            runner.runtime().world.team_alive(0),
            Some([EntityIdx(0), EntityIdx(1)].as_slice())
        );
        assert_eq!(runner.runtime().world.team_alive(1), Some([EntityIdx(2)].as_slice()));
        assert_eq!(runner.runtime().entities.get(EntityIdx(1)).unwrap().template.max_hp, 9);
        assert!(
            runner
                .runtime()
                .entities
                .get(EntityIdx(1))
                .unwrap()
                .runtime
                .flags
                .contains(PlayerKindFlags::BED2)
        );

        let actor_attack = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.attack;
        let plain_hp = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.max_hp;
        let plain_defense = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.defense;
        let plain_resistance = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.resistance;

        let actual = runner.run_round_normalized();
        let expected = NormalizedOutcome {
            winner_team: None,
            round: 1,
            total_score: actor_attack as u64,
            rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::default(),
            entity_ids: vec![1, 2, 3],
            teams: vec![0, 0, 1],
            hp: vec![plain_hp, 9, DEFAULT_BED2_HP - actor_attack],
            defense: vec![plain_defense, 99, 99],
            resistance: vec![plain_resistance, 99, 99],
            alive: vec![true, true, true],
            round_order: vec![0, 1, 2],
            flat_alive: vec![0, 1, 2],
            team_alive: vec![vec![0, 1], vec![2]],
            alive_group_count: 2,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 1,
                actor: 0,
                target: 2,
                amount: actor_attack,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 0,
                target: 2,
                targets: Vec::new(),
                param: None,
                score: actor_attack as u32,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        };

        assert_eq!(strict_diff(&expected, &actual), Ok(()));
    }

    #[test]
    fn runtime_v2_runner_rejects_plain_rows_in_bed2_roster() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies::default(),
            )
            .expect("bed2 kind should register");
        let registry = builder.build();
        let raw_groups = vec![vec!["plain".to_owned()], vec!["beta@blue@bed2".to_owned()]];

        let err = RuntimeV2Runner::from_bed2_roster(&raw_groups, registry, bed2, summon)
            .expect_err("bed2-only runner constructor should reject non-bed2 rows");

        assert_eq!(
            err,
            CustomBed2RosterImportError {
                team_index: 0,
                player_index: 0,
                raw: "plain".to_owned(),
            }
        );
    }

    #[cfg(not(feature = "no_debug"))]
    #[test]
    fn run_minimal_round_records_trace_when_enabled() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.enable_trace();

        runtime.run_minimal_round();

        let trace = runtime.trace().expect("trace should be enabled");
        assert_eq!(trace.actions.len(), 1);
        assert_eq!(trace.actions[0].actor, EntityIdx(0));
        assert_eq!(trace.actions[0].target, EntityIdx(1));
        assert_eq!(
            trace.actions[0].rng_before,
            Some(RngCheckpoint {
                i: 0,
                j: 0,
                byte_count: 0,
            })
        );
        assert_eq!(trace.actions[0].rng_after, trace.actions[0].rng_before);
        assert_eq!(trace.frames.len(), 1);
        assert_eq!(trace.frames[0].total_score, 3);
        assert_eq!(trace.frames[0].winner_team, None);
        assert_eq!(
            trace.frames[0].rng_after,
            Some(RngCheckpoint {
                i: 0,
                j: 0,
                byte_count: 0,
            })
        );
        assert_eq!(trace.frames[0].updates[0].score, 3);
    }

    #[test]
    fn run_minimal_round_applies_damage_frame() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        let outcome = runtime.run_minimal_round();

        assert_eq!(outcome.winner_team, None);
        assert!(outcome.frame.is_some());
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
        assert_eq!(runtime.round, 1);
    }

    #[test]
    fn run_minimal_round_reports_winner_after_lethal_damage() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 3, 3));
        let outcome = runtime.run_minimal_round();

        assert_eq!(outcome.winner_team, Some(0));
        assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
    }

    #[test]
    fn run_minimal_round_dispatches_die_and_kill_state_hooks_after_lethal_damage() {
        let mut builder = ExtensionRegistryBuilder::default();
        let die_state = builder
            .register_state("custom", "die", "custom.die", ProcMask::DIE, SkillPriority(0))
            .expect("die state should register");
        let kill_state = builder
            .register_state("custom", "kill", "custom.kill", ProcMask::KILL, SkillPriority(0))
            .expect("kill state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 3, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 11,
            extension_state_id: Some(kill_state),
            hook_mask: ProcMask::KILL,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
        });
        runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 22,
            extension_state_id: Some(die_state),
            hook_mask: ProcMask::DIE,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(1),
        });
        runtime.set_state_handler(die_state, state_marks_update);
        runtime.set_state_handler(kill_state, state_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("lethal attack should emit hooks");

        assert_eq!(outcome.winner_team, Some(0));
        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "state mark");
        assert_eq!(frame.updates.updates[1].score, 22);
        assert_eq!(frame.updates.updates[2].message, "state mark");
        assert_eq!(frame.updates.updates[2].score, 11);
    }

    #[test]
    fn flush_effects_dispatches_die_and_kill_skill_hooks_after_lethal_damage() {
        let mut builder = ExtensionRegistryBuilder::default();
        let die_skill = builder
            .register_skill_with_hooks(
                "custom",
                "die-skill",
                "custom.die_skill",
                ProcMask::DIE,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("die skill should register");
        let kill_skill = builder
            .register_skill_with_hooks(
                "custom",
                "kill-skill",
                "custom.kill_skill",
                ProcMask::KILL,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("kill skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([kill_skill]),
                PlayerTemplate::new(2, "right", 1, 3, 3).with_skills([die_skill]),
            ],
            registry,
        ));
        runtime.set_skill_handler(die_skill, skill_marks_update);
        runtime.set_skill_handler(kill_skill, skill_marks_update);
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            amount: 3,
        });

        let frame = runtime.flush_effects().expect("lethal damage should emit hooks");

        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "skill mark");
        assert_eq!(frame.updates.updates[1].score, die_skill.0);
        assert_eq!(frame.updates.updates[2].message, "skill mark");
        assert_eq!(frame.updates.updates[2].score, kill_skill.0);
    }

    #[test]
    fn flush_effects_routes_root_owner_damage_to_owner_entity() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("summon kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 3),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1),
        });
        runtime.flush_effects().expect("spawn should emit update");
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(2),
            amount: 4,
        });

        let frame = runtime.flush_effects().expect("routed damage should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 5);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[0].target, 0);
        assert_eq!(frame.updates.updates[0].score, 4);
    }

    #[test]
    fn flush_effects_runs_die_hook_on_resolved_root_owner() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("summon kind should register");
        let die_state = builder
            .register_state("custom", "die", "custom.die", ProcMask::DIE, SkillPriority(0))
            .expect("die state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 4, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 99,
            extension_state_id: Some(die_state),
            hook_mask: ProcMask::DIE,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
        });
        runtime.set_state_handler(die_state, state_marks_update);
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1),
        });
        runtime.flush_effects().expect("spawn should emit update");
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(2),
            amount: 4,
        });

        let frame = runtime.flush_effects().expect("lethal routed damage should emit hooks");

        assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
        assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].target, 0);
        assert_eq!(frame.updates.updates[1].message, "state mark");
        assert_eq!(frame.updates.updates[1].score, 99);
    }

    #[test]
    fn flush_effects_shares_summon_damage_to_owner_entity() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("summon kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 3),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1),
        });
        runtime.flush_effects().expect("spawn should emit update");
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(2),
            amount: 4,
        });

        let frame = runtime.flush_effects().expect("shared damage should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 1);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].target, 2);
        assert_eq!(frame.updates.updates[0].score, 4);
        assert_eq!(frame.updates.updates[1].target, 0);
        assert_eq!(frame.updates.updates[1].score, 4);
    }

    #[test]
    fn flush_effects_runs_die_hook_on_damage_share_owner() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("summon kind should register");
        let die_state = builder
            .register_state("custom", "die", "custom.die", ProcMask::DIE, SkillPriority(0))
            .expect("die state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 4, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 101,
            extension_state_id: Some(die_state),
            hook_mask: ProcMask::DIE,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
        });
        runtime.set_state_handler(die_state, state_marks_update);
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1),
        });
        runtime.flush_effects().expect("spawn should emit update");
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(2),
            amount: 4,
        });

        let frame = runtime.flush_effects().expect("shared lethal damage should emit hooks");

        assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
        assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].target, 2);
        assert_eq!(frame.updates.updates[1].target, 0);
        assert_eq!(frame.updates.updates[2].message, "state mark");
        assert_eq!(frame.updates.updates[2].score, 101);
    }

    #[test]
    fn flush_effects_removes_lethal_damage_target_from_alive_views() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 4, 3));
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            amount: 4,
        });

        runtime.flush_effects().expect("lethal damage should emit update");

        assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
        assert_eq!(runtime.world.team_alive(1), Some([].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0)]);
        assert_eq!(runtime.world.alive_group_count(), 1);
        assert_eq!(runtime.world.first_alive_enemy(EntityIdx(0), &runtime.entities), None);
    }

    #[test]
    fn flush_effects_shares_owner_damage_to_alive_summons() {
        let mut builder = ExtensionRegistryBuilder::default();
        let owner_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "owner",
                "custom.owner",
                PlayerKindFlags::default(),
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToSummons,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("owner kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 3),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::new(3, "summon-a", 0, 5, 1),
        });
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::new(4, "summon-b", 0, 5, 1),
        });
        runtime.flush_effects().expect("spawns should emit updates");
        runtime.entities.get_mut(EntityIdx(3)).unwrap().runtime.alive = false;
        runtime.entities.get_mut(EntityIdx(3)).unwrap().runtime.hp = 0;
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(0),
            amount: 3,
        });

        let frame = runtime.flush_effects().expect("shared summon damage should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 7);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 2);
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 0);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].target, 0);
        assert_eq!(frame.updates.updates[0].score, 3);
        assert_eq!(frame.updates.updates[1].target, 2);
        assert_eq!(frame.updates.updates[1].score, 3);
    }

    #[test]
    fn custom_summon_fixture_combines_owner_route_share_and_skill_reuse() {
        let mut builder = ExtensionRegistryBuilder::default();
        let recast_skill = builder
            .register_skill(
                "custom",
                "summon-recast",
                "custom.summon_recast",
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("summon recast skill should register");
        let owner_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon-owner",
                "custom.summon_owner",
                PlayerKindFlags::default(),
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToSummons,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("summon owner kind should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: true,
                },
            )
            .expect("summon kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 3),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1).with_skills([recast_skill]),
        });
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(4, "summon-recast", summon_kind, 0, 5, 1).with_skills([recast_skill]),
        });
        runtime.flush_effects().expect("summon spawns should emit updates");

        assert_eq!(
            runtime.entities.get(EntityIdx(2)).unwrap().template.skills.skills(),
            &[recast_skill]
        );
        assert_eq!(
            runtime.entities.get(EntityIdx(3)).unwrap().template.skills.skills(),
            &[recast_skill]
        );
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.owner, EntityIdx(0));
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.root_owner, EntityIdx(0));
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.owner, EntityIdx(0));
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.root_owner, EntityIdx(0));

        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(2),
            amount: 4,
        });
        let routed = runtime.flush_effects().expect("summon/root owner damage should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 5);
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 5);
        assert_eq!(routed.updates.updates.len(), 1);
        assert_eq!(routed.updates.updates[0].target, 0);
        assert_eq!(routed.updates.updates[0].score, 4);

        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(0),
            amount: 2,
        });
        let shared = runtime.flush_effects().expect("owner damage should share to summons");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 4);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 3);
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 3);
        assert_eq!(shared.updates.updates.len(), 3);
        assert_eq!(shared.updates.updates[0].target, 0);
        assert_eq!(shared.updates.updates[1].target, 2);
        assert_eq!(shared.updates.updates[2].target, 3);
    }

    #[test]
    fn custom_summon_fixture_inherits_owner_defense_and_resistance() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: true,
                },
            )
            .expect("summon kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 20, 3).with_def_res(77, 88),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
            ],
            registry,
        ));

        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 10, 1).with_def_res(11, 22),
        });
        runtime.flush_effects().expect("summon spawn should emit update");

        let summon = runtime.entities.get(EntityIdx(2)).expect("summon should spawn");
        assert_eq!(summon.template.defense, 77);
        assert_eq!(summon.template.resistance, 88);
        assert_eq!(summon.runtime.defense, 77);
        assert_eq!(summon.runtime.resistance, 88);
    }

    #[test]
    fn custom_summon_recast_handler_revives_existing_summon_entity() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summoned_slot = builder
            .reserve_entity_slot("custom", "summoned-entity", "custom.summon.summoned_entity")
            .expect("summoned entity slot should reserve");
        let recast_skill = builder
            .register_skill_with_hooks(
                "custom",
                "summon-recast",
                "custom.summon_recast",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("summon recast skill should register");
        let owner_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon-owner",
                "custom.summon_owner",
                PlayerKindFlags::default(),
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToSummons,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("summon owner kind should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: true,
                },
            )
            .expect("summon kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 20, 3)
                    .with_def_res(77, 88)
                    .with_skills([recast_skill]),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
            ],
            registry,
        ));
        runtime.set_skill_handler_with_capabilities(
            recast_skill,
            skill_summon_recast_fixture_handler,
            &[ExtensionCapability::ReadAllies, ExtensionCapability::MutateEntitySlots],
        );

        let first = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("first summon cast should emit spawn update");

        assert_eq!(runtime.entities.len(), 3);
        assert_eq!(first.updates.updates.len(), 1);
        assert_eq!(first.updates.updates[0].message, "出现一个新的[1]");
        assert_eq!(first.updates.updates[0].target, 2);
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(summoned_slot),
            Some(&SlotValue::U64(2))
        );
        let summon = runtime.entities.get(EntityIdx(2)).expect("summon should exist");
        assert_eq!(summon.template.kind, summon_kind);
        assert_eq!(summon.template.skills.skills(), &[recast_skill]);
        assert_eq!(summon.runtime.owner, EntityIdx(0));
        assert_eq!(summon.runtime.root_owner, EntityIdx(0));
        assert_eq!(summon.runtime.defense, 77);
        assert_eq!(summon.runtime.resistance, 88);

        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(2),
            amount: 10,
        });
        runtime.flush_effects().expect("lethal summon damage should emit update");
        assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
        assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0)].as_slice()));

        let recast = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("summon recast should revive existing entity");

        assert_eq!(runtime.entities.len(), 3);
        assert_eq!(recast.updates.updates.len(), 1);
        assert_eq!(recast.updates.updates[0].message, "[1][复活]了");
        assert_eq!(recast.updates.updates[0].target, 2);
        let revived = runtime.entities.get(EntityIdx(2)).expect("summon should revive in place");
        assert!(revived.runtime.alive);
        assert_eq!(revived.runtime.hp, 10);
        assert_eq!(revived.template.skills.skills(), &[recast_skill]);
        assert_eq!(runtime.world.round_order(), &[EntityIdx(0), EntityIdx(1), EntityIdx(2)]);
        assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0), EntityIdx(2)].as_slice()));
    }

    #[test]
    fn custom_minion_heal_fixture_does_not_share_with_owner_or_summons() {
        let mut builder = ExtensionRegistryBuilder::default();
        let owner_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "minion-owner",
                "custom.minion_owner",
                PlayerKindFlags::default(),
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToSummons,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("minion owner kind should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "minion",
                "custom.minion",
                PlayerKindFlags::MINION | PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("minion kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 20, 3),
                PlayerTemplate::new(2, "healer", 0, 10, 1),
                PlayerTemplate::new(3, "enemy", 1, 10, 1),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(4, "summon-a", summon_kind, 0, 10, 1),
        });
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(5, "summon-b", summon_kind, 0, 10, 1),
        });
        runtime.flush_effects().expect("minion spawns should emit updates");
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(2),
            target: EntityIdx(0),
            amount: 4,
        });
        let shared_damage = runtime.flush_effects().expect("owner damage should share to minions");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 16);
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 6);
        assert_eq!(runtime.entities.get(EntityIdx(4)).unwrap().runtime.hp, 6);
        assert_eq!(shared_damage.updates.updates.len(), 3);
        assert_eq!(shared_damage.updates.updates[0].target, 0);
        assert_eq!(shared_damage.updates.updates[1].target, 3);
        assert_eq!(shared_damage.updates.updates[2].target, 4);

        runtime.effects.push(QueuedEffect::Heal {
            caster: EntityIdx(1),
            target: EntityIdx(3),
            amount: 3,
        });
        let minion_heal = runtime.flush_effects().expect("minion heal should emit one update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 16);
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 9);
        assert_eq!(runtime.entities.get(EntityIdx(4)).unwrap().runtime.hp, 6);
        assert_eq!(minion_heal.updates.updates.len(), 1);
        assert_eq!(minion_heal.updates.updates[0].message, "[1]回复体力[2]点");
        assert_eq!(minion_heal.updates.updates[0].target, 3);
        assert_eq!(minion_heal.updates.updates[0].score, 3);
    }

    #[test]
    fn custom_minion_owner_death_removes_linked_minions_in_entity_order() {
        let mut builder = ExtensionRegistryBuilder::default();
        let minion_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "minion",
                "custom.minion",
                PlayerKindFlags::MINION | PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("minion kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "owner?0", minion_kind, 0, 4, 1),
        });
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(4, "owner?1", minion_kind, 0, 4, 1),
        });
        runtime.flush_effects().expect("minion spawns should emit updates");

        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(0),
            amount: 10,
        });
        let frame = runtime.flush_effects().expect("owner death should cleanup linked minions");

        assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
        assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
        assert!(!runtime.entities.get(EntityIdx(3)).unwrap().runtime.alive);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 0);
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 0);
        assert_eq!(runtime.world.round_order(), &[EntityIdx(0), EntityIdx(1)]);
        assert_eq!(runtime.world.team_alive(0), Some([].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(1)]);
        assert_eq!(runtime.world.alive_group_count(), 1);
        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].target, 0);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].target, 2);
        assert_eq!(frame.updates.updates[1].message, "[1]消失了");
        assert_eq!(frame.updates.updates[2].target, 3);
        assert_eq!(frame.updates.updates[2].message, "[1]消失了");
    }

    #[test]
    fn custom_minion_owner_remove_cleans_linked_minions_in_entity_order() {
        let mut builder = ExtensionRegistryBuilder::default();
        let minion_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "minion",
                "custom.minion",
                PlayerKindFlags::MINION | PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("minion kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "owner?0", minion_kind, 0, 4, 1),
        });
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(4, "owner?1", minion_kind, 0, 4, 1),
        });
        runtime.flush_effects().expect("minion spawns should emit updates");

        runtime.effects.push(QueuedEffect::Remove {
            caster: EntityIdx(1),
            target: EntityIdx(0),
        });
        let frame = runtime.flush_effects().expect("owner remove should cleanup linked minions");

        assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
        assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
        assert!(!runtime.entities.get(EntityIdx(3)).unwrap().runtime.alive);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 0);
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 0);
        assert_eq!(runtime.world.round_order(), &[EntityIdx(1)]);
        assert_eq!(runtime.world.team_alive(0), Some([].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(1)]);
        assert_eq!(runtime.world.alive_group_count(), 1);
        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].target, 0);
        assert_eq!(frame.updates.updates[0].message, "[1]消失了");
        assert_eq!(frame.updates.updates[1].target, 2);
        assert_eq!(frame.updates.updates[1].message, "[1]消失了");
        assert_eq!(frame.updates.updates[2].target, 3);
        assert_eq!(frame.updates.updates[2].message, "[1]消失了");
    }

    #[test]
    fn custom_runner_fixture_matches_strict_diff_golden() {
        let mut builder = ExtensionRegistryBuilder::default();
        let owner_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "runner-owner",
                "custom.runner_owner",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToSummons,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("runner owner kind should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "runner-summon",
                "custom.runner_summon",
                PlayerKindFlags::MINION | PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: true,
                },
            )
            .expect("runner summon kind should register");
        let hp_marker = builder
            .reserve_entity_slot("custom", "hp-marker", "custom.hp_marker")
            .expect("hp marker slot should reserve");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 20, 3).with_def_res(77, 88),
                PlayerTemplate::new(2, "healer", 0, 10, 1),
                PlayerTemplate::new(3, "enemy", 1, 10, 1),
            ],
            registry,
        ));

        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(4, "summon", summon_kind, 0, 10, 1).with_def_res(11, 22),
        });
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(2),
            target: EntityIdx(0),
            amount: 4,
        });
        runtime.effects.push(QueuedEffect::Heal {
            caster: EntityIdx(1),
            target: EntityIdx(3),
            amount: 2,
        });
        runtime.effects.push(QueuedEffect::Replay {
            caster: EntityIdx(0),
            target: EntityIdx(0),
            message: "[0]还剩[2]点血".to_owned(),
            score: 87,
        });

        let frame = runtime.flush_effects().expect("custom runner fixture should emit updates");
        runtime
            .entities
            .get_mut(EntityIdx(0))
            .unwrap()
            .slots
            .set(hp_marker, SlotValue::Bool(true))
            .expect("hp marker slot should write");
        let outcome = RoundOutcome {
            action: None,
            frame: Some(frame),
            winner_team: runtime.world.sync_winner(&runtime.entities),
        };
        let actual = NormalizedOutcome::from_runtime(&runtime, &outcome);
        let expected = NormalizedOutcome {
            winner_team: None,
            round: 0,
            total_score: 97,
            rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::default(),
            entity_ids: vec![1, 2, 3, 4],
            teams: vec![0, 0, 1, 0],
            hp: vec![16, 10, 10, 8],
            defense: vec![77, 0, 0, 77],
            resistance: vec![88, 0, 0, 88],
            alive: vec![true, true, true, true],
            round_order: vec![0, 1, 2, 3],
            flat_alive: vec![0, 1, 3, 2],
            team_alive: vec![vec![0, 1, 3], vec![2]],
            alive_group_count: 2,
            actions: Vec::new(),
            frames: vec![
                NormalizedUpdateFrame {
                    message: "出现一个新的[1]".to_owned(),
                    caster: 0,
                    target: 3,
                    targets: Vec::new(),
                    param: None,
                    score: 0,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
                NormalizedUpdateFrame {
                    message: "[0]攻击[1]".to_owned(),
                    caster: 2,
                    target: 0,
                    targets: Vec::new(),
                    param: None,
                    score: 4,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
                NormalizedUpdateFrame {
                    message: "[0]攻击[1]".to_owned(),
                    caster: 2,
                    target: 3,
                    targets: Vec::new(),
                    param: None,
                    score: 4,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
                NormalizedUpdateFrame {
                    message: "[1]回复体力[2]点".to_owned(),
                    caster: 1,
                    target: 3,
                    targets: Vec::new(),
                    param: None,
                    score: 2,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
                NormalizedUpdateFrame {
                    message: "[0]还剩[2]点血".to_owned(),
                    caster: 0,
                    target: 0,
                    targets: Vec::new(),
                    param: None,
                    score: 87,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
            ],
        };

        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(hp_marker),
            Some(&SlotValue::Bool(true))
        );
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().template.defense, 77);
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().template.resistance, 88);
        assert_eq!(strict_diff(&expected, &actual), Ok(()));
    }

    #[test]
    fn custom_runner_minion_owner_death_matches_strict_diff_golden() {
        let mut builder = ExtensionRegistryBuilder::default();
        let minion_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "runner-linked-minion",
                "custom.runner_linked_minion",
                PlayerKindFlags::MINION | PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("runner minion kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
            ],
            registry,
        ));

        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "owner?0", minion_kind, 0, 4, 1),
        });
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(4, "owner?1", minion_kind, 0, 4, 1),
        });
        runtime.flush_effects().expect("minion spawns should emit updates");
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(0),
            amount: 10,
        });

        let frame = runtime.flush_effects().expect("owner death should cleanup linked minions");
        let outcome = RoundOutcome {
            action: None,
            frame: Some(frame),
            winner_team: runtime.world.sync_winner(&runtime.entities),
        };
        let actual = NormalizedOutcome::from_runtime(&runtime, &outcome);
        let expected = NormalizedOutcome {
            winner_team: Some(1),
            round: 0,
            total_score: 10,
            rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::default(),
            entity_ids: vec![1, 2, 3, 4],
            teams: vec![0, 1, 0, 0],
            hp: vec![0, 10, 0, 0],
            defense: vec![0, 0, 0, 0],
            resistance: vec![0, 0, 0, 0],
            alive: vec![false, true, false, false],
            round_order: vec![0, 1],
            flat_alive: vec![1],
            team_alive: vec![Vec::new(), vec![1]],
            alive_group_count: 1,
            actions: Vec::new(),
            frames: vec![
                NormalizedUpdateFrame {
                    message: "[0]攻击[1]".to_owned(),
                    caster: 1,
                    target: 0,
                    targets: Vec::new(),
                    param: None,
                    score: 10,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
                NormalizedUpdateFrame {
                    message: "[1]消失了".to_owned(),
                    caster: 0,
                    target: 2,
                    targets: Vec::new(),
                    param: None,
                    score: 0,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
                NormalizedUpdateFrame {
                    message: "[1]消失了".to_owned(),
                    caster: 0,
                    target: 3,
                    targets: Vec::new(),
                    param: None,
                    score: 0,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
            ],
        };

        assert_eq!(strict_diff(&expected, &actual), Ok(()));
    }

    #[test]
    fn custom_runner_merge_matches_strict_diff_golden() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill_a = builder
            .register_skill("custom", "runner-a", "custom.runner_a", TargetPolicy::Enemy, SkillPriority(0))
            .expect("runner skill should register");
        let skill_b = builder
            .register_skill("custom", "runner-b", "custom.runner_b", TargetPolicy::Enemy, SkillPriority(1))
            .expect("runner skill should register");
        let skill_c = builder
            .register_skill("custom", "runner-c", "custom.runner_c", TargetPolicy::Enemy, SkillPriority(2))
            .expect("runner skill should register");
        let merge_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "runner-merge",
                "custom.runner_merge",
                PlayerKindFlags::NONE,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("runner merge kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "merge-owner", merge_kind, 0, 10, 3).with_skills([skill_a]),
                PlayerTemplate::new(2, "merge-target", 1, 10, 3).with_skills([skill_b, skill_c]),
            ],
            registry,
        ));

        runtime.effects.push(QueuedEffect::Merge {
            caster: EntityIdx(0),
            target: EntityIdx(1),
        });

        let frame = runtime.flush_effects().expect("runner merge should emit updates");
        let outcome = RoundOutcome {
            action: None,
            frame: Some(frame),
            winner_team: runtime.world.sync_winner(&runtime.entities),
        };
        let actual = NormalizedOutcome::from_runtime(&runtime, &outcome);
        let expected = NormalizedOutcome {
            winner_team: None,
            round: 0,
            total_score: 60,
            rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::default(),
            entity_ids: vec![1, 2],
            teams: vec![0, 1],
            hp: vec![10, 10],
            defense: vec![0, 0],
            resistance: vec![0, 0],
            alive: vec![true, true],
            round_order: vec![0, 1],
            flat_alive: vec![0, 1],
            team_alive: vec![vec![0], vec![1]],
            alive_group_count: 2,
            actions: Vec::new(),
            frames: vec![
                NormalizedUpdateFrame {
                    message: "[0][吞噬]了[1]".to_owned(),
                    caster: 0,
                    target: 1,
                    targets: Vec::new(),
                    param: None,
                    score: 60,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
                NormalizedUpdateFrame {
                    message: "[0]属性上升".to_owned(),
                    caster: 0,
                    target: 1,
                    targets: Vec::new(),
                    param: None,
                    score: 0,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
            ],
        };

        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().template.skills.skills(),
            &[skill_b, skill_c]
        );
        assert_eq!(strict_diff(&expected, &actual), Ok(()));
    }

    fn custom_marks_update(context: &mut EffectContext<'_>, effect: &CustomEffect) {
        let CustomEffectPayload::Text(message) = &effect.payload else {
            panic!("custom test effect expects text payload");
        };
        context.add_update(crate::engine::update::RunUpdate::new(
            message.clone(),
            effect.caster.0 as usize,
            effect.target.unwrap().0 as usize,
            0,
        ));
    }

    fn custom_spawns_nested_damage(context: &mut EffectContext<'_>, effect: &CustomEffect) {
        let CustomEffectPayload::Int(amount) = effect.payload else {
            panic!("custom test effect expects int payload");
        };
        context.push_nested(QueuedEffect::Damage {
            caster: effect.caster,
            target: effect.target.expect("custom test effect needs target"),
            amount,
        });
    }

    fn custom_spawns_nested_heal(context: &mut EffectContext<'_>, effect: &CustomEffect) {
        let CustomEffectPayload::Int(amount) = effect.payload else {
            panic!("custom test effect expects int payload");
        };
        context.push_nested(QueuedEffect::Heal {
            caster: effect.caster,
            target: effect.target.expect("custom test effect needs target"),
            amount,
        });
    }

    fn custom_rejects_cross_entity_read(context: &mut EffectContext<'_>, _: &CustomEffect) {
        assert_eq!(
            context.entity(EntityIdx(2)),
            Err(EffectContextError::MissingCapability(ExtensionCapability::ReadEnemies))
        );
        context.add_update(crate::engine::update::RunUpdate::new("read denied", 0, 0, 0));
    }

    fn custom_reads_cross_entity(context: &mut EffectContext<'_>, _: &CustomEffect) {
        let observed = context.entity(EntityIdx(2)).expect("capability should allow cross-entity read");
        context.add_update(crate::engine::update::RunUpdate::new(observed.template.name.clone(), 0, 2, 0));
    }

    fn custom_mutates_entity_slot(context: &mut EffectContext<'_>, effect: &CustomEffect) {
        let CustomEffectPayload::Int(slot) = effect.payload else {
            panic!("custom test effect expects entity slot id payload");
        };
        context
            .set_entity_slot(
                effect.target.expect("custom test effect needs target"),
                EntitySlotId(slot as u32),
                SlotValue::Bool(true),
            )
            .expect("capability should allow entity slot mutation");
        context.add_update(crate::engine::update::RunUpdate::new("slot set", 0, 0, 0));
    }

    fn custom_consumes_rng(context: &mut EffectContext<'_>, effect: &CustomEffect) {
        let CustomEffectPayload::Int(max) = effect.payload else {
            panic!("custom test effect expects rng max payload");
        };
        let value = context.rng_next_i32(max);
        let next_byte = context.rng_next_u8();
        context.add_update(crate::engine::update::RunUpdate::new(
            format!("rng:{value}:{next_byte}"),
            effect.caster.0 as usize,
            effect.target.unwrap().0 as usize,
            value as u32,
        ));
    }

    fn skill_marks_update(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
        context.add_update(crate::engine::update::RunUpdate::new(
            "skill mark",
            entry.owner.0 as usize,
            entry.owner.0 as usize,
            entry.skill_id.0,
        ));
    }

    fn skill_pushes_nested_damage(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        context.push_nested(QueuedEffect::Damage {
            caster: context.owner_idx(),
            target: EntityIdx(1),
            amount: 2,
        });
    }

    fn skill_bed2_template_slot_summon_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        let summon_template = match context
            .template_slot(TemplateSlotId(0))
            .expect("bed2 summon handler should read template slot")
        {
            Some(SlotValue::PlayerTemplate(template)) => template.as_ref().clone(),
            _ => panic!("bed2 summon handler expects PlayerTemplate payload"),
        };
        context.push_nested(QueuedEffect::Spawn {
            caster: context.owner_idx(),
            template: summon_template,
        });
    }

    fn skill_consumes_rng(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
        let value = context.rng_next_i32(10);
        let next_byte = context.rng_next_u8();
        context.add_update(crate::engine::update::RunUpdate::new(
            format!("skill-rng:{value}:{next_byte}"),
            entry.owner.0 as usize,
            entry.owner.0 as usize,
            value as u32,
        ));
    }

    fn skill_summon_recast_fixture_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        let owner = context.owner_idx();
        let remembered = context
            .owner()
            .and_then(|entity| entity.slots.get(EntitySlotId(0)))
            .and_then(|value| match value {
                SlotValue::U64(idx) => Some(EntityIdx(*idx as u32)),
                _ => None,
            });
        if let Some(summon) = remembered
            && context.entity(summon).is_ok_and(|entity| !entity.runtime.alive)
        {
            context.push_nested(QueuedEffect::Revive {
                caster: owner,
                target: summon,
                hp: 10,
            });
            return;
        }
        let summon_template = PlayerTemplate::with_kind(3, "summon", PlayerKindId(1), 0, 10, 1)
            .with_def_res(11, 22)
            .with_skills([SkillId(0)]);
        context.push_nested(QueuedEffect::Spawn {
            caster: owner,
            template: summon_template,
        });
        context
            .set_entity_slot(owner, EntitySlotId(0), SlotValue::U64(2))
            .expect("summon recast fixture should store spawned entity");
    }

    fn state_marks_update(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
        context.add_update(crate::engine::update::RunUpdate::new(
            "state mark",
            entry.owner.0 as usize,
            entry.owner.0 as usize,
            entry.legacy_order_key,
        ));
    }

    fn state_pushes_nested_heal(context: &mut StateContext<'_>, _: &StateHookPlanEntry) {
        context.push_nested(QueuedEffect::Heal {
            caster: context.owner_idx(),
            target: context.owner_idx(),
            amount: 2,
        });
    }

    fn state_consumes_rng(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
        let value = context.rng_next_i32(10);
        let next_byte = context.rng_next_u8();
        context.add_update(crate::engine::update::RunUpdate::new(
            format!("state-rng:{value}:{next_byte}"),
            entry.owner.0 as usize,
            entry.owner.0 as usize,
            value as u32,
        ));
    }

    fn render_first_message_replay(frame: &RuntimeFrame) -> Option<RenderedReplay> {
        Some(RenderedReplay::new(
            ReplayRendererId(0),
            frame.updates.updates.first()?.message.to_string(),
        ))
    }

    fn render_update_count_replay(frame: &RuntimeFrame) -> Option<RenderedReplay> {
        Some(RenderedReplay::new(
            ReplayRendererId(1),
            frame.updates.updates.len().to_string(),
        ))
    }

    fn render_first_message_show(frame: &RuntimeFrame) -> Option<RenderedShow> {
        Some(RenderedShow::new(
            ShowRendererId(0),
            frame.updates.updates.first()?.message.to_string(),
        ))
    }

    fn render_hp_marker_bar_show(frame: &RuntimeFrame) -> Option<RenderedShow> {
        let hp_report = frame.updates.updates.iter().find(|update| update.message == "[0]还剩[2]点血")?;
        Some(RenderedShow::new(
            ShowRendererId(0),
            format!(
                "hp-bar:actor={}:value={}:text={}",
                hp_report.caster,
                hp_report.param.unwrap_or(hp_report.score),
                hp_report.msg()
            ),
        ))
    }

    #[test]
    fn run_skill_hooks_dispatches_registered_skill_handlers() {
        let mut builder = ExtensionRegistryBuilder::default();
        let marker = builder
            .register_skill_with_hooks(
                "custom",
                "marker",
                "custom.marker",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([marker])],
            registry,
        ));
        runtime.set_skill_handler(marker, skill_marks_update);

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("skill handler should emit update");

        assert_eq!(frame.updates.updates[0].message, "skill mark");
        assert_eq!(frame.updates.updates[0].score, marker.0);
    }

    #[test]
    fn run_skill_hooks_exposes_controlled_rng_to_skill_handlers() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill = builder
            .register_skill_with_hooks(
                "custom",
                "rng-skill",
                "custom.rng_skill",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill])],
            registry,
        ));
        runtime.set_skill_handler(skill, skill_consumes_rng);
        let mut expected_rng = RC4::default();
        let expected_value = expected_rng.next_i32(10);
        let expected_byte = expected_rng.next_u8();

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("skill rng handler should emit update");

        assert_eq!(
            frame.updates.updates[0].message,
            format!("skill-rng:{expected_value}:{expected_byte}")
        );
        assert_eq!(frame.updates.updates[0].score, expected_value as u32);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    }

    #[test]
    fn run_skill_hooks_flushes_nested_effects() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill = builder
            .register_skill_with_hooks(
                "custom",
                "damage",
                "custom.damage",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_skill_handler(skill, skill_pushes_nested_damage);

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("nested damage should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 8);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[0].score, 2);
    }

    #[test]
    fn run_minimal_round_dispatches_pre_action_skill_before_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let marker = builder
            .register_skill_with_hooks(
                "custom",
                "marker",
                "custom.marker",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([marker]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_skill_handler(marker, skill_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("skill plus attack should emit update");

        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "skill mark");
        assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
    }

    #[test]
    fn run_minimal_round_flushes_pre_action_skill_effect_before_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill = builder
            .register_skill_with_hooks(
                "custom",
                "damage",
                "custom.damage",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_skill_handler(skill, skill_pushes_nested_damage);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("skill damage plus attack should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 5);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].score, 2);
        assert_eq!(frame.updates.updates[1].score, 3);
    }

    #[test]
    fn run_minimal_round_dispatches_damage_skill_hooks_around_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let pre_damage = builder
            .register_skill_with_hooks(
                "custom",
                "pre-damage",
                "custom.pre_damage",
                ProcMask::PRE_DAMAGE,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("pre-damage skill should register");
        let post_damage = builder
            .register_skill_with_hooks(
                "custom",
                "post-damage",
                "custom.post_damage",
                ProcMask::POST_DAMAGE,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("post-damage skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([pre_damage, post_damage]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_skill_handler(pre_damage, skill_marks_update);
        runtime.set_skill_handler(post_damage, skill_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("damage skill hooks plus attack should emit update");

        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "skill mark");
        assert_eq!(frame.updates.updates[0].score, pre_damage.0);
        assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[2].message, "skill mark");
        assert_eq!(frame.updates.updates[2].score, post_damage.0);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
    }

    #[test]
    fn run_minimal_round_dispatches_post_action_skill_before_state() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill = builder
            .register_skill_with_hooks(
                "custom",
                "post-action-skill",
                "custom.post_action_skill",
                ProcMask::POST_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("post-action skill should register");
        let state = builder
            .register_state(
                "custom",
                "post-action-state",
                "custom.post_action_state",
                ProcMask::POST_ACTION,
                SkillPriority(0),
            )
            .expect("post-action state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 55,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
        });
        runtime.set_skill_handler(skill, skill_marks_update);
        runtime.set_state_handler(state, state_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("post-action skill and state plus attack should emit update");

        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "skill mark");
        assert_eq!(frame.updates.updates[1].score, skill.0);
        assert_eq!(frame.updates.updates[2].message, "state mark");
        assert_eq!(frame.updates.updates[2].score, 55);
    }

    #[test]
    fn run_minimal_round_dispatches_post_action_state_after_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let state = builder
            .register_state("custom", "marker", "custom.marker", ProcMask::POST_ACTION, SkillPriority(0))
            .expect("state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 77,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
        });
        runtime.set_state_handler(state, state_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("attack plus state hook should emit update");

        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "state mark");
        assert_eq!(frame.updates.updates[1].score, 77);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
    }

    #[test]
    fn run_minimal_round_flushes_post_action_state_effect_after_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let state = builder
            .register_state("custom", "regen", "custom.regen", ProcMask::POST_ACTION, SkillPriority(0))
            .expect("state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 4;
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 88,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
        });
        runtime.set_state_handler(state, state_pushes_nested_heal);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("attack plus state heal should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "[1]回复体力[2]点");
        assert_eq!(frame.updates.updates[1].score, 2);
    }

    #[test]
    fn run_minimal_round_dispatches_damage_state_hooks_around_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let pre_damage = builder
            .register_state(
                "custom",
                "pre-damage",
                "custom.pre_damage",
                ProcMask::PRE_DAMAGE,
                SkillPriority(0),
            )
            .expect("pre-damage state should register");
        let post_damage = builder
            .register_state(
                "custom",
                "post-damage",
                "custom.post_damage",
                ProcMask::POST_DAMAGE,
                SkillPriority(0),
            )
            .expect("post-damage state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        {
            let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
            store.add_entry(StateEntry {
                legacy_order_key: 11,
                extension_state_id: Some(pre_damage),
                hook_mask: ProcMask::PRE_DAMAGE,
                priority: SkillPriority(0),
                registration_order: RegistrationOrder(0),
            });
            store.add_entry(StateEntry {
                legacy_order_key: 22,
                extension_state_id: Some(post_damage),
                hook_mask: ProcMask::POST_DAMAGE,
                priority: SkillPriority(0),
                registration_order: RegistrationOrder(1),
            });
        }
        runtime.set_state_handler(pre_damage, state_marks_update);
        runtime.set_state_handler(post_damage, state_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("damage state hooks plus attack should emit update");

        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "state mark");
        assert_eq!(frame.updates.updates[0].score, 11);
        assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[2].message, "state mark");
        assert_eq!(frame.updates.updates[2].score, 22);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
    }

    #[test]
    fn run_minimal_round_flushes_post_damage_state_effect_before_post_action() {
        let mut builder = ExtensionRegistryBuilder::default();
        let post_damage = builder
            .register_state(
                "custom",
                "post-damage-regen",
                "custom.post_damage_regen",
                ProcMask::POST_DAMAGE,
                SkillPriority(0),
            )
            .expect("post-damage state should register");
        let post_action = builder
            .register_state(
                "custom",
                "post-action-marker",
                "custom.post_action_marker",
                ProcMask::POST_ACTION,
                SkillPriority(0),
            )
            .expect("post-action state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 4;
        {
            let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
            store.add_entry(StateEntry {
                legacy_order_key: 33,
                extension_state_id: Some(post_damage),
                hook_mask: ProcMask::POST_DAMAGE,
                priority: SkillPriority(0),
                registration_order: RegistrationOrder(0),
            });
            store.add_entry(StateEntry {
                legacy_order_key: 44,
                extension_state_id: Some(post_action),
                hook_mask: ProcMask::POST_ACTION,
                priority: SkillPriority(0),
                registration_order: RegistrationOrder(1),
            });
        }
        runtime.set_state_handler(post_damage, state_pushes_nested_heal);
        runtime.set_state_handler(post_action, state_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("post-damage effect plus post-action state should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "[1]回复体力[2]点");
        assert_eq!(frame.updates.updates[2].message, "state mark");
        assert_eq!(frame.updates.updates[2].score, 44);
    }

    #[test]
    fn run_state_hooks_dispatches_registered_state_handlers() {
        let mut builder = ExtensionRegistryBuilder::default();
        let state = builder
            .register_state("custom", "burning", "custom.burning", ProcMask::POST_ACTION, SkillPriority(0))
            .expect("state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 42,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
        });
        runtime.set_state_handler(state, state_marks_update);

        let frame = runtime
            .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
            .expect("state handler should emit update");

        assert_eq!(frame.updates.updates[0].message, "state mark");
        assert_eq!(frame.updates.updates[0].score, 42);
    }

    #[test]
    fn run_state_hooks_exposes_controlled_rng_to_state_handlers() {
        let mut builder = ExtensionRegistryBuilder::default();
        let state = builder
            .register_state(
                "custom",
                "rng-state",
                "custom.rng_state",
                ProcMask::POST_ACTION,
                SkillPriority(0),
            )
            .expect("state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 88,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
        });
        runtime.set_state_handler(state, state_consumes_rng);
        let mut expected_rng = RC4::default();
        let expected_value = expected_rng.next_i32(10);
        let expected_byte = expected_rng.next_u8();

        let frame = runtime
            .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
            .expect("state rng handler should emit update");

        assert_eq!(
            frame.updates.updates[0].message,
            format!("state-rng:{expected_value}:{expected_byte}")
        );
        assert_eq!(frame.updates.updates[0].score, expected_value as u32);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    }

    #[test]
    fn run_state_hooks_flushes_nested_effects_and_skips_legacy_entries() {
        let mut builder = ExtensionRegistryBuilder::default();
        let state = builder
            .register_state("custom", "regen", "custom.regen", ProcMask::POST_ACTION, SkillPriority(0))
            .expect("state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 4;
        {
            let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
            store.add_legacy_key(11);
            store.add_entry(StateEntry {
                legacy_order_key: 22,
                extension_state_id: Some(state),
                hook_mask: ProcMask::POST_ACTION,
                priority: SkillPriority(0),
                registration_order: RegistrationOrder(1),
            });
        }
        runtime.set_state_handler(state, state_pushes_nested_heal);

        let frame = runtime
            .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
            .expect("state heal should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
        assert_eq!(frame.updates.updates.len(), 1);
        assert_eq!(frame.updates.updates[0].message, "[1]回复体力[2]点");
        assert_eq!(frame.updates.updates[0].score, 2);
    }

    #[test]
    fn flush_effects_dispatches_custom_handlers() {
        let mut builder = ExtensionRegistryBuilder::default();
        let marker = builder
            .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(0))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_effect_handler(marker, custom_marks_update);

        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            marker,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Text("custom mark".to_owned()),
        )));

        let frame = runtime.flush_effects().expect("custom handler should emit update");
        assert_eq!(frame.updates.updates[0].message, "custom mark");
    }

    #[test]
    fn flush_effects_exposes_controlled_rng_to_custom_handlers() {
        let mut builder = ExtensionRegistryBuilder::default();
        let rng_handler = builder
            .register_effect_handler("custom", "rng", "custom.rng", SkillPriority(0))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_effect_handler(rng_handler, custom_consumes_rng);
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            rng_handler,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Int(10),
        )));
        let mut expected_rng = RC4::default();
        let expected_value = expected_rng.next_i32(10);
        let expected_byte = expected_rng.next_u8();

        let frame = runtime.flush_effects().expect("custom rng handler should emit update");

        assert_eq!(
            frame.updates.updates[0].message,
            format!("rng:{expected_value}:{expected_byte}")
        );
        assert_eq!(frame.updates.updates[0].score, expected_value as u32);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    }

    #[test]
    fn flush_effects_runs_nested_custom_effect_before_older_siblings() {
        let mut builder = ExtensionRegistryBuilder::default();
        let nested_damage = builder
            .register_effect_handler("custom", "nested-damage", "custom.nested_damage", SkillPriority(0))
            .expect("handler should register");
        let marker = builder
            .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(1))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_effect_handler(nested_damage, custom_spawns_nested_damage);
        runtime.set_effect_handler(marker, custom_marks_update);

        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            nested_damage,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Int(4),
        )));
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            marker,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Text("after nested".to_owned()),
        )));

        let frame = runtime.flush_effects().expect("nested damage should emit update");
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 6);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "after nested");
    }

    #[test]
    fn flush_effects_applies_heal_without_exceeding_max_hp() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 4;
        runtime.effects.push(QueuedEffect::Heal {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            amount: 20,
        });

        let frame = runtime.flush_effects().expect("heal should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10);
        assert_eq!(frame.updates.updates[0].message, "[1]回复体力[2]点");
    }

    #[test]
    fn flush_effects_readds_healed_dead_target_to_alive_views() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 4, 3));
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            amount: 4,
        });
        runtime.flush_effects().expect("lethal damage should emit update");
        runtime.effects.push(QueuedEffect::Heal {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            amount: 2,
        });

        runtime.flush_effects().expect("heal should emit update");

        assert!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 2);
        assert_eq!(runtime.world.team_alive(1), Some([EntityIdx(1)].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(1)]);
        assert_eq!(runtime.world.alive_group_count(), 2);
        assert_eq!(
            runtime.world.first_alive_enemy(EntityIdx(0), &runtime.entities),
            Some(EntityIdx(1))
        );
    }

    #[test]
    fn flush_effects_spawns_entity_and_adds_it_to_round_order() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::new(3, "summoned", 0, 5, 2),
        });

        let frame = runtime.flush_effects().expect("spawn should emit update");

        assert_eq!(runtime.entities.len(), 3);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().template.name, "summoned");
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 5);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.owner, EntityIdx(0));
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.root_owner, EntityIdx(0));
        assert_eq!(runtime.world.round_order(), &[EntityIdx(0), EntityIdx(1), EntityIdx(2)]);
        assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0), EntityIdx(2)].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(2), EntityIdx(1)]);
        assert_eq!(frame.updates.updates[0].message, "出现一个新的[1]");
    }

    #[test]
    fn spawned_entity_can_be_selected_by_scheduler() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            ExtensionRegistry::default(),
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::new(2, "enemy", 1, 8, 4),
        });
        runtime.flush_effects().expect("spawn should emit update");

        assert_eq!(runtime.world.sync_winner(&runtime.entities), None);
        assert_eq!(
            runtime.scheduler.select_minimal_action(&mut runtime.world, &runtime.entities),
            Some(ActionPlan {
                actor: EntityIdx(0),
                target: EntityIdx(1),
                amount: 3,
            })
        );
    }

    #[test]
    fn flush_effects_adds_and_clears_state_entries() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        let state = StateEntry {
            legacy_order_key: 77,
            extension_state_id: Some(StateId(1)),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(5),
            registration_order: RegistrationOrder(2),
        };

        runtime.effects.push(QueuedEffect::AddState {
            target: EntityIdx(1),
            state,
        });
        let add_frame = runtime.flush_effects().expect("add state should emit update");
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.entry(77), Some(&state));
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.generation(), 1);
        assert_eq!(add_frame.updates.updates[0].message, "[1]状态改变");

        runtime.effects.push(QueuedEffect::ClearState {
            target: EntityIdx(1),
            legacy_order_key: 77,
        });
        let clear_frame = runtime.flush_effects().expect("clear state should emit update");
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.entry(77), None);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.generation(), 2);
        assert_eq!(clear_frame.updates.updates[0].message, "[1]状态解除");
    }

    #[test]
    fn flush_effects_runs_nested_heal_before_older_siblings() {
        let mut builder = ExtensionRegistryBuilder::default();
        let nested_heal = builder
            .register_effect_handler("custom", "nested-heal", "custom.nested_heal", SkillPriority(0))
            .expect("handler should register");
        let marker = builder
            .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(1))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 3;
        runtime.set_effect_handler(nested_heal, custom_spawns_nested_heal);
        runtime.set_effect_handler(marker, custom_marks_update);

        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            nested_heal,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Int(4),
        )));
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            marker,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Text("after heal".to_owned()),
        )));

        let frame = runtime.flush_effects().expect("nested heal should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
        assert_eq!(frame.updates.updates[0].message, "[1]回复体力[2]点");
        assert_eq!(frame.updates.updates[1].message, "after heal");
    }

    #[test]
    fn flush_effects_revives_dead_entity_with_capped_hp() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.world.remove_round_actor(EntityIdx(1));
        let target = runtime.entities.get_mut(EntityIdx(1)).unwrap();
        target.runtime.hp = 0;
        target.runtime.alive = false;
        runtime.effects.push(QueuedEffect::Revive {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            hp: 20,
        });

        let frame = runtime.flush_effects().expect("revive should emit update");

        let target = runtime.entities.get(EntityIdx(1)).unwrap();
        assert_eq!(target.runtime.hp, 10);
        assert!(target.runtime.alive);
        assert_eq!(runtime.world.round_order(), &[EntityIdx(0), EntityIdx(1)]);
        assert_eq!(runtime.world.team_alive(1), Some([EntityIdx(1)].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(1)]);
        assert_eq!(frame.updates.updates[0].message, "[1][复活]了");
    }

    #[test]
    fn flush_effects_removes_entity_from_alive_set() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.effects.push(QueuedEffect::Remove {
            caster: EntityIdx(0),
            target: EntityIdx(1),
        });

        let frame = runtime.flush_effects().expect("remove should emit update");

        let target = runtime.entities.get(EntityIdx(1)).unwrap();
        assert_eq!(target.runtime.hp, 0);
        assert!(!target.runtime.alive);
        assert_eq!(runtime.world.round_order(), &[EntityIdx(0)]);
        assert_eq!(runtime.world.team_alive(1), Some([].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0)]);
        assert_eq!(frame.updates.updates[0].message, "[1]消失了");
    }

    #[test]
    fn flush_effects_emits_replay_effect_without_state_mutation() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.effects.push(QueuedEffect::Replay {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            message: "custom replay".to_owned(),
            score: 7,
        });

        let frame = runtime.flush_effects().expect("replay should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10);
        assert_eq!(frame.updates.updates[0].message, "custom replay");
        assert_eq!(frame.updates.updates[0].score, 7);
    }

    #[test]
    fn flush_effects_merges_fixed_lane_skill_loadout() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill_a = builder
            .register_skill("custom", "a", "custom.a", TargetPolicy::Enemy, SkillPriority(0))
            .expect("skill should register");
        let skill_b = builder
            .register_skill("custom", "b", "custom.b", TargetPolicy::Enemy, SkillPriority(1))
            .expect("skill should register");
        let skill_c = builder
            .register_skill("custom", "c", "custom.c", TargetPolicy::Enemy, SkillPriority(2))
            .expect("skill should register");
        let merge_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "merge",
                "custom.merge",
                PlayerKindFlags::default(),
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("merge kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "left", merge_kind, 0, 10, 3).with_skills([skill_a]),
                PlayerTemplate::new(2, "right", 1, 10, 3).with_skills([skill_b, skill_c]),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Merge {
            caster: EntityIdx(0),
            target: EntityIdx(1),
        });

        let frame = runtime.flush_effects().expect("merge should emit update");

        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().template.skills.skills(),
            &[skill_b, skill_c]
        );
        assert_eq!(frame.updates.updates[0].message, "[0][吞噬]了[1]");
        assert_eq!(frame.updates.updates[0].score, 60);
        assert_eq!(frame.updates.updates[1].message, "[0]属性上升");
        assert_eq!(frame.updates.updates[1].score, 0);
    }

    #[test]
    fn flush_effects_merge_drops_unmapped_skills_when_policy_requires() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill_a = builder
            .register_skill("custom", "a", "custom.a", TargetPolicy::Enemy, SkillPriority(0))
            .expect("skill should register");
        let skill_b = builder
            .register_skill("custom", "b", "custom.b", TargetPolicy::Enemy, SkillPriority(1))
            .expect("skill should register");
        let skill_c = builder
            .register_skill("custom", "c", "custom.c", TargetPolicy::Enemy, SkillPriority(2))
            .expect("skill should register");
        let merge_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "merge",
                "custom.merge",
                PlayerKindFlags::default(),
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::DropUnmappedSkills,
                    inherit_owner_def_res: false,
                },
            )
            .expect("merge kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "left", merge_kind, 0, 10, 3).with_skills([skill_a]),
                PlayerTemplate::new(2, "right", 1, 10, 3).with_skills([skill_b, skill_c]),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Merge {
            caster: EntityIdx(0),
            target: EntityIdx(1),
        });

        runtime.flush_effects().expect("merge should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.skills(), &[skill_b]);
    }

    #[test]
    fn flush_effects_panics_on_unknown_damage_target() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(0),
            target: EntityIdx(99),
            amount: 1,
        });

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

        assert!(result.is_err());
    }

    fn assert_effect_panics(effect: QueuedEffect) {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.effects.push(effect);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

        assert!(result.is_err());
    }

    fn dummy_state_entry() -> StateEntry {
        StateEntry {
            legacy_order_key: 999,
            extension_state_id: None,
            hook_mask: ProcMask::NONE,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
        }
    }

    #[test]
    fn flush_effects_panics_on_unknown_effect_entities() {
        assert_effect_panics(QueuedEffect::Damage {
            caster: EntityIdx(99),
            target: EntityIdx(1),
            amount: 1,
        });
        assert_effect_panics(QueuedEffect::Heal {
            caster: EntityIdx(99),
            target: EntityIdx(1),
            amount: 1,
        });
        assert_effect_panics(QueuedEffect::Heal {
            caster: EntityIdx(0),
            target: EntityIdx(99),
            amount: 1,
        });
        assert_effect_panics(QueuedEffect::Spawn {
            caster: EntityIdx(99),
            template: PlayerTemplate::new(3, "ghost", 1, 1, 0),
        });
        assert_effect_panics(QueuedEffect::AddState {
            target: EntityIdx(99),
            state: dummy_state_entry(),
        });
        assert_effect_panics(QueuedEffect::ClearState {
            target: EntityIdx(99),
            legacy_order_key: 999,
        });
        assert_effect_panics(QueuedEffect::Revive {
            caster: EntityIdx(99),
            target: EntityIdx(1),
            hp: 1,
        });
        assert_effect_panics(QueuedEffect::Revive {
            caster: EntityIdx(0),
            target: EntityIdx(99),
            hp: 1,
        });
        assert_effect_panics(QueuedEffect::Remove {
            caster: EntityIdx(99),
            target: EntityIdx(1),
        });
        assert_effect_panics(QueuedEffect::Remove {
            caster: EntityIdx(0),
            target: EntityIdx(99),
        });
        assert_effect_panics(QueuedEffect::Merge {
            caster: EntityIdx(99),
            target: EntityIdx(1),
        });
        assert_effect_panics(QueuedEffect::Merge {
            caster: EntityIdx(0),
            target: EntityIdx(99),
        });
        assert_effect_panics(QueuedEffect::Replay {
            caster: EntityIdx(99),
            target: EntityIdx(1),
            message: "bad caster".to_owned(),
            score: 0,
        });
        assert_effect_panics(QueuedEffect::Replay {
            caster: EntityIdx(0),
            target: EntityIdx(99),
            message: "bad target".to_owned(),
            score: 0,
        });
    }

    #[test]
    fn flush_effects_panics_on_unknown_custom_effect_entities() {
        let mut builder = ExtensionRegistryBuilder::default();
        let handler = builder
            .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(0))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_effect_handler(handler, custom_marks_update);
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            handler,
            EntityIdx(99),
            Some(EntityIdx(1)),
            CustomEffectPayload::Text("bad caster".to_owned()),
        )));

        let bad_caster = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

        assert!(bad_caster.is_err());

        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            handler,
            EntityIdx(0),
            Some(EntityIdx(99)),
            CustomEffectPayload::Text("bad target".to_owned()),
        )));

        let bad_target = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

        assert!(bad_target.is_err());
    }

    #[test]
    fn custom_context_restricts_cross_entity_reads_by_capability() {
        let mut builder = ExtensionRegistryBuilder::default();
        let reader = builder
            .register_effect_handler("custom", "reader", "custom.reader", SkillPriority(0))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
                PlayerTemplate::new(3, "third", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_effect_handler(reader, custom_rejects_cross_entity_read);
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            reader,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::None,
        )));
        let denied = runtime.flush_effects().expect("denied read handler should emit update");
        assert_eq!(denied.updates.updates[0].message, "read denied");

        runtime.set_effect_handler_with_capabilities(reader, custom_reads_cross_entity, &[ExtensionCapability::ReadEnemies]);
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            reader,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::None,
        )));
        let allowed = runtime.flush_effects().expect("allowed read handler should emit update");
        assert_eq!(allowed.updates.updates[0].message, "third");
    }

    #[test]
    fn custom_context_requires_capability_for_entity_slot_mutation() {
        let mut builder = ExtensionRegistryBuilder::default();
        let slot = builder
            .reserve_entity_slot("custom", "flag", "custom.flag")
            .expect("entity slot should reserve");
        let mutator = builder
            .register_effect_handler("custom", "mutator", "custom.mutator", SkillPriority(0))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_effect_handler_with_capabilities(
            mutator,
            custom_mutates_entity_slot,
            &[ExtensionCapability::MutateEntitySlots],
        );
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            mutator,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Int(slot.0 as i32),
        )));

        let frame = runtime.flush_effects().expect("slot mutation should emit update");

        assert_eq!(frame.updates.updates[0].message, "slot set");
        assert_eq!(
            runtime.entities.get(EntityIdx(1)).unwrap().slots.get(slot),
            Some(&SlotValue::Bool(true))
        );
    }

    #[test]
    fn runtime_dispatches_replay_renderers_in_registry_order() {
        let mut builder = ExtensionRegistryBuilder::default();
        let late = builder
            .register_replay_renderer("custom", "late", "custom.late_replay", SkillPriority(10))
            .expect("late replay renderer should register");
        let early = builder
            .register_replay_renderer("custom", "early", "custom.early_replay", SkillPriority(1))
            .expect("early replay renderer should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.set_replay_renderer(late, render_update_count_replay);
        runtime.set_replay_renderer(early, render_first_message_replay);
        let frame = RuntimeFrame::single_damage(0, 0, 3);

        let rendered = runtime.render_replay_frame(&frame);

        assert_eq!(
            rendered,
            vec![
                RenderedReplay::new(ReplayRendererId(0), "[0]攻击[1]"),
                RenderedReplay::new(ReplayRendererId(1), "1")
            ]
        );
    }

    #[test]
    fn runtime_dispatches_show_renderers_in_registry_order() {
        let mut builder = ExtensionRegistryBuilder::default();
        let show = builder
            .register_show_renderer("custom", "show", "custom.show", SkillPriority(0))
            .expect("show renderer should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.set_show_renderer(show, render_first_message_show);
        let frame = RuntimeFrame::single_damage(0, 0, 3);

        let rendered = runtime.render_show_frame(&frame);

        assert_eq!(rendered, vec![RenderedShow::new(ShowRendererId(0), "[0]攻击[1]")]);
    }

    #[test]
    fn runtime_dispatches_hp_marker_show_renderer_golden() {
        let mut builder = ExtensionRegistryBuilder::default();
        let show = builder
            .register_show_renderer("custom", "hp-marker", "custom.hp_marker.show", SkillPriority(0))
            .expect("hp marker show renderer should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.set_show_renderer(show, render_hp_marker_bar_show);
        let mut updates = crate::engine::update::RunUpdates::new();
        let mut hp_report = RuntimeFrame::replay_update(0, 0, "[0]还剩[2]点血", 0);
        hp_report.param = Some(87);
        updates.add(hp_report);
        let frame = RuntimeFrame { updates };

        let rendered = runtime.render_show_frame(&frame);

        assert_eq!(
            rendered,
            vec![RenderedShow::new(ShowRendererId(0), "hp-bar:actor=0:value=87:text=0还剩87点血")]
        );
    }

    #[test]
    fn runtime_frame_renders_core_replay_and_show_golden() {
        let mut frame = RuntimeFrame::single_damage(0, 1, 3);
        frame.updates.add(RuntimeFrame::replay_update(0, 1, "[0]属性上升", 0));

        assert_eq!(
            frame.render_core_replay(),
            vec![
                CoreReplayEvent {
                    message: "[0]攻击[1]".to_owned(),
                    caster: 0,
                    target: 1,
                    targets: Vec::new(),
                    param: None,
                    score: 3,
                },
                CoreReplayEvent {
                    message: "[0]属性上升".to_owned(),
                    caster: 0,
                    target: 1,
                    targets: Vec::new(),
                    param: None,
                    score: 0,
                },
            ]
        );
        assert_eq!(
            frame.render_core_show(),
            vec![
                CoreShowEvent {
                    text: "0攻击1".to_owned(),
                    score: 3,
                },
                CoreShowEvent {
                    text: "0属性上升".to_owned(),
                    score: 0,
                },
            ]
        );
    }

    #[test]
    fn runtime_frame_renders_hp_marker_core_show_golden() {
        let mut updates = crate::engine::update::RunUpdates::new();
        let mut hp_report = RuntimeFrame::replay_update(0, 0, "[0]还剩[2]点血", 0);
        hp_report.param = Some(87);
        updates.add(hp_report);
        let frame = RuntimeFrame { updates };

        assert_eq!(
            frame.render_core_replay(),
            vec![CoreReplayEvent {
                message: "[0]还剩[2]点血".to_owned(),
                caster: 0,
                target: 0,
                targets: Vec::new(),
                param: Some(87),
                score: 0,
            }]
        );
        assert_eq!(
            frame.render_core_show(),
            vec![CoreShowEvent {
                text: "0还剩87点血".to_owned(),
                score: 0,
            }]
        );
    }
}

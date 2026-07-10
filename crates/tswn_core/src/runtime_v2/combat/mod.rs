use super::*;

mod attack;
mod defense;
mod effects;
mod infection;
mod lifecycle;
mod passive;
mod round;
mod skills_control;
mod skills_debuff;
mod skills_lifecycle;
mod skills_offense;
mod skills_team;

#[derive(Debug, Clone)]
pub struct RoundOutcome {
    pub action: Option<ActionPlan>,
    pub frame: Option<RuntimeFrame>,
    pub winner_team: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedBuiltinSkill {
    pub skill: BuiltinActiveSkill,
    pub fixed_lane: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedBuiltinSkillAction {
    pub selected: SelectedBuiltinSkill,
    pub targets: Vec<EntityIdx>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlainAttackOnDamage {
    None,
    Absorb,
    Berserk,
    Curse,
    Poison,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedPlainAction {
    BasicAttack {
        target: EntityIdx,
        use_magic: bool,
        amount: i32,
    },
    ForcedAttack {
        target: EntityIdx,
        amount: i32,
    },
    Saitama {
        target: Option<EntityIdx>,
    },
    BuiltinSkill(PreparedBuiltinSkillAction),
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

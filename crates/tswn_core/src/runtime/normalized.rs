use crate::rc4::RC4;
use crate::runtime::update::{RunUpdates, UpdateType};

use super::{CombatRuntime, RoundOutcome};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NormalizedRngCheckpoint {
    pub i: u32,
    pub j: u32,
    #[cfg(not(feature = "no_debug"))]
    pub byte_count: u64,
}

impl NormalizedRngCheckpoint {
    pub fn from_rc4(rng: &RC4) -> Self {
        Self {
            i: rng.i,
            j: rng.j,
            #[cfg(not(feature = "no_debug"))]
            byte_count: rng.byte_count,
        }
    }

    pub fn from_runtime(runtime: &CombatRuntime) -> Self { Self::from_rc4(&runtime.rng) }

    pub fn after_next_u8(count: usize) -> Self {
        let mut rng = RC4::default();
        for _ in 0..count {
            let _ = rng.next_u8();
        }
        Self::from_rc4(&rng)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedActionBoundary {
    pub round: u64,
    pub actor: usize,
    pub target: usize,
    pub amount: i32,
}

impl NormalizedActionBoundary {
    pub fn from_outcome(runtime: &CombatRuntime, outcome: &RoundOutcome) -> Vec<Self> {
        outcome
            .action
            .map(|action| {
                vec![Self {
                    round: runtime.round,
                    actor: action.actor.0 as usize,
                    target: action.target.0 as usize,
                    amount: action.amount,
                }]
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedUpdateFrame {
    pub message: String,
    pub caster: usize,
    pub target: usize,
    pub targets: Vec<usize>,
    pub param: Option<u32>,
    pub score: u32,
    pub delay0: i32,
    pub delay1: i32,
    pub update_type: UpdateType,
}

impl NormalizedUpdateFrame {
    pub fn from_updates(updates: &RunUpdates) -> Vec<Self> {
        updates
            .updates
            .iter()
            .map(|update| Self {
                message: update.message.to_string(),
                caster: update.caster,
                target: update.target,
                targets: update.targets.iter().copied().collect(),
                param: update.param,
                score: update.score,
                delay0: update.delay0,
                delay1: update.delay1,
                update_type: update.update_type,
            })
            .collect()
    }

    pub fn from_outcome(outcome: &RoundOutcome) -> Vec<Self> {
        outcome.frame.as_ref().map(|frame| Self::from_updates(&frame.updates)).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedOutcome {
    pub winner_team: Option<usize>,
    pub round: u64,
    pub total_score: u64,
    pub rng: NormalizedRngCheckpoint,
    pub entity_ids: Vec<usize>,
    pub teams: Vec<usize>,
    pub hp: Vec<i32>,
    pub magic_point: Vec<i32>,
    pub defense: Vec<i32>,
    pub resistance: Vec<i32>,
    pub alive: Vec<bool>,
    pub round_order: Vec<usize>,
    pub flat_alive: Vec<usize>,
    pub team_alive: Vec<Vec<usize>>,
    pub alive_group_count: usize,
    pub actions: Vec<NormalizedActionBoundary>,
    pub frames: Vec<NormalizedUpdateFrame>,
}

impl NormalizedOutcome {
    pub fn from_runtime(runtime: &CombatRuntime, outcome: &RoundOutcome) -> Self {
        let team_count = runtime
            .entities
            .iter()
            .map(|(_, entity)| entity.runtime.team)
            .max()
            .map_or(0, |team| team + 1);
        Self {
            winner_team: outcome.winner_team,
            round: runtime.round,
            total_score: outcome.frame.as_ref().map_or(0, |frame| {
                frame.updates.updates.iter().map(|update| u64::from(update.score)).sum()
            }),
            rng: NormalizedRngCheckpoint::from_runtime(runtime),
            entity_ids: runtime.entities.iter().map(|(_, entity)| entity.template.id).collect(),
            teams: runtime.entities.iter().map(|(_, entity)| entity.runtime.team).collect(),
            hp: runtime.entities.iter().map(|(_, entity)| entity.runtime.hp).collect(),
            magic_point: runtime.entities.iter().map(|(_, entity)| entity.runtime.magic_point).collect(),
            defense: runtime.entities.iter().map(|(_, entity)| entity.runtime.defense).collect(),
            resistance: runtime.entities.iter().map(|(_, entity)| entity.runtime.resistance).collect(),
            alive: runtime.entities.iter().map(|(_, entity)| entity.runtime.alive).collect(),
            round_order: runtime.world.round_order().iter().map(|idx| idx.0 as usize).collect(),
            flat_alive: runtime.world.flat_alive().iter().map(|idx| idx.0 as usize).collect(),
            team_alive: (0..team_count)
                .map(|team| {
                    runtime
                        .world
                        .team_alive(team)
                        .unwrap_or_default()
                        .iter()
                        .map(|idx| idx.0 as usize)
                        .collect()
                })
                .collect(),
            alive_group_count: runtime.world.alive_group_count(),
            actions: NormalizedActionBoundary::from_outcome(runtime, outcome),
            frames: NormalizedUpdateFrame::from_outcome(outcome),
        }
    }
}

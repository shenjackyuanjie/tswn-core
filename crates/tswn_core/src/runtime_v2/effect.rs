use crate::engine::update::{RunUpdate, RunUpdates};
use crate::runtime_v2::entity::EntityIdx;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueuedEffect {
    Damage {
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
    },
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EffectQueue {
    effects: Vec<QueuedEffect>,
}

impl EffectQueue {
    pub fn push(&mut self, effect: QueuedEffect) { self.effects.push(effect); }

    pub fn pop_next(&mut self) -> Option<QueuedEffect> { self.effects.pop() }

    pub fn len(&self) -> usize { self.effects.len() }

    pub fn is_empty(&self) -> bool { self.effects.is_empty() }

    pub fn clear(&mut self) { self.effects.clear(); }
}

#[derive(Debug, Clone)]
pub struct RuntimeFrame {
    pub updates: RunUpdates,
}

impl RuntimeFrame {
    pub fn single_damage(caster: usize, target: usize, amount: i32) -> Self {
        let mut updates = RunUpdates::new();
        updates.add(RunUpdate::new("[0]攻击[1]", caster, target, amount.max(0) as u32));
        Self { updates }
    }
}

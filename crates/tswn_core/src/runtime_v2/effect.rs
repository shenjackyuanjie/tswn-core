use crate::engine::update::{RunUpdate, RunUpdates};
use crate::runtime_v2::entity::EntityIdx;
use std::collections::VecDeque;

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
    effects: VecDeque<QueuedEffect>,
}

impl EffectQueue {
    pub fn push(&mut self, effect: QueuedEffect) { self.effects.push_back(effect); }

    pub fn push_nested(&mut self, effect: QueuedEffect) { self.effects.push_front(effect); }

    pub fn pop_next(&mut self) -> Option<QueuedEffect> { self.effects.pop_front() }

    pub fn len(&self) -> usize { self.effects.len() }

    pub fn is_empty(&self) -> bool { self.effects.is_empty() }

    pub fn clear(&mut self) { self.effects.clear(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn damage(amount: i32) -> QueuedEffect {
        QueuedEffect::Damage {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            amount,
        }
    }

    #[test]
    fn effect_queue_pops_root_effects_in_batch_order() {
        let mut queue = EffectQueue::default();
        queue.push(damage(1));
        queue.push(damage(2));

        assert_eq!(queue.pop_next(), Some(damage(1)));
        assert_eq!(queue.pop_next(), Some(damage(2)));
        assert_eq!(queue.pop_next(), None);
    }

    #[test]
    fn effect_queue_pops_nested_effects_before_older_siblings() {
        let mut queue = EffectQueue::default();
        queue.push(damage(1));
        queue.push(damage(2));

        assert_eq!(queue.pop_next(), Some(damage(1)));
        queue.push_nested(damage(3));

        assert_eq!(queue.pop_next(), Some(damage(3)));
        assert_eq!(queue.pop_next(), Some(damage(2)));
        assert_eq!(queue.pop_next(), None);
    }
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

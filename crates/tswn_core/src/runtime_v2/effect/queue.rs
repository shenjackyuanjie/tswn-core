use super::*;

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

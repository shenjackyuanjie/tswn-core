use super::*;

/// 待结算效果的双端队列：push 入队尾，push_nested 插队首。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EffectQueue {
    effects: VecDeque<QueuedEffect>,
}

impl EffectQueue {
    /// 把效果追加到队尾。
    pub fn push(&mut self, effect: QueuedEffect) { self.effects.push_back(effect); }

    /// 把效果插到队首，优先于队列中已有效果执行。
    pub fn push_nested(&mut self, effect: QueuedEffect) { self.effects.push_front(effect); }

    /// 取出队首效果；队列为空返回 None。
    pub fn pop_next(&mut self) -> Option<QueuedEffect> { self.effects.pop_front() }

    /// 按执行顺序遍历队列，不弹出。
    pub fn iter(&self) -> impl Iterator<Item = &QueuedEffect> { self.effects.iter() }

    /// 队列中待结算的效果数。
    pub fn len(&self) -> usize { self.effects.len() }

    /// 队列是否为空。
    pub fn is_empty(&self) -> bool { self.effects.is_empty() }

    /// 清空队列，丢弃所有未结算效果。
    pub fn clear(&mut self) { self.effects.clear(); }
}

//! 卡牌指令按会话保存；线程局部状态仅在一次推进期间借给 Runtime，退出即还原。
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy)]
pub struct ForcedAction {
    pub actor: u32,
    pub skill: u32,
    pub mp: Option<u32>,
    pub potion: u32,
}

#[derive(Debug, Clone, Default)]
pub struct ForcedState {
    pub queue: Vec<ForcedAction>,
    pub controlled_team: Option<usize>,
    pub paused_actor: Option<u32>,
    active_potions: BTreeMap<u32, (u8, i32)>,
    fallback: Option<(u32, &'static str)>,
}

thread_local! {
    static ACTIVE: RefCell<ForcedState> = RefCell::new(ForcedState::default());
    static PENDING: RefCell<Vec<ForcedAction>> = const { RefCell::new(Vec::new()) };
    static HITS: Cell<u32> = const { Cell::new(0) };
    static TAKEN: Cell<u32> = const { Cell::new(0) };
}

/// 校验三元组完整性、主动技能编号和打包字段，拒绝静默丢弃尾部数据。
pub fn decode_actions(actions: &[u32]) -> Result<Vec<ForcedAction>, &'static str> {
    if !actions.len().is_multiple_of(3) {
        return Err("actions must contain complete actor/skill/mp triples");
    }
    actions
        .chunks_exact(3)
        .map(|a| {
            if a[1] > 24 && a[1] != 255 {
                return Err("forced skill must be active id 0..24 or normal attack 255");
            }
            let potion = a[2] >> 8;
            if a[2] > 0xffff || !matches!(potion, 0..=7 | 9 | 10) {
                return Err("invalid packed potion code");
            }
            let cost = a[2] & 0xff;
            Ok(ForcedAction {
                actor: a[0],
                skill: a[1],
                mp: (cost > 0).then_some(cost),
                potion,
            })
        })
        .collect()
}

pub fn set_actions(actions: Vec<ForcedAction>) { PENDING.with(|cell| *cell.borrow_mut() = actions); }
pub fn take_pending_actions() -> Vec<ForcedAction> { PENDING.with(|cell| std::mem::take(&mut *cell.borrow_mut())) }
pub fn clear() { PENDING.with(|cell| cell.borrow_mut().clear()); }

/// 作用域守卫保证错误或 unwind 不会把当前会话的控制队伍、药水泄漏给另一场对局。
pub fn with_state<R>(state: &mut ForcedState, f: impl FnOnce() -> R) -> R {
    struct Restore<'a> {
        state: &'a mut ForcedState,
        previous: ForcedState,
    }
    impl Drop for Restore<'_> {
        fn drop(&mut self) { *self.state = ACTIVE.with(|cell| cell.replace(std::mem::take(&mut self.previous))); }
    }
    let previous = ACTIVE.with(|cell| cell.replace(std::mem::take(state)));
    let _restore = Restore { state, previous };
    f()
}

pub fn controlled_team() -> Option<usize> { ACTIVE.with(|cell| cell.borrow().controlled_team) }
pub fn set_pause(actor: u32) { ACTIVE.with(|cell| cell.borrow_mut().paused_actor = Some(actor)); }
pub fn pause_pending() -> bool { ACTIVE.with(|cell| cell.borrow().paused_actor.is_some()) }
pub fn take(actor: u32) -> Option<ForcedAction> {
    ACTIVE.with(|cell| {
        let mut state = cell.borrow_mut();
        let index = state.queue.iter().position(|a| a.actor == actor)?;
        TAKEN.with(|count| count.set(count.get() + 1));
        Some(state.queue.remove(index))
    })
}
pub fn set_active_potion(actor: u32, field: u8, delta: i32) {
    ACTIVE.with(|cell| {
        cell.borrow_mut().active_potions.insert(actor, (field, delta));
    });
}
pub fn take_active_potion(actor: u32) -> Option<(u8, i32)> { ACTIVE.with(|cell| cell.borrow_mut().active_potions.remove(&actor)) }
pub fn record_fallback(actor: u32, reason: &'static str) {
    ACTIVE.with(|cell| cell.borrow_mut().fallback = Some((actor, reason)));
}
pub fn take_fallback() -> Option<(u32, &'static str)> { ACTIVE.with(|cell| cell.borrow_mut().fallback.take()) }
pub fn hit() { HITS.with(|cell| cell.set(cell.get() + 1)); }
pub fn hits() -> u32 { HITS.with(|cell| cell.get()) }
pub fn taken() -> u32 { TAKEN.with(|cell| cell.get()) }

// 非法包必须在推进之前报错，不能丢尾部或把被动技能当主动技能执行。
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_incomplete_invalid_and_overflow_packets() {
        for packet in [
            vec![1],
            vec![1, 0],
            vec![1, 25, 0],
            vec![1, 254, 0],
            vec![1, 0, 8 << 8],
            vec![1, 0, 11 << 8],
            vec![1, 0, 1 << 16],
        ] {
            assert!(decode_actions(&packet).is_err(), "{packet:?}");
        }
        let queue = decode_actions(&[1, 255, 0, 1, 0, (1 << 8) | 23]).unwrap();
        let mut state = ForcedState {
            queue,
            ..Default::default()
        };
        with_state(&mut state, || {
            assert_eq!(take(1).unwrap().potion, 0);
            let second = take(1).unwrap();
            assert_eq!(second.potion, 1);
            assert_eq!(second.mp, Some(23));
            set_active_potion(1, 0, 10);
            set_active_potion(2, 1, 15);
            assert_eq!(take_active_potion(3), None);
            assert_eq!(take_active_potion(1), Some((0, 10)));
            assert_eq!(take_active_potion(2), Some((1, 15)));
        });
        assert_eq!(controlled_team(), None);
    }
}

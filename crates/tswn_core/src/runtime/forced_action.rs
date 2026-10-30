//! 前端强制行动队列（卡牌玩法的注入点）。
//!
//! 卡牌游戏里玩家会为「每个自己单位」指定这一次行动用哪个技能。
//! Runtime 本身只按熟练度概率自动挑选技能，这里提供一个线程内队列：
//! 调用方在开始一场战斗前排好 (实体 id, 技能 id) 对，轮到某个实体时取走
//! 属于它的第一条并直接执行；队列空了恢复原有的自动概率选择。
//!
//! 技能 id 使用 overlay 的公开编号 0..34；255 表示「普通攻击」。
//! wasm 是单线程的，用 thread_local 承载队列不需要改 `CombatRuntime` 的字段布局。

use std::cell::RefCell;

#[derive(Default)]
struct ForcedState {
    // (实体 id, 技能 id, 这一手要扣的 MP；None 表示按 tswn 现场掷骰)
    queue: Vec<(u32, u32, Option<u32>)>,
}

thread_local! {
    static FORCED: RefCell<ForcedState> = const {
        RefCell::new(ForcedState { queue: Vec::new() })
    };
    static HITS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static TAKEN: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    /// 「由玩家操控」的队伍编号；该队伍的本体与分身必须等玩家下指令。
    static CONTROLLED_TEAM: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    /// 本轮遇到未下指令的受控单位时记录其 id，供会话层截断回放。
    static PAUSE: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
}

/// 设定受控队伍；`None` 表示完全回到引擎自动行动。
pub fn set_controlled_team(team: Option<usize>) {
    CONTROLLED_TEAM.with(|cell| cell.set(team));
    PAUSE.with(|cell| cell.set(None));
}

pub fn controlled_team() -> Option<usize> { CONTROLLED_TEAM.with(|cell| cell.get()) }

/// 标记「需要一个玩家指令」：记录是哪个单位把战斗逼停的。
pub fn set_pause(actor: u32) { PAUSE.with(|cell| cell.set(Some(actor))); }

pub fn pause_pending() -> bool { PAUSE.with(|cell| cell.get().is_some()) }

/// 取走暂停标记（会话层用完即清，避免下一场残留）。
pub fn take_pause() -> Option<u32> { PAUSE.with(|cell| cell.replace(None)) }

/// 设定 (实体 id, 技能 id) 队列；按入队顺序消费。
pub fn set(pairs: Vec<(u32, u32, Option<u32>)>) {
    FORCED.with(|state| {
        let mut state = state.borrow_mut();
        state.queue = pairs;
    });
}

pub fn clear() { FORCED.with(|state| state.borrow_mut().queue.clear()); }

pub fn pending() -> usize { FORCED.with(|state| state.borrow().queue.len()) }

/// 取走属于 `actor` 的第一条强制技能，连同它抽牌时定好的 MP 消耗。
pub fn take(actor: u32) -> Option<(u32, Option<u32>)> {
    FORCED.with(|state| {
        let mut state = state.borrow_mut();
        let index = state.queue.iter().position(|(id, _, _)| *id == actor)?;
        let (_, skill, mp) = state.queue.remove(index);
        TAKEN.with(|cell| cell.set(cell.get() + 1));
        Some((skill, mp))
    })
}

pub fn hit() { HITS.with(|cell| cell.set(cell.get() + 1)); }

pub fn hits() -> u32 { HITS.with(|cell| cell.get()) }

pub fn taken() -> u32 { TAKEN.with(|cell| cell.get()) }
// —— 药水（卡牌玩法注入）：不占回合，加强当前角色一回合 ——
thread_local! {
    static POTIONS: RefCell<Vec<(u32, u32)>> = const { RefCell::new(Vec::new()) };
    static POTION_ACTIVE: std::cell::Cell<Option<(u32, u8, i32)>> = const { std::cell::Cell::new(None) };
}

/// 登记「轮到某实体时顺手喝一瓶药」；code 的低 4 位是属性下标（0..6），8 = 回血
pub fn set_potions(list: Vec<(u32, u32)>) { POTIONS.with(|cell| *cell.borrow_mut() = list); }

pub fn take_potion(actor: u32) -> Option<u32> {
    POTIONS.with(|cell| {
        let mut list = cell.borrow_mut();
        let index = list.iter().position(|(id, _)| *id == actor)?;
        Some(list.remove(index).1)
    })
}

pub fn set_active_potion(value: Option<(u32, u8, i32)>) { POTION_ACTIVE.with(|cell| cell.set(value)); }

pub fn take_active_potion() -> Option<(u32, u8, i32)> { POTION_ACTIVE.with(|cell| cell.replace(None)) }

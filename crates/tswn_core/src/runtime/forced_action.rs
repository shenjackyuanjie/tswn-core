//! 卡牌控制快照：推进期间借给 Runtime 的受控队伍与能力卡，以及诊断计数。
//!
//! 指令队列、暂停标记与药水状态属于 [`crate::runtime::control::BattleControl`]，
//! 这里只保留机制层拿不到控制句柄、又必须按队伍查询的两项数据：受控队伍编号与能力卡。
//! 会话层在每次推进前用 [`with_state`] 注入快照、退出即还原，避免借出状态跨对局泄漏。
//! 能力作用域 [`AbilityScope`] 与随机攻击乘区按根角色隔离，嵌套反弹/反击退出后恢复原作用域。
use std::cell::{Cell, RefCell};
#[cfg(feature = "cards")]
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct ForcedState {
    /// 受控队伍编号快照；`usize::MAX` 表示双方本体与分身都由玩家控制。
    pub controlled_team: Option<usize>,
    /// 能力卡为独立乘区及减耗，不参与技能熟练度读写。
    #[cfg(feature = "cards")]
    pub abilities: BTreeMap<String, (u32, u32)>,
}

thread_local! {
    static ACTIVE: RefCell<ForcedState> = RefCell::new(ForcedState::default());
    static HITS: Cell<u32> = const { Cell::new(0) };
    static TAKEN: Cell<u32> = const { Cell::new(0) };
}

/// 作用域守卫保证错误或 unwind 不会把当前会话的受控队伍、能力卡泄漏给另一场对局。
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

/// 诊断计数：引擎实际走到「这一次行动要谁出招」的次数。
pub fn hit() { HITS.with(|cell| cell.set(cell.get() + 1)); }
pub fn hits() -> u32 { HITS.with(|cell| cell.get()) }
/// 诊断计数：脚本指令被真正取走使用的次数。
pub fn note_taken() { TAKEN.with(|cell| cell.set(cell.get() + 1)); }
pub fn taken() -> u32 { TAKEN.with(|cell| cell.get()) }

/// 能力卡内核：编号表、开局解码、按队伍取值与作用域守卫。
/// 只随 cards feature 编译；关闭时本模块及其导出整体不存在。
#[cfg(feature = "cards")]
mod ability {
    use super::*;

    /// 编号与卡牌前端一致；附体和使魔自爆保留引擎的实际导出名。
    pub const ABILITY_SKILLS: [&str; 37] = [
        "fire",
        "ice",
        "thunder",
        "quake",
        "absorb",
        "poison",
        "rapid",
        "critical",
        "half",
        "exchange",
        "berserk",
        "charm",
        "haste",
        "slow",
        "curse",
        "heal",
        "revive",
        "disperse",
        "iron",
        "charge",
        "accumulate",
        "assassinate",
        "summon",
        "clone",
        "shadow",
        "defend",
        "protect",
        "reflect",
        "reraise",
        "shield",
        "counter",
        "merge",
        "zombie",
        "upgrade",
        "hide",
        "summon-explode",
        "possess",
    ];
    pub fn ability_export(id: usize) -> Option<String> {
        ABILITY_SKILLS.get(id).map(|name| {
            if *name == "possess" {
                "custom.minion.possess".into()
            } else {
                format!("core.skill.{name}")
            }
        })
    }
    pub fn decode_abilities(packet: &[u32]) -> Result<BTreeMap<String, (u32, u32)>, &'static str> {
        if !packet.len().is_multiple_of(3) {
            return Err("ability packets must contain skill/cost/power triples");
        }
        let mut out = BTreeMap::new();
        for effect in packet.chunks_exact(3) {
            let Some(name) = ability_export(effect[0] as usize) else {
                return Err("unknown ability skill");
            };
            if effect[1] > 20 || !(10000..=20000).contains(&effect[2]) {
                return Err("invalid ability strength");
            }
            if out.insert(name, (effect[1], effect[2])).is_some() {
                return Err("duplicate ability skill packet");
            }
        }
        Ok(out)
    }
    pub fn ability_value(team: usize, skill: &str) -> (u32, f64) {
        ACTIVE.with(|cell| {
            let state = cell.borrow();
            let controlled = state
                .controlled_team
                .is_some_and(|controlled| controlled == team || controlled == u32::MAX as usize);
            if !controlled {
                return (0, 1.0);
            }
            state
                .abilities
                .get(skill)
                .map_or((0, 1.0), |(cost, power)| (*cost, *power as f64 / 10000.0))
        })
    }
    pub fn max_ability_cost(team: usize) -> u32 {
        ACTIVE.with(|cell| {
            let state = cell.borrow();
            let controlled = state
                .controlled_team
                .is_some_and(|controlled| controlled == team || controlled == u32::MAX as usize);
            if !controlled {
                return 0;
            }
            state.abilities.values().map(|value| value.0).max().unwrap_or(0)
        })
    }
    thread_local! { static ABILITY_SCOPE: Cell<(u32, f64, u32)> = const { Cell::new((u32::MAX, 1.0, 0)) }; }
    /// 技能内的随机攻击乘区按根角色隔离，嵌套反弹/反击退出后恢复原作用域。
    pub struct AbilityScope((u32, f64, u32));
    impl Drop for AbilityScope {
        fn drop(&mut self) { ABILITY_SCOPE.with(|cell| cell.set(self.0)); }
    }
    pub fn scope_ability(root: u32, power: f64, cost: u32) -> AbilityScope {
        AbilityScope(ABILITY_SCOPE.with(|cell| cell.replace((root, power, cost))))
    }
    pub fn attack_multiplier(root: u32) -> f64 {
        ABILITY_SCOPE.with(|cell| {
            let (owner, power, _) = cell.get();
            if root == owner { power } else { 1.0 }
        })
    }
    pub fn mp_reduction(root: u32) -> i32 {
        ABILITY_SCOPE.with(|cell| {
            let (owner, _, cost) = cell.get();
            if root == owner { cost as i32 } else { 0 }
        })
    }
}
#[cfg(feature = "cards")]
pub use ability::*;

// 非法包必须在推进之前报错，不能丢尾部或把被动技能当主动技能执行。
#[cfg(all(test, feature = "cards"))]
mod ability_tests {
    use super::*;

    #[test]
    fn rejects_malformed_ability_packets() {
        for packet in [
            vec![0],
            vec![0, 1],
            vec![37, 0, 10000],
            vec![0, 21, 10000],
            vec![0, 0, 9999],
            vec![0, 0, 20001],
        ] {
            assert!(decode_abilities(&packet).is_err(), "{packet:?}");
        }
        assert!(decode_abilities(&[0, 1, 10000, 0, 2, 12000]).is_err(), "重复技能必须拒绝");
        let decoded = decode_abilities(&[0, 1, 10000, 36, 2, 20000]).unwrap();
        assert_eq!(decoded.get("core.skill.fire"), Some(&(1, 10000)));
        assert_eq!(decoded.get("custom.minion.possess"), Some(&(2, 20000)));
    }

    // 只有受控队伍读到卡牌；倍率按万分数换算，未受控队伍保持原值。
    #[test]
    fn ability_values_are_scoped_to_the_controlled_team() {
        let mut state = ForcedState {
            abilities: decode_abilities(&[0, 1, 11000, 29, 3, 10800]).unwrap(),
            controlled_team: Some(0),
        };
        with_state(&mut state, || {
            assert_eq!(ability_value(0, "core.skill.fire"), (1, 1.1));
            assert_eq!(ability_value(0, "core.skill.shield"), (3, 1.08));
            assert_eq!(ability_value(1, "core.skill.fire"), (0, 1.0));
            assert_eq!(max_ability_cost(0), 3);
            assert_eq!(max_ability_cost(1), 0);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_scope_restores_the_previous_snapshot() {
        let mut state = ForcedState {
            controlled_team: Some(1),
            ..Default::default()
        };
        with_state(&mut state, || {
            assert_eq!(controlled_team(), Some(1));
        });
        assert_eq!(controlled_team(), None);
    }
}

//! 生命之轮（wheel）Boss 实现。
//!
//! 线上新版 ?lang=zh（fight-369rVhWe.js）新增的 Boss，对应原实现类 `_0x355251`：
//! - 8 维加成：`[0x17, 0x87, 0x0, 0x13, 0x1b, 0x59, 0xa, 0x27]`，即
//!   `[23, 135, 0, 19, 27, 89, 10, 39]`；
//! - postDefend：`super.postDefend(...)` 返回的伤害大于 0 且 spsum 未封顶时，
//!   `spsum = min(0x800, spsum + 0x500)`（每次受击最多推进 1280 行动点，上限 2048）；
//! - 默认行动（`_0x46c448`）：继承交换技能的行动类 `_0xd4d3a7`（`act` 直接改写双方 HP
//!   并播报 `sklExchange`），并覆写三处选择逻辑：
//!   - `validTarget`：`target.hp - owner.hp >= 0x40`；
//!   - `isAvoidedBy` 恒为 false：交换既不掷回避，也不做交换免疫判定（不消耗这两处随机数）；
//!   - `select`：在敌方存活者里取 hp 最高且合格者，没有合格目标时回退普通攻击的目标选择。
//!   因此合格目标会执行「体力互换」（文案 `sklExchange` 在新版被改名为 `[0]使用[生命之轮]`，
//!   与本 Boss 同名），`select` 返回 `None` 时走普通攻击（可被回避）。
//! - 技能：`createSkills` 只 push 一个 `_0xed8723` 实例并置 `level = 0x30`。
//!   `_0xed8723` 继承通用技能基类 `_0x3a6405`，实现 `addToProcs` -> `postdamages`、
//!   `postDamage` 标记待反击、`onCounter` 在批次结束时反打，播报 `sklCounter`
//!   （即 `[0]发起[反击]`），因此它就是通用 **反击（counter，兼容键 30）** 技能，
//!   等级 48。它是被动，不进入 `k4` / `ActionSkill` 扫描，
//!   所以 `action_skills` 为空，注册走 `default_skill_levels`。
//!
//! 本模块只放生命之轮专属的数值与行为辅助函数；注册由运行时集成方维护。

#![allow(dead_code)]

/// 角色名（小写键名，与文案键 `bossName_wheel` 的后缀一致）。
pub const ID: &str = "wheel";

/// 显示名，对应文案 `bossName_wheel`。
pub const DISPLAY_NAME: &str = "生命之轮";

/// `gan()` 返回的 8 维加成。
pub const APPEND_ATTR: [i32; 8] = [23, 135, 0, 19, 27, 89, 10, 39];

/// 通用技能的兼容键：30 = 反击（counter）。
pub const COUNTER_SKILL_KEY: usize = 30;

/// `createSkills` 写入的技能等级（`0x30`）。
pub const COUNTER_SKILL_LEVEL: u32 = 48;

/// 每次受击推进的行动点（`0x500`）。
pub const MOVE_GAIN_PER_HIT: i32 = 0x500;

/// 行动点累计上限（`0x800`，与 `MOVE_POINT_THRESHOLD` 同值）。
pub const MOVE_CAP: i32 = 0x800;

/// 选定目标所需的体力差下限（`0x40`）。
pub const TARGET_HP_GAP: i32 = 0x40;

pub fn display_name() -> &'static str { DISPLAY_NAME }

/// 受击一次后的行动点。
///
/// 对应 `postDefend` 里的 `if (spsum < 0x800) spsum = min(0x800, spsum + 0x500)`：
/// 已经达到上限时原样返回；已超过上限（召唤物、疾走等直接赋值）同样不再改写。
/// 调用方负责先确认本次伤害大于 0，见 `spsum_after_defend`。
pub fn spsum_after_hit(current: i32) -> i32 {
    if current >= MOVE_CAP {
        return current;
    }
    current.saturating_add(MOVE_GAIN_PER_HIT).min(MOVE_CAP)
}

/// 一次受击结算后的行动点：只有实际掉血（`damage > 0`）才推进，吸收与恢复不推进。
pub fn spsum_after_defend(current: i32, damage: i32) -> i32 { if damage > 0 { spsum_after_hit(current) } else { current } }

/// `validTarget`：目标体力比自身高出至少 `TARGET_HP_GAP`（差值 63 不合格，64 合格）。
pub fn is_valid_target(owner_hp: i32, target_hp: i32) -> bool {
    i64::from(target_hp) - i64::from(owner_hp) >= i64::from(TARGET_HP_GAP)
}

/// 在 `(实体下标, 当前体力)` 候选里选出 hp 最高且满足 `is_valid_target` 的实体下标。
///
/// 对应 `select` 的循环：按候选顺序扫描，只接受严格更高的体力，
/// 因此并列最高时保留最先出现的候选；没有合格目标时返回 `None`，
/// 由调用方回退普通攻击的目标选择。
///
/// 返回值同时是行动判据：`Some` 时对选中者执行体力互换（跳过回避与交换免疫），
/// `None` 时执行普通攻击。
pub fn select_target<T: Copy>(owner_hp: i32, candidates: &[(T, i32)]) -> Option<T> {
    let mut best: Option<(T, i32)> = None;
    for &(index, hp) in candidates {
        if !is_valid_target(owner_hp, hp) {
            continue;
        }
        match best {
            Some((_, best_hp)) if hp <= best_hp => {}
            _ => best = Some((index, hp)),
        }
    }
    best.map(|(index, _)| index)
}

/// 生命之轮不写入 `k4`：唯一的技能是被动反击，见 `default_skill_levels`。
pub fn action_skills() -> &'static [(crate::namerena::BossActionSkill, u32)] { &[] }

/// `ac()` 写入的通用技能等级：反击 48 级。
pub fn default_skill_levels() -> &'static [(usize, u32)] { &[(COUNTER_SKILL_KEY, COUNTER_SKILL_LEVEL)] }

use super::*;
use crate::runtime::lang::keys;

impl CombatRuntime {
    /// 生命之轮的交换目标：敌方存活者里 hp 比自己高 ≥ [`TARGET_HP_GAP`] 的最高血量者。
    ///
    /// 与 JS `_0x46c448.select` 一致：只用 `world.flat_alive()` 与
    /// `plain_effective_team` 判敌我，取 hp 最高者；并列时取先出现者。
    pub fn select_plain_wheel_exchange_target(&mut self, actor: EntityIdx) -> Option<EntityIdx> {
        let actor_hp = self
            .entities
            .get(actor)
            .map(|entity| entity.runtime.hp)
            .unwrap_or_else(|| panic!("unknown runtime wheel actor: {}", actor.0));
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive();
        let mut candidates = smallvec::SmallVec::<[(EntityIdx, i32); 8]>::new();
        for &idx in all_alive {
            if idx == actor {
                continue;
            }
            if let Some(entity) = self.entities.get(idx) {
                if entity.runtime.team != actor_team {
                    candidates.push((idx, entity.runtime.hp));
                }
            }
        }
        select_target(actor_hp, &candidates)
    }

    /// 生命之轮的 postDefend 被动：每次实际受伤后推进行动值（上限 0x800）。
    ///
    /// 在整条 postDefend 链结束后按最终伤害调用，对应 JS `super.postDefend` 的返回值。
    pub fn apply_wheel_post_defend_spsum(&mut self, target: EntityIdx, damage: i32) {
        let Some(entity) = self.entities.get_mut(target) else {
            return;
        };
        if entity.template.name != ID {
            return;
        }
        let current = entity.runtime.move_state.speed_points;
        entity.runtime.move_state.speed_points = spsum_after_defend(current, damage);
    }
    /// 结算生命之轮：体力互换，但不掷回避、不做 exchange 免疫判定、不折半技能等级。
    ///
    /// 其余与通用交换技能一致：蓄力时夺取对方行动值、互换后按自身 max_hp 截断、
    /// 报 `sklExchange`/`sklExchanged` 并走 post_damage 与致死链。
    pub fn drain_plain_wheel_exchange_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let (charge_active, owner_hp, owner_max_hp) = self
            .entities
            .get(actor)
            .map(|owner| {
                (
                    owner.runtime.charge.active || owner.runtime.at_boost_millionths >= 3_000_000,
                    owner.runtime.hp,
                    owner.template.max_hp,
                )
            })
            .unwrap_or_else(|| panic!("unknown runtime wheel actor: {}", actor.0));
        let target_hp = self
            .entities
            .get(target)
            .map(|entity| entity.runtime.hp)
            .unwrap_or_else(|| panic!("unknown runtime wheel target: {}", target.0));
        updates.add(RunUpdate::new_lang(keys::SKL_EXCHANGE, actor.0 as usize, target.0 as usize, 1));
        if charge_active {
            let target_move_points = self
                .entities
                .get(target)
                .map(|entity| entity.runtime.move_state.speed_points)
                .unwrap_or_else(|| panic!("unknown runtime wheel target: {}", target.0));
            self.entities.get_mut(actor).unwrap().runtime.move_state.speed_points += target_move_points;
            self.entities.get_mut(target).unwrap().runtime.move_state.speed_points = 0;
        }
        self.entities.get_mut(actor).unwrap().runtime.hp = target_hp.min(owner_max_hp);
        self.entities.get_mut(target).unwrap().runtime.hp = owner_hp;
        updates.add(RunUpdate::new_lang(
            keys::SKL_EXCHANGED,
            actor.0 as usize,
            target.0 as usize,
            ((target_hp - owner_hp) * 2).max(0) as u32,
        ));
        if target_hp > owner_hp {
            self.drain_plain_post_damage_skill_chain_into(target, target_hp - owner_hp, actor, updates);
            let needs_lethal = self
                .entities
                .get(target)
                .is_some_and(|entity| entity.runtime.hp <= 0 && entity.runtime.alive);
            if needs_lethal {
                self.drain_plain_lethal_damage_into(actor, target, updates);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_attr_and_display_name_match_artifact_tables() {
        assert_eq!(ID, "wheel");
        assert_eq!(display_name(), "生命之轮");
        assert_eq!(APPEND_ATTR, [23, 135, 0, 19, 27, 89, 10, 39]);
    }

    #[test]
    fn spsum_gains_on_hit_and_stops_at_cap() {
        assert_eq!(spsum_after_hit(0), 0x500);
        assert_eq!(spsum_after_hit(1), 0x501);
        assert_eq!(spsum_after_hit(0x300), 0x800);
        // 到顶后不再变化，也不会溢出翻负。
        assert_eq!(spsum_after_hit(0x800), 0x800);
        assert_eq!(spsum_after_hit(0x801), 0x801);
        assert_eq!(spsum_after_hit(i32::MAX), i32::MAX);
        // 负的行动点（召唤物直接赋值）按算式推进。
        assert_eq!(spsum_after_hit(-0x800), -0x300);
    }

    #[test]
    fn spsum_only_advances_on_real_damage() {
        assert_eq!(spsum_after_defend(0x100, 1), 0x600);
        assert_eq!(spsum_after_defend(0x100, 0), 0x100);
        assert_eq!(spsum_after_defend(0x100, -30), 0x100);
        assert_eq!(spsum_after_defend(0x800, 12), 0x800);
    }

    #[test]
    fn valid_target_boundary_is_64() {
        assert!(!is_valid_target(1000, 1063));
        assert!(is_valid_target(1000, 1064));
        assert!(is_valid_target(1000, 1065));
        assert!(!is_valid_target(1000, 1000));
        // 极端差值按 i64 计算，不会因溢出误判。
        assert!(is_valid_target(i32::MIN, i32::MAX));
        assert!(!is_valid_target(i32::MAX, i32::MIN));
    }

    #[test]
    fn select_target_prefers_highest_valid_hp() {
        // 无合格目标：全部低于门槛。
        assert_eq!(select_target(1000, &[(0, 1000), (1, 1063)]), None);
        assert_eq!(select_target::<usize>(1000, &[]), None);
        // 多个合格候选时取体力最高者。
        assert_eq!(select_target(1000, &[(0, 1064), (1, 2000), (2, 1500)]), Some(1));
        // 并列最高：保留最先出现的候选（对应严格大于的比较）。
        assert_eq!(select_target(1000, &[(3, 1500), (4, 1500)]), Some(3));
        // 不合格的候选被跳过，合格者里取最大。
        assert_eq!(select_target(1000, &[(5, 900), (6, 1100), (7, 800)]), Some(6));
    }

    #[test]
    fn wheel_registers_passive_counter_only() {
        assert!(action_skills().is_empty());
        assert_eq!(default_skill_levels(), &[(30, 48)]);
    }
}

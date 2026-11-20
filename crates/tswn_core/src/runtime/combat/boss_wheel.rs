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
//!   - `act`：对已选目标重验体力差；未达 64 时攻击同一目标，不重新抽选。
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
/// 返回 `None` 时按普通攻击方式选敌；实际执行前会对已选目标重验体力差。
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
use crate::runtime::update::RunUpdateHp;

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
        if entity.template.name != ID || !entity.runtime.flags.contains(PlayerKindFlags::BOSS) {
            return;
        }
        let current = entity.runtime.move_state.speed_points;
        entity.runtime.move_state.speed_points = spsum_after_defend(current, damage);
    }

    pub(super) fn plain_wheel_action_amount(&self, actor: EntityIdx, target: EntityIdx) -> i32 {
        let owner = &self.entities.get(actor).unwrap().runtime;
        let enemy = &self.entities.get(target).unwrap().runtime;
        if is_valid_target(owner.hp, enemy.hp) {
            0
        } else {
            owner.attack
        }
    }

    /// 结算生命之轮的默认行动。差值不足时物理普攻；互换不掷回避或免疫。
    ///
    /// 其余与通用交换技能一致：蓄力时夺取对方行动值、互换后按自身 max_hp 截断、
    /// 报 `sklExchange`/`sklExchanged` 并走 post_damage 与致死链。
    pub fn drain_plain_wheel_exchange_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let (charge_active, owner_hp, owner_max_hp) = self
            .entities
            .get(actor)
            .map(|owner| (owner.runtime.charge.active, owner.runtime.hp, owner.template.max_hp))
            .unwrap_or_else(|| panic!("unknown runtime wheel actor: {}", actor.0));
        let target_hp = self
            .entities
            .get(target)
            .map(|entity| entity.runtime.hp)
            .unwrap_or_else(|| panic!("unknown runtime wheel target: {}", target.0));
        if !is_valid_target(owner_hp, target_hp) {
            self.drain_plain_default_attack_into(actor, target, false, updates);
            return;
        }
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
        let owner_new_hp = target_hp.min(owner_max_hp);
        self.entities.get_mut(actor).unwrap().runtime.hp = owner_new_hp;
        self.entities.get_mut(target).unwrap().runtime.hp = owner_hp;
        updates.add_with(|| {
            RunUpdate::new_lang(
                keys::SKL_EXCHANGED,
                actor.0 as usize,
                target.0 as usize,
                ((target_hp - owner_hp) * 2).max(0) as u32,
            )
            .with_hp_changes([
                RunUpdateHp {
                    player: actor.0 as usize,
                    before: owner_hp,
                    after: owner_new_hp,
                },
                RunUpdateHp {
                    player: target.0 as usize,
                    before: target_hp,
                    after: owner_hp,
                },
            ])
        });
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

    /// 生命之轮（队伍 0，体力 100）加两名敌方英雄；队伍 0 另有一名高体力队友专门验证排除。
    fn wheel_runtime() -> CombatRuntime {
        let mut builder = ExtensionRegistryBuilder::default();
        let boss_kind = builder
            .register_player_kind_with_policies(
                "core",
                "boss",
                "core.boss",
                PlayerKindFlags::BOSS,
                PlayerKindPolicies::default(),
            )
            .expect("boss kind should register");
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, ID, boss_kind, 0, 1000, 3),
                PlayerTemplate::new(2, ID, 1, 2000, 3),
                PlayerTemplate::new(3, "hero-b", 1, 300, 3),
                PlayerTemplate::new(4, "mate", 0, 5000, 3),
            ],
            builder.build(),
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 100;
        runtime
    }

    fn messages_of(updates: &RunUpdates) -> Vec<String> {
        updates.updates.iter().map(|update| update.message.to_string()).collect()
    }

    #[test]
    fn wheel_select_skips_own_team_and_takes_highest_valid_hp() {
        let mut runtime = wheel_runtime();
        // 普通角色 wheel 的 2000 体力最高且合格；队友 mate 的 5000 必须被排除。
        assert_eq!(runtime.select_plain_wheel_exchange_target(EntityIdx(0)), Some(EntityIdx(1)));
        // 门槛是 hp 差 >= 64：两名敌人都是 163（差 63）时全不合格。
        runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 163;
        runtime.entities.get_mut(EntityIdx(2)).unwrap().runtime.hp = 163;
        assert_eq!(runtime.select_plain_wheel_exchange_target(EntityIdx(0)), None);
        // 差 64 恰好合格。
        runtime.entities.get_mut(EntityIdx(2)).unwrap().runtime.hp = 164;
        assert_eq!(runtime.select_plain_wheel_exchange_target(EntityIdx(0)), Some(EntityIdx(2)));
    }

    #[test]
    fn wheel_select_keeps_first_candidate_on_tied_hp() {
        let mut runtime = wheel_runtime();
        runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 900;
        runtime.entities.get_mut(EntityIdx(2)).unwrap().runtime.hp = 900;
        assert_eq!(runtime.select_plain_wheel_exchange_target(EntityIdx(0)), Some(EntityIdx(1)));
    }

    #[test]
    fn wheel_exchange_swaps_hp_clamps_owner_and_spends_no_rng() {
        let mut runtime = wheel_runtime();
        let before = (runtime.rng.i, runtime.rng.j);
        let mut updates = RunUpdates::new();
        runtime.drain_plain_wheel_exchange_into(EntityIdx(0), EntityIdx(1), &mut updates);
        assert_eq!((runtime.rng.i, runtime.rng.j), before);
        // 施法者截断到自身最大体力 1000，目标拿到施法者交换前的 100。
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 1000);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 100);
        assert_eq!(messages_of(&updates), vec!["[0]使用[生命之轮]", "[1]的体力值与[0]互换"]);
        assert_eq!(updates.updates[0].score, 1);
        // 分值为体力差的两倍：(2000 - 100) * 2。
        assert_eq!(updates.updates[1].score, 3800);
    }

    #[test]
    fn wheel_exchange_steals_move_points_when_charged() {
        let mut runtime = wheel_runtime();
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.charge.active = true;
        runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.move_state.speed_points = 0x500;
        runtime.drain_plain_wheel_exchange_into(EntityIdx(0), EntityIdx(1), &mut RunUpdates::new());
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points,
            0x500
        );
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.move_state.speed_points, 0);
    }

    #[test]
    fn wheel_exchange_skips_charge_steal_when_not_charged() {
        let mut runtime = wheel_runtime();
        runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.move_state.speed_points = 0x500;
        runtime.drain_plain_wheel_exchange_into(EntityIdx(0), EntityIdx(1), &mut RunUpdates::new());
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points, 0);
        assert_eq!(
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.move_state.speed_points,
            0x500
        );
    }

    fn wheel_turn_runtime(
        target_hp: i32,
        another_enemy: bool,
        at_boost: f64,
        before_attack: Option<SkillHandlerFn>,
    ) -> CombatRuntime {
        let mut builder = ExtensionRegistryBuilder::default();
        let boss_kind = builder
            .register_player_kind_with_policies(
                "core",
                "boss",
                "core.boss",
                PlayerKindFlags::BOSS,
                PlayerKindPolicies::default(),
            )
            .unwrap();
        let mut wheel = PlayerTemplate::with_kind(1, ID, boss_kind, 0, 100, 30)
            .with_speed(1)
            .with_speed_points(2049)
            .with_at_boost(at_boost);
        let before_attack = before_attack.map(|handler| {
            let skill = builder
                .register_skill_with_hooks(
                    "test",
                    "before-action-damage",
                    "test.before-action-damage",
                    ProcMask::PRE_DAMAGE,
                    TargetPolicy::None,
                    SkillPriority(0),
                )
                .unwrap();
            (skill, handler)
        });
        if let Some((skill, _)) = before_attack {
            wheel = wheel.with_skills([skill]);
        }
        let mut players = vec![wheel, PlayerTemplate::new(2, "enemy", 1, 164, 3).with_speed_points(0x500)];
        if another_enemy {
            players.push(PlayerTemplate::new(3, "other-enemy", 1, 164, 3));
        }
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(players, builder.build()));
        runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = target_hp;
        if let Some((skill, handler)) = before_attack {
            runtime.set_skill_handler(skill, handler);
        }
        runtime
    }

    fn lower_first_target_hp(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
        context.push_nested(QueuedEffect::Damage {
            caster: entry.owner,
            target: EntityIdx(1),
            amount: 1,
        });
    }

    fn raise_first_target_hp(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
        context.push_nested(QueuedEffect::Heal {
            caster: entry.owner,
            target: EntityIdx(1),
            amount: 1,
        });
    }

    #[test]
    fn wheel_rechecks_hp_at_execution_and_uses_the_same_target_for_fallback() {
        // 两个敌人本来都合格，先选第一人；钩子只把第一人的差值改成 63。
        let mut runtime = wheel_turn_runtime(164, true, 1.0, Some(lower_first_target_hp));
        let (wheel, target) = (EntityIdx(0), EntityIdx(1));
        let mut expected = runtime.clone();
        for _ in 0..3 {
            // 步长、smart、技能 MP 门槛。原版换血选敌和重验失败时都不重抽。
            expected.rng.next_u8();
        }
        expected.entities.get_mut(target).unwrap().runtime.hp = 163;
        expected.drain_plain_default_attack_into(wheel, target, false, &mut RunUpdates::new());
        expected.rng.r127(); // 行动后 MP 恢复。

        let round = runtime.run_minimal_round();
        let action = round.action.unwrap();
        assert_eq!((action.actor, action.target), (wheel, target));
        assert_eq!(action.amount, runtime.entities.get(wheel).unwrap().runtime.attack);
        let updates = round.frame.unwrap().updates;
        assert!(!updates.updates.iter().any(|u| u.message == "[0]使用[生命之轮]"));
        assert!(updates.updates.iter().any(|u| u.message == "[0]发起攻击" && u.target == 1));
        assert_eq!(runtime.entities.get(wheel).unwrap().runtime.hp, 100);
        assert_eq!(
            runtime.entities.get(target).unwrap().runtime.hp,
            expected.entities.get(target).unwrap().runtime.hp
        );
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 164);
        assert_eq!(
            (runtime.rng.i, runtime.rng.j, runtime.rng.main_val),
            (expected.rng.i, expected.rng.j, expected.rng.main_val)
        );
    }

    #[test]
    fn wheel_rechecks_the_fallback_target_and_exchanges_at_gap_64() {
        let mut runtime = wheel_turn_runtime(163, false, 1.0, Some(raise_first_target_hp));
        let round = runtime.run_minimal_round();
        let action = round.action.unwrap();
        assert_eq!((action.target, action.amount), (EntityIdx(1), 0));
        let updates = round.frame.unwrap().updates;
        assert!(!updates.updates.iter().any(|u| u.message == "[0]发起攻击"));
        assert!(
            updates
                .updates
                .iter()
                .any(|u| u.message == "[1]的体力值与[0]互换" && u.target == 1 && u.score == 128)
        );
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 100);
    }

    #[test]
    fn wheel_exchange_only_steals_move_points_for_real_charge() {
        for charged in [false, true] {
            let mut runtime = wheel_turn_runtime(164, false, if charged { 1.0 } else { 3.0 }, None);
            let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
            if charged {
                owner.activate_charge_runtime();
            }
            assert_eq!(owner.runtime.charge.active, charged);
            assert_eq!(owner.runtime.at_boost(), 3.0);
            let mut expected_rng = runtime.rng.clone();
            let points_after_step = 1 + i32::from(expected_rng.next_u8() & 3);
            // smart、技能 MP 门槛、行动后 MP 恢复。互换本身不掷回避或免疫。
            for _ in 0..3 {
                expected_rng.next_u8();
            }
            let round = runtime.run_minimal_round();
            assert_eq!(round.action.unwrap().target, EntityIdx(1));
            assert!(round.frame.unwrap().updates.updates.iter().any(|u| u.message == "[0]使用[生命之轮]"));
            let owner = &runtime.entities.get(EntityIdx(0)).unwrap().runtime;
            let target = &runtime.entities.get(EntityIdx(1)).unwrap().runtime;
            assert_eq!(target.move_state.speed_points, if charged { 0 } else { 0x500 });
            assert_eq!(
                owner.move_state.speed_points,
                points_after_step + if charged { 0x500 } else { 0 }
            );
            assert_eq!(
                (runtime.rng.i, runtime.rng.j, runtime.rng.main_val),
                (expected_rng.i, expected_rng.j, expected_rng.main_val)
            );
        }
    }

    #[cfg(feature = "cards")]
    #[test]
    fn wheel_exchange_does_not_steal_from_a_scoped_attack_multiplier() {
        use crate::runtime::forced_action::{attack_multiplier, scope_ability};

        let mut normal = wheel_turn_runtime(164, false, 1.0, None);
        let mut boosted = normal.clone();
        normal.run_minimal_round();
        let _ability = scope_ability(0, 3.0, 0);
        assert_eq!(attack_multiplier(0), 3.0);
        boosted.run_minimal_round();
        for idx in [EntityIdx(0), EntityIdx(1)] {
            let normal = &normal.entities.get(idx).unwrap().runtime;
            let boosted = &boosted.entities.get(idx).unwrap().runtime;
            assert!(!boosted.charge.active);
            assert_eq!((boosted.hp, boosted.move_state), (normal.hp, normal.move_state));
        }
        assert_eq!(
            boosted.entities.get(EntityIdx(1)).unwrap().runtime.move_state.speed_points,
            0x500
        );
        assert_eq!(
            (boosted.rng.i, boosted.rng.j, boosted.rng.main_val),
            (normal.rng.i, normal.rng.j, normal.rng.main_val)
        );
    }

    #[test]
    fn wheel_post_defend_only_advances_the_boss_itself() {
        let mut runtime = wheel_runtime();
        assert!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.flags.contains(PlayerKindFlags::BOSS));
        runtime.apply_wheel_post_defend_spsum(EntityIdx(0), 12);
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points,
            0x500
        );
        runtime.apply_wheel_post_defend_spsum(EntityIdx(0), 0);
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points,
            0x500
        );
    }

    #[test]
    fn wheel_post_defend_distinguishes_a_real_boss_from_an_ordinary_namesake() {
        let mut runtime = wheel_runtime();
        for (attacker, target, initial_hp, expected_points) in
            [(EntityIdx(2), EntityIdx(0), 100, 0x500), (EntityIdx(3), EntityIdx(1), 2000, 0)]
        {
            let damage = runtime.drain_plain_attack_after_dodge_into(
                attacker,
                target,
                false,
                64.0,
                DamageKind::Default,
                None,
                PlainAttackOnDamage::None,
                &mut RunUpdates::new(),
            );
            let entity = runtime.entities.get(target).unwrap();
            assert!(damage > 0);
            assert_eq!(entity.runtime.hp, initial_hp - damage);
            assert_eq!(entity.runtime.move_state.speed_points, expected_points);
        }
    }
}

//! 灭霸（thanos）Boss 实现。
//!
//! 语义来自线上新版 ?lang=zh 构建（assets/fight-369rVhWe.js）的反混淆产物
//! .tmp_namerena_diff/fight.deobf.js 中的 _0xab7917 类：
//!
//! - get appendAttr() 返回 [0x27,0x24,0xc,0x10,0x0,0x25,0x14,0x71]，见 [`APPEND_ATTR`]；
//! - 该类没有覆盖 immuned / immunedx，免疫列表写在默认行动类 _0x5057e0 的
//!   get immuned() 上，见 [`IMMUNE`]；
//! - createSkills() 只做两件事：先 this.dftAct = new _0x5057e0(this)，再 push 一个
//!   level = 0x50（80）的 _0x1ebbfe。_0x1ebbfe extends _0x3a6405（被动技能基类），
//!   它的 addToProcs() 把自己挂到 owner.preactions，preAction 在
//!   level >= shield.shield 时按 nextInt(1 + trunc(level * 3 / 4)) + 1 追加护盾
//!   （护盾对象 _0x423eea 就是带 shield 字段的 postDefends 条目）。
//!   这与 run_shield_pre_action_skill（core.skill.shield）逐行一致，因此映射为
//!   护盾（技能键 29，等级 80）；它不继承 _0x2d7e12（ActionSkill 基类），
//!   而 addSkillsToProc() 只把 _0x2d7e12 实例收进 actions，所以灭霸的 k4 为空，
//!   见 [`action_skills`] 与 [`default_skill_levels`]；
//! - 默认行动 _0x5057e0.act：敌方存活人数 > 己方存活人数时先报「响指」，再随机让
//!   floor(敌方存活 / 2) 个敌人 hp = 0 并走 onDie；否则普通攻击，见
//!   [`should_snap`] / [`snap_target_count`] / [`dust_delay`]。
//!
//! 本模块只放灭霸专属的数值与纯函数；注册项（名字表、加成表、免疫表、默认技能等级、
//! 文案 key、行动分派）由运行时集成方维护，因此这些公开项在集成落地前只有单测引用。
#![allow(dead_code)]

use crate::namerena::BossActionSkill;

/// Boss 标识，对应 JS bossTypes 的 thanos 键。
pub const ID: &str = "thanos";

/// 8 维加成，对应 JS get appendAttr() 的 [0x27,0x24,0xc,0x10,0x0,0x25,0x14,0x71]。
pub const APPEND_ATTR: [i32; 8] = [39, 36, 12, 16, 0, 37, 20, 113];

/// 默认行动类 _0x5057e0.get immuned() 列出的状态键。
///
/// 键名沿用 boss_immune_threshold 的写法；这 5 个键也在 Boss 基类的默认 8 键免疫表内，
/// 阈值同为 192。
pub const IMMUNE: [&str; 5] = ["assassinate", "half", "curse", "exchange", "charm"];

/// 响指文案键，对应 lang/zh.json 的 sklThanosSnap = 「[0]打了个[响指]」。
///
/// JS 里写作 `_0x3018a2('fqtA')`；用同一份 4 字符哈希反解可得
/// hash4("sklThanosSnap") == "fqtA"，与文案表逐字对应。
pub const SNAP_TEXT_KEY: &str = "sklThanosSnap";

/// 灰烬文案键，对应 lang/zh.json 的 sklThanosDust = 「[1]化成了灰烬」。
///
/// JS 里写作 `_0x3018a2('ysFN')`，同样由 hash4("sklThanosDust") == "ysFN" 反解确认。
pub const DUST_TEXT_KEY: &str = "sklThanosDust";

/// onDie 未指定自定义文案时的默认 update 延迟（JS 0x32）。
pub const DEFAULT_DIE_DELAY: i32 = 50;

/// 灭霸 push 的护盾技能等级（JS 0x50）。
pub const SHIELD_LEVEL: u32 = 80;

/// 护盾在通用技能键表中的编号，见 namerena::skill_name_for_export。
pub const SHIELD_SKILL_KEY: usize = 29;

/// 显示名，对应 lang/zh.json 的 bossName_thanos。
pub fn display_name() -> &'static str { "灭霸" }

/// 默认行动是否改为「响指」。
///
/// JS 里 alives 是全场存活者，enemies 过滤掉自己和 allyGroup 同队者，
/// 己方人数用 alives.length - enemies.length 得到，因此**包含灭霸自己**。
/// 原判定是 enemies.length <= allies 时普通攻击，所以响指条件是严格大于。
pub fn should_snap(enemy_alive: usize, ally_alive: usize) -> bool { enemy_alive > ally_alive }

/// 响指随机消灭的目标数：JS Math.floor(enemies.length / 2)。
///
/// 每次循环从局部副本里 splice 掉一个随机敌人，因此每个敌人最多被处理一次；
/// 1 个敌人时取到 0（实际也进不了响指分支，因为 1 <= 己方人数）。
pub fn snap_target_count(alive: usize) -> usize { alive / 2 }

/// 灰烬 update 的延迟：JS 里 onDie 的第五参是 [_0x3018a2('ysFN'), hp + 0x32]，
/// 其中第二项是 update 延迟（不是消息参数），按死者原体力再加 50。
pub fn dust_delay(hp_before: i32) -> i32 { hp_before + DEFAULT_DIE_DELAY }

/// k4（Boss 主动技能扫描）条目；灭霸为空。
///
/// JS addSkillsToProc() 只把 _0x2d7e12（ActionSkill）实例收进 actions，
/// 灭霸唯一的技能 _0x1ebbfe 是 preaction 护盾，因此 k4 没有任何条目。
pub const ACTION_SKILLS: [(BossActionSkill, u32); 0] = [];

/// 返回灭霸的 k4 条目（空表），供 boss_action_skills 直接引用。
pub fn action_skills() -> &'static [(BossActionSkill, u32)] { &ACTION_SKILLS[..] }

/// 灭霸的默认技能等级表，形如 boss_default_skill_levels 的 (技能键, 等级)。
///
/// JS 只 push 一个 level = 0x50 的护盾，因此条目是 (29, 80)。
pub fn default_skill_levels() -> &'static [(usize, u32)] { &[(SHIELD_SKILL_KEY, SHIELD_LEVEL)] }

use super::*;
use crate::runtime::lang::keys;

impl CombatRuntime {
    /// 灭霸的响指目标：敌方存活严格多于己方存活时，随机抽取 floor(敌方/2) 个敌人。
    ///
    /// 己方口径与 JS 一致：全场存活者里与自己同队（`plain_effective_team`）的个数，包含灭霸自己。
    pub fn select_plain_thanos_snap_targets(&mut self, actor: EntityIdx) -> Option<PreparedTargetList> {
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive();
        let mut enemies = smallvec::SmallVec::<[EntityIdx; 8]>::new();
        let mut allies = 0usize;
        for &idx in all_alive {
            if self.entities.get(idx).is_some_and(|entity| entity.runtime.team == actor_team) {
                allies += 1;
            } else {
                enemies.push(idx);
            }
        }
        if !should_snap(enemies.len(), allies) {
            return None;
        }
        let count = snap_target_count(enemies.len());
        let mut targets = PreparedTargetList::new();
        for _ in 0..count {
            let picked = self.rng.pick(&enemies)?;
            targets.push(enemies.swap_remove(picked));
        }
        Some(targets)
    }

    /// 结算响指：先报「打了个响指」，再逐个把目标 hp 置 0 并走死亡链。
    ///
    /// 灰烬帧的延迟沿用 JS onDie 第五参的 `hp + 0x32`，见 [`dust_delay`]。
    pub fn drain_plain_thanos_snap_into(&mut self, actor: EntityIdx, targets: PreparedTargetList, updates: &mut RunUpdates) {
        updates.add(RunUpdate::new_lang(
            keys::SKL_THANOS_SNAP,
            actor.0 as usize,
            actor.0 as usize,
            0,
        ));
        for target in targets {
            let hp_before = self.entities.get(target).map(|entity| entity.runtime.hp).unwrap_or(0);
            updates.add_newline();
            let killed = self.kill_entity_without_damage_into(target, updates);
            if killed {
                let mut frame = RunUpdate::new_lang(keys::SKL_THANOS_DUST, actor.0 as usize, target.0 as usize, 0);
                frame.delay0 = dust_delay(hp_before);
                updates.add(frame);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_attr_matches_js_hex_table() {
        assert_eq!(APPEND_ATTR, [0x27, 0x24, 0xc, 0x10, 0x0, 0x25, 0x14, 0x71]);
        assert_eq!(APPEND_ATTR, [39, 36, 12, 16, 0, 37, 20, 113]);
    }

    #[test]
    fn identity_matches_lang_table() {
        assert_eq!(ID, "thanos");
        assert_eq!(display_name(), "灭霸");
        assert_eq!(SNAP_TEXT_KEY, "sklThanosSnap");
        assert_eq!(DUST_TEXT_KEY, "sklThanosDust");
    }

    #[test]
    fn immune_lists_the_five_act_keys() {
        assert_eq!(IMMUNE, ["assassinate", "half", "curse", "exchange", "charm"]);
    }

    #[test]
    fn snap_requires_strictly_more_enemies_than_allies() {
        // (敌方存活, 己方存活含自己, 期望)
        for (enemy_alive, ally_alive, expected) in [
            (0, 0, false),
            (0, 1, false),
            (1, 1, false),
            (1, 2, false),
            (2, 2, false),
            (2, 3, false),
            (2, 1, true),
            (3, 2, true),
            (5, 4, true),
        ] {
            assert_eq!(
                should_snap(enemy_alive, ally_alive),
                expected,
                "enemy={enemy_alive} ally={ally_alive}"
            );
        }
    }

    #[test]
    fn snap_target_count_floors_half_and_handles_edges() {
        for (alive, expected) in [(0, 0), (1, 0), (2, 1), (3, 1), (4, 2), (5, 2), (6, 3), (7, 3), (9, 4)] {
            assert_eq!(snap_target_count(alive), expected, "alive={alive}");
        }
    }

    #[test]
    fn single_enemy_never_loses_anything() {
        assert_eq!(snap_target_count(1), 0);
    }

    #[test]
    fn k4_is_empty_because_the_skill_is_a_pre_action_shield() {
        assert!(action_skills().is_empty());
        assert_eq!(ACTION_SKILLS.len(), 0);
    }

    #[test]
    fn default_skill_levels_register_the_shield_at_80() {
        assert_eq!(default_skill_levels(), &[(29, 80)]);
        assert_eq!(SHIELD_LEVEL, 0x50);
        assert_eq!(
            crate::namerena::skill_name_for_export(SHIELD_SKILL_KEY),
            "sklshield",
            "技能键 29 必须是护盾"
        );
    }

    #[test]
    fn dust_delay_offsets_the_hp_by_fifty() {
        assert_eq!(dust_delay(0), 50);
        assert_eq!(dust_delay(1234), 1284);
        assert_eq!(DEFAULT_DIE_DELAY, 0x32);
    }
}

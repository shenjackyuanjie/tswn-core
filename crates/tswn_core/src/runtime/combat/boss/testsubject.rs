//! 实验体（testsubject）Boss 实现：三阶段行动表、易伤、无实体与异常再生。
//!
//! 实验体是正式 Boss：七围 80、体力 100、初始 MP 1000，按 `revives_used` 0/1/2
//! 分三个阶段，每个阶段有各自的主打技能与抽签阈值。它此前的「测试 Boss」只是名字，
//! 不参与任何兼容输入或半成品定位，玩家可见输入名仍是 `testsubject@!`。
//!
//! # 行动表
//!
//! 每个阶段先抽 `roll_skill(level) = rng.r127() < level`，未中才回落到普攻：
//!
//! | 阶段 | 抽签阈值 | 技能 | 效果 |
//! | --- | --- | --- | --- |
//! | 一 | 64 | 颅骨重击 | 全体敌人 80% 物理；有效伤害附加易伤 |
//! | 二 | 128 | 多段爪击 | 60% 物理 ×（已用次数 + 3）段；累计伤害为正时叠迟缓 |
//! | 三 | 21 | 割裂 | 60% 物理 × 3 |
//! | 三 | 64 | 大猛扑 | 225% 物理 |
//! | 三 | 128 | 燃烧咆哮 | 全体敌人 80% 魔法，命中附加灼烧 |
//!
//! 一阶段普攻与颅骨重击都吃 `1.0 + 0.1 × 本阶段已行动次数` 的倍率爬升。
//!
//! 注意 `r127()` 取值 0..=127，所以阈值 128 的两次抽签恒成立：二阶段必然多段爪击，
//! 三阶段在割裂、大猛扑都未中时必然燃烧咆哮。只有一阶段（阈值 64）才有普攻回落，
//! 旧代码里另外两个 else 分支是不可达的防御性写法。
//!
//! # 概率与结算分离
//!
//! 抽签发生在 `scan_plain_boss_action_skills`（Boss `k4` 扫描）阶段，选中的招式记在
//! `TestSubjectRuntime::pending_skill`，由 `drain_plain_boss_skill_into` 原样读取。
//! 两阶段必须分开，否则同一批随机字节会被抽两次。
//!
//! # 属性爬升落在模板上
//!
//! 新 Runtime 的运行态属性由 `PlayerTemplate` 派生（`refresh_runtime_stats_from_template`），
//! 隐匿、垂死、聚气等机制都会重算。因此异常再生时同步改写模板与运行态两侧，
//! 与 `xjy_iron` / `fish_comfort` 的永久属性改写一致；只改运行态会被下一次刷新抹平。
//!
//! # 复活走死亡钩子
//!
//! 旧实现靠 `post_damage_priority = i32::MIN` 抢在死亡记录前复活。新 Runtime 的等价位置是
//! `drain_die_hooks_into`：它在 `alive = false` 与 `mark_dead_with_linked_minions_into`
//! 之前执行，与马里奥复活同一条路径，因此同样不会和死亡特效、胜负判定打架。
#![allow(dead_code)]

use super::super::*;
use crate::runtime::profile::{
    DEFAULT_CORE_TESTSUBJECT_NO_ENTITY_STATE_EXPORT, DEFAULT_CORE_TESTSUBJECT_SKULL_FRACTURE_STATE_EXPORT, PLAIN_FIRE_STATE_KEY,
    PLAIN_TESTSUBJECT_NO_ENTITY_STATE_KEY, PLAIN_TESTSUBJECT_SKULL_FRACTURE_STATE_KEY,
};
use crate::runtime::update::RunUpdateHp;

/// Boss 标识，对应输入名 `testsubject` 与文案键 `bossName_testsubject`。
pub const ID: &str = "testsubject";

/// 显示名，对应文案 `bossName_testsubject`。
pub const DISPLAY_NAME: &str = "实验体 #C8";

/// 最多异常再生次数；`revives_used` 上限即为 `MAX_REVIVES`。
pub const MAX_REVIVES: i32 = 2;

/// 一阶段基础七围；每阶段爬升 `PHASE_ATTR_STEP`。
pub const BASE_ATTR: i32 = 80;

/// 每阶段七围增量。
pub const PHASE_ATTR_STEP: i32 = 40;

/// 一阶段基础体力；每阶段爬升 `PHASE_HP_STEP`。
pub const BASE_HP: i32 = 100;

/// 每阶段体力增量。
pub const PHASE_HP_STEP: i32 = 100;

/// 异常再生后重置的 MP。
pub const PHASE_MAGIC_POINT: i32 = 1000;

/// 一阶段颅骨重击的抽签阈值。
pub const SKULL_SMASH_LEVEL: u32 = 64;

/// 颅骨重击的物理倍率。
pub const SKULL_SMASH_SCALE: f64 = 0.8;

/// 二阶段多段爪击的抽签阈值。
pub const MULTI_CLAW_LEVEL: u32 = 128;

/// 多段爪击单段物理倍率。
pub const MULTI_CLAW_SCALE: f64 = 0.6;

/// 多段爪击在已用次数之外固定追加的段数。
pub const MULTI_CLAW_EXTRA_HITS: i32 = 3;

/// 三阶段割裂的抽签阈值。
pub const RUPTURE_LEVEL: u32 = 21;

/// 割裂的物理倍率。
pub const RUPTURE_SCALE: f64 = 0.6;

/// 割裂的段数。
pub const RUPTURE_HITS: usize = 3;

/// 三阶段大猛扑的抽签阈值。
pub const GREAT_POUNCE_LEVEL: u32 = 64;

/// 大猛扑的物理倍率。
pub const GREAT_POUNCE_SCALE: f64 = 2.25;

/// 三阶段燃烧咆哮的抽签阈值。
pub const BURNING_ROAR_LEVEL: u32 = 128;

/// 燃烧咆哮的魔法倍率。
pub const BURNING_ROAR_SCALE: f64 = 0.8;

/// 易伤倍率；向上取整。
pub const SKULL_FRACTURE_MULTIPLIER: f64 = 1.5;

/// 实验体的显示名，与 `DISPLAY_NAME` 一致。
pub fn display_name() -> &'static str { DISPLAY_NAME }

/// 该实体是否是实验体 Boss。
pub fn is_testsubject(entity: &EntityRecord) -> bool {
    entity.runtime.flags.contains(PlayerKindFlags::BOSS) && entity.template.name == ID
}

/// 指定阶段的七围。
pub fn phase_attr(revives_used: i32) -> i32 { BASE_ATTR + revives_used.max(0) * PHASE_ATTR_STEP }

/// 指定阶段的体力上限。
pub fn phase_hp(revives_used: i32) -> i32 { BASE_HP + revives_used.max(0) * PHASE_HP_STEP }

/// 实验体不写入 `k4` 之外的默认技能表：它的招式全部由三阶段行动表驱动。
pub fn action_skills() -> &'static [(crate::namerena::BossActionSkill, u32)] { &[] }

impl CombatRuntime {
    /// 实验体当前的阶段序号（1 起算）；非实验体返回 1。
    pub fn plain_testsubject_phase(&self, actor: EntityIdx) -> i32 {
        self.entities
            .get(actor)
            .and_then(|entity| entity.runtime.boss.as_deref().and_then(|b| b.testsubject.as_ref()))
            .map_or(1, |state| state.phase())
    }

    /// 一阶段伤害倍率：非一阶段恒为 1.0。
    pub fn plain_testsubject_phase_attack_scale(&self, actor: EntityIdx) -> f64 {
        let Some(state) = self
            .entities
            .get(actor)
            .and_then(|entity| entity.runtime.boss.as_deref().and_then(|b| b.testsubject.as_ref()))
        else {
            return 1.0;
        };
        if state.phase() == 1 {
            state.phase_one_damage_scale()
        } else {
            1.0
        }
    }

    /// 实验体的敌方存活列表：排除自己、排除与自己有效队伍同队者。
    ///
    /// 与灭霸响指的选敌口径一致，因此不消耗随机字节；群体技能按世界顺序逐个结算。
    fn testsubject_enemies(&self, actor: EntityIdx) -> Vec<EntityIdx> {
        let actor_team = self.plain_effective_team(actor);
        self.world
            .flat_alive()
            .iter()
            .copied()
            .filter(|&idx| idx != actor && self.entities.get(idx).is_some_and(|entity| entity.runtime.team != actor_team))
            .collect()
    }

    /// 目标是否存活且体力为正。
    fn testsubject_target_alive(&self, target: EntityIdx) -> bool {
        self.entities
            .get(target)
            .is_some_and(|entity| entity.runtime.alive && entity.runtime.hp > 0)
    }

    /// 按当前阶段抽签；命中则把招式记进 pending_skill 并返回 true。
    ///
    /// 只在这里消费随机字节：三阶段最多连抽三次，全部未中时返回 false，
    /// 由 k4 扫描回落到普攻。
    pub fn roll_plain_testsubject_phase_skill(&mut self, actor: EntityIdx) -> bool {
        if !self.entities.get(actor).is_some_and(is_testsubject) {
            return false;
        }
        let skill = match self.plain_testsubject_phase(actor) {
            1 => (self.rng.r127() < SKULL_SMASH_LEVEL).then_some(TestSubjectSkill::SkullSmash),
            2 => (self.rng.r127() < MULTI_CLAW_LEVEL).then_some(TestSubjectSkill::MultiClaw),
            _ => {
                if self.rng.r127() < RUPTURE_LEVEL {
                    Some(TestSubjectSkill::Rupture)
                } else if self.rng.r127() < GREAT_POUNCE_LEVEL {
                    Some(TestSubjectSkill::GreatPounce)
                } else if self.rng.r127() < BURNING_ROAR_LEVEL {
                    Some(TestSubjectSkill::BurningRoar)
                } else {
                    None
                }
            }
        };
        let Some(entity) = self.entities.get_mut(actor) else {
            return false;
        };
        let Some(state) = entity.runtime.boss_mut().testsubject.as_mut() else {
            return false;
        };
        state.pending_skill = skill;
        skill.is_some()
    }

    /// 结算本回合选中的实验体招式，并推进阶段计数与无实体轮换。
    pub fn drain_testsubject_action_into(&mut self, actor: EntityIdx, smart: bool, updates: &mut RunUpdates) {
        let skill = self
            .entities
            .get(actor)
            .and_then(|entity| entity.runtime.boss.as_deref().and_then(|b| b.testsubject.as_ref()))
            .and_then(|state| state.pending_skill);
        match skill {
            Some(TestSubjectSkill::SkullSmash) => self.drain_testsubject_skull_smash_into(actor, updates),
            Some(TestSubjectSkill::MultiClaw) => self.drain_testsubject_multi_claw_into(actor, smart, updates),
            Some(TestSubjectSkill::Rupture) => self.drain_testsubject_rupture_into(actor, smart, updates),
            Some(TestSubjectSkill::GreatPounce) => self.drain_testsubject_great_pounce_into(actor, smart, updates),
            Some(TestSubjectSkill::BurningRoar) => self.drain_testsubject_burning_roar_into(actor, updates),
            // 未选中招式时 k4 扫描已经回落到普攻，这里只推进阶段计数。
            None => {}
        }
        self.advance_testsubject_phase_action_into(actor, updates);
    }

    /// 实验体普攻：一阶段按已行动次数爬升倍率，其余阶段保持 1.0。
    ///
    /// `use_magic` 由调用方按 SklAttack 的口径决定并原样透传；实验体的 k3 不是
    /// SklAttack，因此实战里恒为物理。
    pub fn drain_testsubject_default_attack_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        use_magic: bool,
        updates: &mut RunUpdates,
    ) {
        let scale = self.plain_testsubject_phase_attack_scale(actor);
        updates.add_with(|| RuntimeFrame::replay_update(actor.0 as usize, target.0 as usize, "[0]发起攻击", 0));
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime testsubject default-attack actor: {}", actor.0))
            .runtime
            .get_at(use_magic, &mut self.rng)
            * scale;
        self.drain_plain_attack_with_atp_into(actor, target, use_magic, atp, updates);
    }

    /// 颅骨重击：全体敌人 80% 物理，有效伤害附加易伤。
    fn drain_testsubject_skull_smash_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let enemies = self.testsubject_enemies(actor);
        if enemies.is_empty() {
            return;
        }
        let scale = SKULL_SMASH_SCALE * self.plain_testsubject_phase_attack_scale(actor);
        updates.add_with(|| RunUpdate::new("[0]使用[颅骨重击]", actor.0 as usize, actor.0 as usize, 1));
        for target in enemies {
            if !self.testsubject_target_alive(target) {
                continue;
            }
            let atp = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime testsubject skull-smash actor: {}", actor.0))
                .runtime
                .get_at(false, &mut self.rng)
                * scale;
            let damage = self.drain_plain_attack_with_atp_into(actor, target, false, atp, updates);
            if damage > 0 {
                self.apply_testsubject_skull_fracture_into(actor, target, updates);
            }
        }
    }

    /// 多段爪击：60% 物理 ×（已用次数 + 3）段，累计伤害为正时叠一层迟缓。
    fn drain_testsubject_multi_claw_into(&mut self, actor: EntityIdx, smart: bool, updates: &mut RunUpdates) {
        let Some(target) = self.select_plain_default_attack_target(actor, smart) else {
            return;
        };
        let hit_count = self
            .entities
            .get(actor)
            .and_then(|entity| entity.runtime.boss.as_deref().and_then(|b| b.testsubject.as_ref()))
            .map_or(MULTI_CLAW_EXTRA_HITS, |state| state.multi_claw_used + MULTI_CLAW_EXTRA_HITS);
        updates.add_with(|| RunUpdate::new("[0]使用[多段爪击]", actor.0 as usize, target.0 as usize, 1));
        let mut total_damage = 0;
        for _ in 0..hit_count.max(0) {
            if !self.testsubject_target_alive(target) {
                break;
            }
            let atp = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime testsubject multi-claw actor: {}", actor.0))
                .runtime
                .get_at(false, &mut self.rng)
                * MULTI_CLAW_SCALE;
            total_damage += self.drain_plain_attack_with_atp_into(actor, target, false, atp, updates).max(0);
        }
        if let Some(state) = self
            .entities
            .get_mut(actor)
            .and_then(|entity| entity.runtime.boss_mut().testsubject.as_mut())
        {
            state.multi_claw_used += 1;
        }
        if total_damage > 0 {
            self.apply_testsubject_slow_into(actor, target, updates);
        }
    }

    /// 割裂：60% 物理 × 3。
    fn drain_testsubject_rupture_into(&mut self, actor: EntityIdx, smart: bool, updates: &mut RunUpdates) {
        let Some(target) = self.select_plain_default_attack_target(actor, smart) else {
            return;
        };
        updates.add_with(|| RunUpdate::new("[0]使用[割裂]", actor.0 as usize, target.0 as usize, 1));
        for _ in 0..RUPTURE_HITS {
            if !self.testsubject_target_alive(target) {
                break;
            }
            let atp = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime testsubject rupture actor: {}", actor.0))
                .runtime
                .get_at(false, &mut self.rng)
                * RUPTURE_SCALE;
            self.drain_plain_attack_with_atp_into(actor, target, false, atp, updates);
        }
    }

    /// 大猛扑：225% 物理。
    fn drain_testsubject_great_pounce_into(&mut self, actor: EntityIdx, smart: bool, updates: &mut RunUpdates) {
        let Some(target) = self.select_plain_default_attack_target(actor, smart) else {
            return;
        };
        updates.add_with(|| RunUpdate::new("[0]使用[大猛扑]", actor.0 as usize, target.0 as usize, 1));
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime testsubject great-pounce actor: {}", actor.0))
            .runtime
            .get_at(false, &mut self.rng)
            * GREAT_POUNCE_SCALE;
        self.drain_plain_attack_with_atp_into(actor, target, false, atp, updates);
    }

    /// 燃烧咆哮：全体敌人 80% 魔法，命中走火球术一致的 on_fire 附加灼烧。
    fn drain_testsubject_burning_roar_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let enemies = self.testsubject_enemies(actor);
        if enemies.is_empty() {
            return;
        }
        updates.add_with(|| RunUpdate::new("[0]使用[燃烧咆哮]", actor.0 as usize, actor.0 as usize, 1));
        for target in enemies {
            if !self.testsubject_target_alive(target) {
                continue;
            }
            let atp = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime testsubject burning-roar actor: {}", actor.0))
                .runtime
                .get_at(true, &mut self.rng)
                * BURNING_ROAR_SCALE;
            self.drain_plain_attack_with_atp_and_on_damage_into(
                actor,
                target,
                true,
                atp,
                DamageKind::Default,
                PlainAttackOnDamage::Fire(PLAIN_FIRE_STATE_KEY),
                updates,
            );
        }
    }

    /// 每次实验体行动后：推进阶段计数，三阶段按奇偶轮换无实体。
    fn advance_testsubject_phase_action_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let (enter_no_entity, phase_three) = {
            let Some(entity) = self.entities.get_mut(actor) else {
                return;
            };
            let Some(state) = entity.runtime.boss_mut().testsubject.as_mut() else {
                return;
            };
            state.pending_skill = None;
            state.phase_action_count += 1;
            (state.phase_action_count % 2 == 1, state.phase() == 3)
        };
        // 一、二阶段不参与无实体轮换，保持旧实现的提前返回口径。
        if !phase_three {
            return;
        }
        if enter_no_entity {
            if !self.has_testsubject_no_entity(actor) {
                self.set_testsubject_no_entity(actor, true);
                updates.add_with(|| RunUpdate::new("[1]进入[无实体]状态", actor.0 as usize, actor.0 as usize, 0));
            }
        } else if self.has_testsubject_no_entity(actor) {
            self.set_testsubject_no_entity(actor, false);
            updates.add_with(|| RunUpdate::new("[1]从[无实体]状态中解除", actor.0 as usize, actor.0 as usize, 0));
        }
    }

    /// 实验体当前是否处于无实体。
    pub fn has_testsubject_no_entity(&self, actor: EntityIdx) -> bool {
        self.entities
            .get(actor)
            .is_some_and(|entity| entity.states.entry(PLAIN_TESTSUBJECT_NO_ENTITY_STATE_KEY).is_some())
    }

    /// 进入或解除无实体。
    fn set_testsubject_no_entity(&mut self, actor: EntityIdx, enter: bool) {
        let state_id = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_TESTSUBJECT_NO_ENTITY_STATE_EXPORT)
            .expect("default runtime profile must register core testsubject no-entity state");
        let priority = self
            .registry
            .state(state_id)
            .expect("default runtime core testsubject no-entity state disappeared")
            .priority;
        let Some(entity) = self.entities.get_mut(actor) else {
            return;
        };
        if enter {
            if entity.states.entry(PLAIN_TESTSUBJECT_NO_ENTITY_STATE_KEY).is_none() {
                assert!(
                    entity.states.add_entry(StateEntry::testsubject_no_entity(
                        PLAIN_TESTSUBJECT_NO_ENTITY_STATE_KEY,
                        state_id,
                        priority,
                    )),
                    "runtime testsubject no-entity state should be inserted"
                );
            }
        } else {
            entity.states.clear_legacy_key(PLAIN_TESTSUBJECT_NO_ENTITY_STATE_KEY);
        }
    }

    /// 施加易伤：没有则给 2 层持续时间，已有则追加 1 层。
    pub fn apply_testsubject_skull_fracture_into(&mut self, caster: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        if !self.testsubject_target_alive(target) {
            return;
        }
        let state_id = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_TESTSUBJECT_SKULL_FRACTURE_STATE_EXPORT)
            .expect("default runtime profile must register core testsubject skull-fracture state");
        let priority = self
            .registry
            .state(state_id)
            .expect("default runtime core testsubject skull-fracture state disappeared")
            .priority;
        let Some(entity) = self.entities.get_mut(target) else {
            return;
        };
        let existing = entity
            .states
            .entry(PLAIN_TESTSUBJECT_SKULL_FRACTURE_STATE_KEY)
            .and_then(|entry| match entry.payload {
                StatePayload::SkullFracture { stacks } => Some(stacks),
                _ => None,
            });
        match existing {
            Some(stacks) => assert!(
                entity.states.set_payload(
                    PLAIN_TESTSUBJECT_SKULL_FRACTURE_STATE_KEY,
                    StatePayload::SkullFracture { stacks: stacks + 1 },
                ),
                "runtime skull-fracture state disappeared during extension"
            ),
            None => assert!(
                entity.states.add_entry(StateEntry::skull_fracture(
                    PLAIN_TESTSUBJECT_SKULL_FRACTURE_STATE_KEY,
                    state_id,
                    2,
                    priority,
                )),
                "runtime skull-fracture state should be inserted"
            ),
        }
        updates.add_with(|| RunUpdate::new("[1]进入[易伤]状态", caster.0 as usize, target.0 as usize, 60));
    }

    /// 多段爪击的迟缓：先过免疫检定，再扣 speed + 64 行动点并叠 2 步。
    pub fn apply_testsubject_slow_into(&mut self, caster: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        if !self.testsubject_target_alive(target) {
            return;
        }
        if self.status_immune(target, "slow") {
            return;
        }
        self.apply_plain_slow_into(caster, target, false, updates);
    }

    /// 专属复活：体力归零时异常再生并爬升阶段属性。
    ///
    /// 返回 true 表示已接管本次死亡，调用方必须中止后续死亡链。
    pub fn drain_testsubject_reraise_into(&mut self, target: EntityIdx, updates: &mut RunUpdates) -> bool {
        let Some(entity) = self.entities.get_mut(target) else {
            return false;
        };
        if !is_testsubject(entity) || entity.runtime.hp > 0 {
            return false;
        }
        let Some(state) = entity.runtime.boss.as_deref().and_then(|b| b.testsubject.as_ref()) else {
            return false;
        };
        if state.revives_used >= MAX_REVIVES {
            return false;
        }
        let next_revives = state.revives_used + 1;
        let attr = phase_attr(next_revives);
        let hp = phase_hp(next_revives);
        {
            let state = entity.runtime.boss_mut().testsubject.as_mut().expect("testsubject state checked above");
            state.revives_used = next_revives;
            state.phase_action_count = 0;
            state.multi_claw_used = 0;
            state.pending_skill = None;
        }
        // 属性爬升必须同时落在模板上：运行态由模板派生，只改一侧会被后续刷新抹平。
        entity.template.max_hp = hp;
        entity.runtime.magic_point = PHASE_MAGIC_POINT;
        for (runtime_stat, template_stat) in [
            (&mut entity.runtime.attack, &mut entity.template.attack),
            (&mut entity.runtime.defense, &mut entity.template.defense),
            (&mut entity.runtime.speed, &mut entity.template.speed),
            (&mut entity.runtime.agility, &mut entity.template.agility),
            (&mut entity.runtime.magic, &mut entity.template.magic),
            (&mut entity.runtime.resistance, &mut entity.template.resistance),
            (&mut entity.runtime.wisdom, &mut entity.template.wisdom),
        ] {
            *runtime_stat = attr;
            *template_stat = attr;
        }
        entity.refresh_runtime_stats_from_template();
        entity.runtime.hp = entity.template.max_hp;
        updates.add_newline();
        updates.add_with(|| {
            RunUpdate::new("[1]发生了异常再生", target.0 as usize, target.0 as usize, 80).with_hp_changes([RunUpdateHp {
                player: target.0 as usize,
                before: 0,
                after: entity.runtime.hp,
            }])
        });
        updates.add_with(|| RunUpdate::new("[1]长出了一个新的头颅", target.0 as usize, target.0 as usize, 0));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::lang::{get_lang, keys};

    /// 用默认 profile 构造实验体 Boss 与指定数量敌方英雄的战场。
    ///
    /// 必须走默认 profile：易伤与无实体两个状态要在注册表里拿到 StateId。
    fn testsubject_runtime(heroes: usize, hero_hp: i32) -> CombatRuntime {
        let config = default_custom_runtime_import_config().expect("default runtime profile should build");
        let registry = config.registry;
        let boss_kind = registry
            .player_kind_id_by_export_name(DEFAULT_CORE_BOSS_KIND_EXPORT)
            .expect("default profile should register core boss kind");
        let mut templates = vec![PlayerTemplate::with_kind(1, ID, boss_kind, 0, BASE_HP, BASE_ATTR)];
        for index in 0..heroes {
            templates.push(PlayerTemplate::new(2 + index, format!("hero-{index}"), 1, hero_hp, BASE_ATTR));
        }
        CombatRuntime::from_template(PreparedCombatTemplate::with_registry(templates, registry))
    }

    /// 测试辅助：直接指定本回合要结算的招式，绕开抽签。
    fn queue_skill(runtime: &mut CombatRuntime, skill: TestSubjectSkill) {
        runtime
            .entities
            .get_mut(EntityIdx(0))
            .unwrap()
            .runtime
            .boss_mut()
            .testsubject
            .as_mut()
            .unwrap()
            .pending_skill = Some(skill);
    }

    /// 测试辅助：抽取 RunUpdates 中全部帧更新的消息文本。
    fn messages_of(updates: &RunUpdates) -> Vec<String> {
        updates.updates.iter().map(|update| update.message.to_string()).collect()
    }

    /// 测试辅助：把实验体逼到指定阶段。
    fn force_phase(runtime: &mut CombatRuntime, revives_used: i32) {
        let attr = phase_attr(revives_used);
        let hp = phase_hp(revives_used);
        let entity = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        entity.runtime.boss_mut().testsubject = Some(Box::new(TestSubjectRuntime {
            revives_used,
            phase_action_count: 0,
            multi_claw_used: 0,
            pending_skill: None,
        }));
        entity.template.max_hp = hp;
        entity.runtime.hp = hp;
        entity.runtime.attack = attr;
        entity.template.attack = attr;
    }

    /// 测试辅助：推进 RNG 直到下一次 r127 低于给定阈值。
    fn advance_until_r127_below(runtime: &mut CombatRuntime, level: u32) {
        while {
            let mut probe = runtime.rng.clone();
            probe.r127() >= level
        } {
            runtime.rng.next_u8();
        }
    }

    /// 测试辅助：推进 RNG 直到单次抽签未中。
    ///
    /// 二、三阶段不适用：`r127()` 取值 0..=127，多段爪击与燃烧咆哮的阈值都是 128，
    /// 恒成立，因此这两个阶段没有普攻回落，见 `phase_two_and_three_always_use_a_skill`。
    fn advance_until_phase_misses(runtime: &mut CombatRuntime, level: u32) {
        while {
            let mut probe = runtime.rng.clone();
            probe.r127() < level
        } {
            runtime.rng.next_u8();
        }
    }

    /// 测试辅助：推进 RNG 直到三阶段第一次抽签未中割裂、第二次低于给定阈值。
    fn advance_until_phase_three_second_roll(runtime: &mut CombatRuntime, level: u32) {
        loop {
            let mut probe = runtime.rng.clone();
            if probe.r127() >= RUPTURE_LEVEL && probe.r127() < level {
                return;
            }
            runtime.rng.next_u8();
        }
    }

    /// 测试辅助：推进 RNG 直到三阶段前两次抽签都未中。
    fn advance_until_phase_three_first_two_miss(runtime: &mut CombatRuntime) {
        loop {
            let mut probe = runtime.rng.clone();
            if probe.r127() >= RUPTURE_LEVEL && probe.r127() >= GREAT_POUNCE_LEVEL {
                return;
            }
            runtime.rng.next_u8();
        }
    }

    /// 测试辅助：读取实验体记下的待执行招式。
    fn pending_skill(runtime: &CombatRuntime) -> Option<TestSubjectSkill> {
        runtime
            .entities
            .get(EntityIdx(0))
            .unwrap()
            .runtime
            .boss
            .as_deref()
            .and_then(|b| b.testsubject.as_ref())
            .unwrap()
            .pending_skill
    }

    #[test]
    fn identity_matches_lang_table() {
        assert_eq!(ID, "testsubject");
        assert_eq!(display_name(), DISPLAY_NAME);
        assert_eq!(get_lang(keys::BOSS_NAME_TESTSUBJECT), DISPLAY_NAME);
        assert_eq!(crate::namerena::boss_display_name(ID), DISPLAY_NAME);
    }

    #[test]
    fn phase_attrs_climb_by_forty_and_one_hundred() {
        assert_eq!((phase_attr(0), phase_hp(0)), (80, 100));
        assert_eq!((phase_attr(1), phase_hp(1)), (120, 200));
        assert_eq!((phase_attr(2), phase_hp(2)), (160, 300));
        assert_eq!(MAX_REVIVES, 2);
        assert_eq!(PHASE_MAGIC_POINT, 1000);
    }

    #[test]
    fn boss_action_skills_registers_the_testsubject_entry() {
        assert_eq!(
            crate::namerena::boss_action_skills(ID),
            &[(crate::namerena::BossActionSkill::TestSubject, 0)]
        );
    }

    #[test]
    fn phase_one_rolls_skull_smash_at_sixty_four() {
        let mut runtime = testsubject_runtime(2, 500);
        advance_until_r127_below(&mut runtime, SKULL_SMASH_LEVEL);
        assert!(runtime.roll_plain_testsubject_phase_skill(EntityIdx(0)));
        assert_eq!(pending_skill(&runtime), Some(TestSubjectSkill::SkullSmash));
    }

    #[test]
    fn phase_two_rolls_multi_claw_at_one_twenty_eight() {
        let mut runtime = testsubject_runtime(1, 500);
        force_phase(&mut runtime, 1);
        advance_until_r127_below(&mut runtime, MULTI_CLAW_LEVEL);
        assert!(runtime.roll_plain_testsubject_phase_skill(EntityIdx(0)));
        assert_eq!(pending_skill(&runtime), Some(TestSubjectSkill::MultiClaw));
    }

    #[test]
    fn phase_three_prefers_rupture_then_pounce_then_roar() {
        // 割裂看第一次抽签；大猛扑与燃烧咆哮都要求第一次未中割裂。
        let mut rupture = testsubject_runtime(1, 500);
        force_phase(&mut rupture, 2);
        advance_until_r127_below(&mut rupture, RUPTURE_LEVEL);
        assert!(rupture.roll_plain_testsubject_phase_skill(EntityIdx(0)));
        assert_eq!(pending_skill(&rupture), Some(TestSubjectSkill::Rupture));

        for (level, expected) in [
            (GREAT_POUNCE_LEVEL, TestSubjectSkill::GreatPounce),
            (BURNING_ROAR_LEVEL, TestSubjectSkill::BurningRoar),
        ] {
            let mut runtime = testsubject_runtime(1, 500);
            force_phase(&mut runtime, 2);
            if level == BURNING_ROAR_LEVEL {
                advance_until_phase_three_first_two_miss(&mut runtime);
            } else {
                advance_until_phase_three_second_roll(&mut runtime, level);
            }
            assert!(runtime.roll_plain_testsubject_phase_skill(EntityIdx(0)), "level={level}");
            assert_eq!(pending_skill(&runtime), Some(expected), "level={level}");
        }
    }

    #[test]
    fn single_target_skills_follow_action_wisdom() {
        let actor = EntityIdx(0);
        let cases = [
            (1, 3, TestSubjectSkill::MultiClaw, "[0]使用[多段爪击]"),
            (2, 49, TestSubjectSkill::Rupture, "[0]使用[割裂]"),
            (2, 3, TestSubjectSkill::GreatPounce, "[0]使用[大猛扑]"),
        ];
        // 固定随机流前缀选出指定招式；目标 1 更吸引聪明攻击，随机评分则偏向目标 2。
        let targets = cases.map(|(phase, rng_skip, skill, message)| {
            [0, 64].map(|wisdom| {
                let mut runtime = testsubject_runtime(2, 100_000);
                force_phase(&mut runtime, phase);
                let boss = &mut runtime.entities.get_mut(actor).unwrap().runtime;
                boss.wisdom = wisdom;
                boss.magic_point = PHASE_MAGIC_POINT;
                runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.attract_bits = 2.0_f64.to_bits();
                runtime.entities.get_mut(EntityIdx(2)).unwrap().runtime.attract_bits = 1.0_f64.to_bits();
                for _ in 0..rng_skip {
                    runtime.rng.next_u8();
                }
                let smart = runtime.roll_actor_smart(actor);
                assert_eq!(smart, wisdom != 0);
                let Some(PreparedPlainAction::BossSkill(prepared)) =
                    runtime.prepare_plain_action(actor, smart, Default::default())
                else {
                    panic!("没有选中实验体招式：{skill:?}");
                };
                assert_eq!(pending_skill(&runtime), Some(skill));
                let mut updates = RunUpdates::new();
                runtime.drain_plain_boss_skill_into(actor, prepared, &mut updates);
                updates.updates.iter().find(|update| update.message == message).unwrap().target
            })
        });
        assert_eq!(targets, [[2, 1]; 3]);
    }

    #[test]
    fn phase_one_miss_falls_back_to_plain_attack() {
        let mut runtime = testsubject_runtime(1, 500);
        advance_until_phase_misses(&mut runtime, SKULL_SMASH_LEVEL);
        assert!(!runtime.roll_plain_testsubject_phase_skill(EntityIdx(0)));
        assert_eq!(pending_skill(&runtime), None);
    }

    #[test]
    fn phase_two_and_three_always_use_a_skill() {
        // r127() 取值 0..=127，多段爪击与燃烧咆哮的阈值都是 128，恒成立：
        // 二阶段必然多段爪击，三阶段必然割裂/大猛扑/燃烧咆哮之一。
        assert_eq!(MULTI_CLAW_LEVEL, 128);
        assert_eq!(BURNING_ROAR_LEVEL, 128);
        let mut two = testsubject_runtime(1, 500);
        force_phase(&mut two, 1);
        assert!(two.roll_plain_testsubject_phase_skill(EntityIdx(0)));
        assert_eq!(pending_skill(&two), Some(TestSubjectSkill::MultiClaw));

        let mut three = testsubject_runtime(1, 500);
        force_phase(&mut three, 2);
        advance_until_phase_three_first_two_miss(&mut three);
        assert!(three.roll_plain_testsubject_phase_skill(EntityIdx(0)));
        assert_eq!(pending_skill(&three), Some(TestSubjectSkill::BurningRoar));
    }

    #[test]
    fn phase_roll_burns_no_bytes_for_a_non_testsubject() {
        let mut runtime = testsubject_runtime(1, 500);
        let before = (runtime.rng.i, runtime.rng.j);
        runtime.entities.get_mut(EntityIdx(0)).unwrap().template.name = "impostor".to_owned();
        assert!(!runtime.roll_plain_testsubject_phase_skill(EntityIdx(0)));
        assert_eq!((runtime.rng.i, runtime.rng.j), before);
    }

    #[test]
    fn skull_smash_hits_every_enemy_and_applies_skull_fracture() {
        let mut runtime = testsubject_runtime(3, 400);
        queue_skill(&mut runtime, TestSubjectSkill::SkullSmash);
        let mut updates = RunUpdates::new();
        runtime.drain_testsubject_action_into(EntityIdx(0), true, &mut updates);
        let messages = messages_of(&updates);
        assert!(messages.contains(&"[0]使用[颅骨重击]".to_owned()), "{messages:?}");
        let fractured = (1..=3)
            .filter(|idx| {
                runtime
                    .entities
                    .get(EntityIdx(*idx))
                    .unwrap()
                    .states
                    .entry(PLAIN_TESTSUBJECT_SKULL_FRACTURE_STATE_KEY)
                    .is_some()
            })
            .count();
        assert!(fractured > 0, "至少一个敌人应获得易伤");
    }

    #[test]
    fn skull_fracture_state_amplifies_incoming_damage() {
        let mut runtime = testsubject_runtime(1, 400);
        let skull_fracture = runtime
            .registry
            .state_id_by_export_name(DEFAULT_CORE_TESTSUBJECT_SKULL_FRACTURE_STATE_EXPORT)
            .expect("skull-fracture state should register");
        assert!(
            runtime
                .entities
                .get_mut(EntityIdx(1))
                .unwrap()
                .states
                .add_entry(StateEntry::skull_fracture(
                    PLAIN_TESTSUBJECT_SKULL_FRACTURE_STATE_KEY,
                    skull_fracture,
                    2,
                    SkillPriority(10_000),
                ))
        );
        let before = runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp;
        let mut updates = RunUpdates::new();
        // 走 after_dodge 入口绕开闪避，保证只验证 POST_DEFEND 的易伤放大。
        let damage = runtime.drain_plain_attack_after_dodge_into(
            EntityIdx(0),
            EntityIdx(1),
            false,
            100.0,
            DamageKind::Default,
            None,
            PlainAttackOnDamage::None,
            &mut updates,
        );
        let after = runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp;
        assert!(damage > 0);
        assert_eq!(before - after, damage);
        assert!(
            messages_of(&updates).contains(&"[易伤]使伤害提高".to_owned()),
            "{:?}",
            messages_of(&updates)
        );
    }

    #[test]
    fn multi_claw_hit_count_grows_with_previous_uses() {
        for used in 0..3 {
            let mut runtime = testsubject_runtime(1, 100_000);
            force_phase(&mut runtime, 1);
            queue_skill(&mut runtime, TestSubjectSkill::MultiClaw);
            runtime
                .entities
                .get_mut(EntityIdx(0))
                .unwrap()
                .runtime
                .boss_mut()
                .testsubject
                .as_mut()
                .unwrap()
                .multi_claw_used = used;
            let mut updates = RunUpdates::new();
            runtime.drain_testsubject_action_into(EntityIdx(0), true, &mut updates);
            let state = runtime
                .entities
                .get(EntityIdx(0))
                .unwrap()
                .runtime
                .boss
                .as_deref()
                .and_then(|b| b.testsubject.as_ref())
                .unwrap();
            assert_eq!(state.multi_claw_used, used + 1, "used={used}");
            assert!(
                runtime
                    .entities
                    .get(EntityIdx(1))
                    .unwrap()
                    .states
                    .entry(78)
                    .and_then(StateEntry::slow_value)
                    .is_some(),
                "多段爪击造成伤害后应附加迟缓，used={used}"
            );
        }
    }

    #[test]
    fn phase_three_toggles_no_entity_on_every_other_action() {
        let mut runtime = testsubject_runtime(1, 100_000);
        force_phase(&mut runtime, 2);
        queue_skill(&mut runtime, TestSubjectSkill::Rupture);
        let mut seen = Vec::new();
        for _ in 0..4 {
            let mut updates = RunUpdates::new();
            runtime.drain_testsubject_action_into(EntityIdx(0), true, &mut updates);
            seen.push(runtime.has_testsubject_no_entity(EntityIdx(0)));
        }
        assert_eq!(seen, vec![true, false, true, false]);
    }

    #[test]
    fn no_entity_state_clamps_incoming_damage_to_one() {
        let mut runtime = testsubject_runtime(1, 100_000);
        force_phase(&mut runtime, 2);
        runtime.set_testsubject_no_entity(EntityIdx(0), true);
        let before = runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp;
        let mut updates = RunUpdates::new();
        // 走 after_dodge 入口绕开闪避，保证只验证 POST_DEFEND 的压伤。
        let damage = runtime.drain_plain_attack_after_dodge_into(
            EntityIdx(1),
            EntityIdx(0),
            false,
            500.0,
            DamageKind::Default,
            None,
            PlainAttackOnDamage::None,
            &mut updates,
        );
        let after = runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp;
        assert_eq!(damage, 1);
        assert_eq!(before - after, 1);
    }

    #[test]
    fn reraise_climbs_attrs_and_resets_phase_counters() {
        let mut runtime = testsubject_runtime(1, 100_000);
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 0;
        let mut updates = RunUpdates::new();
        assert!(runtime.drain_testsubject_reraise_into(EntityIdx(0), &mut updates));
        let entity = runtime.entities.get(EntityIdx(0)).unwrap();
        assert_eq!(entity.runtime.hp, 200);
        assert_eq!(entity.template.max_hp, 200);
        assert_eq!(entity.runtime.attack, 120);
        assert_eq!(entity.template.attack, 120);
        assert_eq!(entity.runtime.magic_point, PHASE_MAGIC_POINT);
        assert_eq!(
            entity.runtime.boss.as_deref().and_then(|b| b.testsubject.as_ref()).unwrap().revives_used,
            1
        );
        assert!(messages_of(&updates).contains(&"[1]发生了异常再生".to_owned()));
        assert!(messages_of(&updates).contains(&"[1]长出了一个新的头颅".to_owned()));
    }

    #[test]
    fn reraise_stops_after_two_revives() {
        let mut runtime = testsubject_runtime(1, 100_000);
        force_phase(&mut runtime, MAX_REVIVES);
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 0;
        let mut updates = RunUpdates::new();
        assert!(!runtime.drain_testsubject_reraise_into(EntityIdx(0), &mut updates));
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 0);
    }

    #[test]
    fn die_hook_revives_before_death_is_recorded() {
        let mut runtime = testsubject_runtime(1, 100_000);
        // drain_die_hooks_into 是致死链在 alive = false 与 mark_dead 之前调用的钩子点，
        // 异常再生必须在这里赢，否则后面没有任何机会补救。
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 0;
        let mut updates = RunUpdates::new();
        runtime.drain_die_hooks_into(EntityIdx(0), &mut updates);
        let entity = runtime.entities.get(EntityIdx(0)).unwrap();
        assert_eq!(entity.runtime.hp, 200);
        assert!(entity.runtime.alive);
        assert_eq!(runtime.world.flat_alive().len(), 2);
        assert!(messages_of(&updates).contains(&"[1]发生了异常再生".to_owned()));
    }
}

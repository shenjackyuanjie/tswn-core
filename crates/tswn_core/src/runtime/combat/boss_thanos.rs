//! 灭霸（thanos）Boss 实现。
//!
//! 语义来自线上新版 ?lang=zh 构建（assets/fight-369rVhWe.js）的反混淆产物
//! .tmp_namerena_diff/fight.deobf.js 中的 `_0xab7917 类与默认行动类 `_0x5057e0：
//!
//! - get appendAttr() 返回 [0x27,0x24,0xc,0x10,0x0,0x25,0x14,0x71]，见 [`APPEND_ATTR]；
//! - `_0xab7917 只覆盖 appendAttr 与 createSkills，**没有**覆盖 immuned / immunedx，
//!   所以它的免疫表就是 Boss 基类 `_0x3340e3 的默认 8 键（阈值 192），见 [`IMMUNE]。
//!   默认行动类 `_0x5057e0 上那份 5 键的 `get immuned() 在整份 bundle 里没有任何
//!   读取点（只有实体自己的 immune() 读 this.immuned），属于死代码，不能当作灭霸的免疫表；
//! - createSkills() 只做两件事：先 this.dftAct = new _0x5057e0(this)，再 push 一个
//!   level = 0x50（80）的 _0x1ebbfe。_0x1ebbfe extends _0x3a6405（被动技能基类），
//!   它的 addToProcs() 把自己挂到 owner.preactions，preAction 在 level >= shield.shield
//!   时按 nextInt(1 + trunc(level * 3 / 4)) + 1 追加护盾（护盾对象 _0x423eea 就是带
//!   shield 字段的 postDefends 条目）。这与 run_shield_pre_action_skill
//!   （core.skill.shield）逐行一致，因此映射为护盾（技能键 29，等级 80）；它不继承
//!   _0x2d7e12（ActionSkill 基类），而 addSkillsToProc() 只把 _0x2d7e12 实例收进
//!   actions，所以灭霸的 k4 为空，见 [`action_skills] 与 [`default_skill_levels]；
//! - 默认行动 _0x5057e0.act：敌方存活人数 > 己方存活人数时先报「响指」，再随机让
//!   floor(敌方存活 / 2) 个敌人 hp = 0 并逐个走 onDie；否则普通攻击。见
//!   [`should_snap] 与 [`snap_target_count]。
//!
//! # onDie 的自定义文案参数是 score，不是延迟
//!
//! `_0x31379c.onDie(hp0, owner, rng, updates, [_0x3018a2('ysFN'), hp0 + 0x32]) 的第五参
//! 被 onDie 解构成 `[text, score] 并传给
//! `new _0x411ef0(text, attacker, dieTarget, null, null, score)；`_0x411ef0 的第 6 个
//! 形参是 score（delay0 在第 7 位、默认 0x3e8，delay1 在第 8 位、默认 0x64）。默认死亡文案
//! 走的正是同一个坑位 `[getDieMessage(), 0x32]，与 Rust 侧通用死亡帧
//! emit_plain_lethal_replay_into 里写死的 score = 50 一致；前端只在 score > 0 时把它加给
//! 施法者的战斗分（players.get(caster).addScore(score)）。因此灰烬帧的正确取值是
//! **score = 死者原体力 + 50、delay 保持默认 1000/100**，见 [`dust_score]。
//!
//! # 随机流：抽取必须与死亡链交错
//!
//! JS 的循环体是「splice(nextInt(len)) → onDie(...)」，每次抽取后立刻结算死亡，而 onDie
//! 内部的 dies 回调（护身符复活、史莱姆分裂、马里奥复活等）同样消费随机字节。因此 Rust
//! 侧不能在 select 阶段一次抽完：抽取与死亡链都在
//! [`CombatRuntime::drain_plain_thanos_snap_into] 里交错进行，
//! [`CombatRuntime::select_plain_thanos_snap_targets] 只返回空列表作为「本次为响指」的标记。
//! 抽取直接用 next_i32(len) 对齐 JS 的 nextInt(len)（长度 1 也消费两个字节，与
//! RC4::pick 的单元素短路不同）；命中后按 remove 保序删除，对齐 JS 的 splice。
//!
//! 本模块只放灭霸专属的数值与行为；注册项（名字表、加成表、免疫表、默认技能等级、
//! 文案 key、行动分派）由运行时集成方维护，因此部分公开项只有单测引用。
#![allow(dead_code)]

use crate::namerena::BossActionSkill;

/// Boss 标识，对应 JS bossTypes 的 thanos 键。
pub const ID: &str = "thanos";

/// 8 维加成，对应 JS get appendAttr() 的 [0x27,0x24,0xc,0x10,0x0,0x25,0x14,0x71]。
pub const APPEND_ATTR: [i32; 8] = [39, 36, 12, 16, 0, 37, 20, 113];

/// Boss 基类 `_0x3340e3.get immuned() 的 8 个状态键，阈值 192（`rng.c75）。
///
/// 灭霸没有覆盖 immuned，也没有覆盖 immunedx（= 空，`rng.c94 永不生效），因此未列出的
/// 键（含 fire/poison/disperse）走 `rng.c33（阈值 84）。行动类
/// `_0x5057e0.get immuned() 里的 5 键列表无人读取，不参与判定。
pub const IMMUNE: [&str; 8] = ["assassinate", "charm", "berserk", "half", "curse", "exchange", "slow", "ice"];

/// 通用死亡文案自带的战斗分（JS 0x32）。
pub const DEFAULT_DIE_SCORE: i32 = 50;

/// 灭霸 push 的护盾技能等级（JS 0x50）。
pub const SHIELD_LEVEL: u32 = 80;

/// 护盾在通用技能键表中的编号，见 namerena::skill_name_for_export。
pub const SHIELD_SKILL_KEY: usize = 29;

/// 显示名，对应 lang/zh.json 的 bossName_thanos。
pub fn display_name() -> &'static str { "灭霸" }

/// 响指文案键，对应 lang/zh.json 的 sklThanosSnap = 「[0]打了个[响指]」。
///
/// JS 里写作 `_0x3018a2('fqtA')；用同一份 4 字符哈希反解可得
/// hash4("sklThanosSnap") == "fqtA"，与文案表逐字对应。
pub const SNAP_TEXT_KEY: &str = "sklThanosSnap";

/// 灰烬文案键，对应 lang/zh.json 的 sklThanosDust = 「[1]化成了灰烬」。
///
/// JS 里写作 `_0x3018a2('ysFN')，同样由 hash4("sklThanosDust") == "ysFN" 反解确认。
pub const DUST_TEXT_KEY: &str = "sklThanosDust";

/// 默认行动是否改为「响指」。
///
/// JS 里 alives 是全场存活者，enemies 过滤掉自己与 owner.allyGroup 同队者，己方人数用
/// alives.length - enemies.length 得到，因此**包含灭霸自己**（自己既被排除出敌方，也必然
/// 计入己方）。原判定是 enemies.length <= allies 时普通攻击，所以响指条件是严格大于。
pub fn should_snap(enemy_alive: usize, ally_alive: usize) -> bool { enemy_alive > ally_alive }

/// 响指随机消灭的目标数：JS Math.floor(enemies.length / 2)。
///
/// 每次循环从局部副本里 splice 掉一个随机敌人，因此每个敌人最多被处理一次；1 个敌人时
/// 取到 0（实际也进不了响指分支，因为 1 <= 己方人数）。
pub fn snap_target_count(alive: usize) -> usize { alive / 2 }

/// 灰烬帧的战斗分：JS 里 onDie 的第五参是 [_0x3018a2('ysFN'), hp + 0x32]，其中第二项进入
/// 消息的 score 字段（不是 delay0），因此等于死者原体力 + 50。
pub fn dust_score(hp_before: i32) -> u32 { (hp_before + DEFAULT_DIE_SCORE).max(0) as u32 }

/// k4（Boss 主动技能扫描）条目；灭霸为空。
///
/// JS addSkillsToProc() 只把 _0x2d7e12（ActionSkill）实例收进 actions，灭霸唯一的技能
/// _0x1ebbfe 是 preaction 护盾，因此 k4 没有任何条目。
pub const ACTION_SKILLS: [(BossActionSkill, u32); 0] = [];

/// 返回灭霸的 k4 条目（空表），供 boss_action_skills 直接引用。
pub fn action_skills() -> &'static [(BossActionSkill, u32)] { &ACTION_SKILLS[..] }

/// 灭霸的默认技能等级表，形如 boss_default_skill_levels 的 (技能键, 等级)。
///
/// JS 只 push 一个 level = 0x50 的护盾，因此条目是 (29, 80)。
pub fn default_skill_levels() -> &'static [(usize, u32)] { &[(SHIELD_SKILL_KEY, SHIELD_LEVEL)] }

use super::*;
use crate::runtime::lang::keys;
use crate::runtime::update::RunUpdateHp;

impl CombatRuntime {
    /// 敌方存活列表：全场存活者里排除自己、排除与自己有效队伍同队者。
    ///
    /// 对齐 JS 的 `alives.filter(e => e !== owner && e.group !== owner.allyGroup)：
    /// 自己被显式排除（被迷惑时也不会把自己算成敌人），队伍口径用有效队伍（迷惑会改写
    /// allyGroup），召唤物沿用 owner 的队伍对象因此算同队。
    fn plain_thanos_snap_enemies(&self, actor: EntityIdx) -> PreparedTargetList {
        let actor_team = self.plain_effective_team(actor);
        let mut enemies = PreparedTargetList::new();
        for &idx in self.world.flat_alive() {
            if idx == actor {
                continue;
            }
            if self.entities.get(idx).is_some_and(|entity| entity.runtime.team != actor_team) {
                enemies.push(idx);
            }
        }
        enemies
    }

    /// 判断本次默认行动是否为「响指」，是则返回空列表作为行动标记。
    ///
    /// 己方人数取 `alives.length - enemies.length（含灭霸自己），与 JS 完全一致。不在这里
    /// 抽取目标：JS 的抽取与逐个 onDie 交错，抽取必须留到
    /// [`Self::drain_plain_thanos_snap_into] 里做，否则死亡回调消耗的随机字节会让后续
    /// 抽取偏移。返回空列表时装配方只把它当作「响指」分支标记。
    pub fn select_plain_thanos_snap_targets(&self, actor: EntityIdx) -> Option<PreparedTargetList> {
        let enemies = self.plain_thanos_snap_enemies(actor);
        let allies = self.world.flat_alive().len() - enemies.len();
        should_snap(enemies.len(), allies).then(PreparedTargetList::new)
    }

    /// 结算响指：先报「打了个响指」，再随机让 floor(敌方存活 / 2) 个敌人化为灰烬。
    ///
    /// 循环体与 JS 一致：`splice(nextInt(len), 1) 保序删除后立刻走该目标的 onDie。
    /// `targets 是 select 阶段返回的标记（空列表），真正目标在这里抽取。
    pub fn drain_plain_thanos_snap_into(&mut self, actor: EntityIdx, _targets: PreparedTargetList, updates: &mut RunUpdates) {
        updates.add_with(|| RunUpdate::new_lang(keys::SKL_THANOS_SNAP, actor.0 as usize, actor.0 as usize, 0).without_target());
        let mut enemies = self.plain_thanos_snap_enemies(actor);
        let count = snap_target_count(enemies.len());
        for _ in 0..count {
            if enemies.is_empty() {
                break;
            }
            let index = self.rng.next_i32(enemies.len() as i32) as usize;
            let target = enemies.remove(index);
            if self.drain_plain_thanos_dust_into(actor, target, updates) {
                break;
            }
        }
    }

    /// 单个目标的响指死亡链，对应 JS 的
    /// `onDie(hp0, owner, rng, updates, [dust, hp0 + 0x32])。
    ///
    /// 顺序照抄 legacy：换行 → 灰烬文案（caster = 灭霸，target = 死者，score = hp + 50）
    /// → dies 回调（护身符复活、史莱姆分裂、马里奥复活等）→ 被救回则中止 → 移出队伍与战场
    /// （连带清理使魔）→ 击杀者的 kills 回调。
    fn drain_plain_thanos_dust_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) -> bool {
        let Some(hp_before) = self.entities.get(target).map(|entity| entity.runtime.hp) else {
            return false;
        };
        // JS `if (!_0x31379c["alive"]) continue;`：抽到已经没体力者只跳过，不补抽。
        if hp_before <= 0 {
            return false;
        }
        updates.add_newline();
        updates.add_with(|| {
            RunUpdate::new_lang(
                keys::SKL_THANOS_DUST,
                actor.0 as usize,
                target.0 as usize,
                dust_score(hp_before),
            )
            .with_hp_changes([RunUpdateHp {
                player: target.0 as usize,
                before: hp_before,
                after: 0,
            }])
        });
        self.entities.get_mut(target).expect("runtime thanos snap target disappeared").runtime.hp = 0;
        self.drain_die_hooks_into(target, updates);
        let (hp, team) = self
            .entities
            .get(target)
            .map(|entity| (entity.runtime.hp, entity.runtime.team))
            .expect("runtime thanos snap target disappeared after die hooks");
        if hp > 0 {
            // 灰烬文案已经报出，但死者被复活：不标记死亡，也不走击杀链。
            return false;
        }
        self.entities
            .get_mut(target)
            .expect("runtime thanos snap target disappeared")
            .runtime
            .alive = false;
        self.mark_dead_with_linked_minions_into(target, team, updates);
        // 原版清空最后一支敌队时由 checkWin 中止 onDie：不走击杀回调，也不再抽签。
        if !self.has_alive_enemy_or_pending_spawn(actor) {
            return true;
        }
        if self.entities.get(actor).is_some_and(|entity| entity.runtime.hp > 0) {
            self.drain_kill_hooks_into(actor, target, updates);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::lang::get_lang;

    /// Boss 与英雄都来自手写模板，没有技能与状态，死亡链不消费随机字节。
    fn thanos_runtime(heroes: usize, hero_hp: i32) -> CombatRuntime {
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
        let mut templates = vec![PlayerTemplate::with_kind(1, "thanos", boss_kind, 0, 1000, 3)];
        for index in 0..heroes {
            templates.push(PlayerTemplate::new(2 + index, format!("hero-{index}"), 1, hero_hp, 3));
        }
        CombatRuntime::from_template(PreparedCombatTemplate::with_registry(templates, builder.build()))
    }

    /// 测试辅助：抽取 [`RunUpdates`] 中全部帧更新的消息文本。
    fn messages_of(updates: &RunUpdates) -> Vec<String> {
        updates.updates.iter().map(|update| update.message.to_string()).collect()
    }

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
        assert_eq!(get_lang(SNAP_TEXT_KEY), "[0]打了个[响指]", "文案必须与 JS 的 fqtA 一致");
        assert_eq!(get_lang(DUST_TEXT_KEY), "[1]化成了灰烬");
    }

    #[test]
    fn immune_is_the_boss_base_eight_key_list() {
        // _0x5057e0.get immuned() 的 5 键列表无人读取，真正生效的是 Boss 基类表。
        assert_eq!(
            IMMUNE,
            ["assassinate", "charm", "berserk", "half", "curse", "exchange", "slow", "ice"]
        );
        for key in IMMUNE {
            assert_eq!(crate::namerena::boss_immune_threshold(ID, key), 192, "{key} 应走 Boss 基类阈值");
        }
        for key in ["fire", "poison", "disperse"] {
            assert_eq!(crate::namerena::boss_immune_threshold(ID, key), 84);
        }
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
    fn dust_score_is_hp_plus_fifty_not_a_delay() {
        assert_eq!(dust_score(0), 50);
        assert_eq!(dust_score(1234), 1284);
        assert_eq!(dust_score(-10), 40);
        assert_eq!(DEFAULT_DIE_SCORE, 0x32);
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
    fn snap_routes_only_when_enemies_outnumber_allies() {
        let runtime = thanos_runtime(3, 100);
        assert_eq!(
            runtime.select_plain_thanos_snap_targets(EntityIdx(0)),
            Some(PreparedTargetList::new())
        );

        // 1 个敌人 vs 己方 1 人（灭霸自己）→ 普通攻击。
        let even = thanos_runtime(1, 100);
        assert_eq!(even.select_plain_thanos_snap_targets(EntityIdx(0)), None);

        // 2 个敌人 vs 1 人 → 响指。
        let odd = thanos_runtime(2, 100);
        assert_eq!(
            odd.select_plain_thanos_snap_targets(EntityIdx(0)),
            Some(PreparedTargetList::new())
        );
    }

    #[test]
    fn snap_kills_half_of_enemies_with_newline_and_dust_score() {
        let mut runtime = thanos_runtime(4, 100);
        let mut updates = RunUpdates::new();

        runtime.drain_plain_thanos_snap_into(EntityIdx(0), PreparedTargetList::new(), &mut updates);

        let messages = messages_of(&updates);
        assert_eq!(messages[0], get_lang(keys::SKL_THANOS_SNAP));
        assert_eq!(updates.updates[0].caster, 0);
        assert_eq!(updates.updates[0].score, 0, "响指文案不带分数");
        assert_eq!(updates.updates[0].delay0, crate::runtime::update::DEFAULT_DELAY0_MS);
        assert_eq!(messages[1], "\n");
        assert_eq!(messages[2], get_lang(keys::SKL_THANOS_DUST));
        assert_eq!(messages[3], "\n");
        assert_eq!(messages[4], get_lang(keys::SKL_THANOS_DUST));

        let dust_0 = &updates.updates[2];
        let dust_1 = &updates.updates[4];
        assert_eq!(dust_0.score, 150, "灰烬帧 score = 死者原体力 + 50");
        assert_eq!(dust_1.score, 150);
        assert_eq!(dust_0.delay0, crate::runtime::update::DEFAULT_DELAY0_MS, "灰烬帧延迟保持默认");
        assert_eq!(dust_1.delay0, crate::runtime::update::DEFAULT_DELAY0_MS);
        assert_eq!(dust_0.caster, 0, "[0] 是灭霸");
        assert_ne!(dust_0.target, dust_1.target, "两个目标不能重复");

        let dead = [EntityIdx(dust_0.target as u32), EntityIdx(dust_1.target as u32)];
        for target in dead {
            let entity = runtime.entities.get(target).unwrap();
            assert_eq!(entity.runtime.hp, 0);
            assert!(!entity.runtime.alive);
        }
        assert_eq!(runtime.world.flat_alive().len(), 3, "4 敌 1 我 → 只剩 2 敌 + 灭霸");
    }

    /// 测试用击杀回调：对选中目标消费一个随机字节，并上报一条“击杀回调”消息。
    fn record_snap_kill(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
        let target = context.selected_target().unwrap();
        let roll = context.rng_next_u8();
        context.add_update(RunUpdate::new(
            "击杀回调",
            entry.owner.0 as usize,
            target.0 as usize,
            u32::from(roll),
        ));
    }

    /// 测试辅助：构造灭霸 + 召唤师 + 指定数量其他敌人与联动召唤物的战场，返回 runtime 与召唤物实体索引。
    fn thanos_against_linked_summons(other_enemies: usize, summons: usize) -> (CombatRuntime, EntityIdx) {
        let mut builder = ExtensionRegistryBuilder::default();
        let boss = builder
            .register_player_kind_with_policies(
                "core",
                "boss",
                "core.boss",
                PlayerKindFlags::BOSS,
                PlayerKindPolicies::default(),
            )
            .unwrap();
        let summon = builder
            .register_player_kind_with_policies(
                "core",
                "summon",
                "core.kind.summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
                PlayerKindPolicies::default(),
            )
            .unwrap();
        let kill = builder
            .register_skill_with_hooks(
                "test",
                "record-kill",
                "test.record-kill",
                ProcMask::KILL,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .unwrap();
        let mut thanos = PlayerTemplate::with_kind(1, ID, boss, 0, 1000, 3).with_speed(1).with_speed_points(2049);
        if other_enemies == 0 {
            thanos = thanos.with_skills([kill]);
        }
        let mut players = vec![thanos, PlayerTemplate::new(2, "summoner", 1, 100, 3)];
        for i in 0..other_enemies {
            players.push(PlayerTemplate::new(3 + i, format!("enemy-{i}"), 1, 100, 3));
        }
        let summon_idx = EntityIdx(players.len() as u32);
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(players, builder.build()));
        runtime.set_skill_handler(kill, record_snap_kill);
        for i in 0..summons {
            runtime.effects.push(QueuedEffect::SpawnSilent {
                caster: EntityIdx(1),
                template: PlayerTemplate::with_kind(5 + i, format!("linked-summon-{i}"), summon, 1, 100, 3),
            });
        }
        runtime.flush_effects();
        (runtime, summon_idx)
    }

    #[test]
    fn snap_skips_a_drawn_summon_that_died_with_its_owner_without_redrawing() {
        let (mut runtime, summon) = thanos_against_linked_summons(2, 1);
        let (thanos, summoner) = (EntityIdx(0), EntityIdx(1));
        assert_eq!(
            runtime.world.flat_alive(),
            &[thanos, summoner, EntityIdx(2), EntityIdx(3), summon]
        );
        for _ in 0..5 {
            runtime.rng.next_u8();
        }
        let mut expected_rng = runtime.rng.clone();
        // 原版 sigils RC4：先过步长、smart、MP 门槛，再从四人取下标 0、从余下三人取下标 2。
        for _ in 0..3 {
            expected_rng.next_u8();
        }
        assert_eq!(expected_rng.next_i32(4), 0);
        assert_eq!(expected_rng.next_i32(3), 2);
        assert_eq!((expected_rng.i, expected_rng.j), (12, 60));
        expected_rng.r127(); // 双方还有存活者，本回合正常恢复。

        let round = runtime.run_minimal_round();
        assert_eq!(round.action.unwrap().actor, thanos);
        assert_eq!(round.winner_team, None);
        let frame = round.frame.unwrap();
        let events: Vec<_> = frame
            .updates
            .updates
            .iter()
            .filter(|u| u.message != "\n")
            .map(|u| (u.message.as_ref(), u.target, u.score))
            .collect();
        assert_eq!((events[0].0, events[0].2), (get_lang(keys::SKL_THANOS_SNAP), 0));
        assert_eq!(
            &events[1..],
            &[
                (get_lang(keys::SKL_THANOS_DUST), summoner.0 as usize, 150),
                ("[1]消失了", summon.0 as usize, 50)
            ]
        );
        for dead in [summoner, summon] {
            let entity = runtime.entities.get(dead).unwrap();
            assert_eq!(entity.runtime.hp, 0);
            assert!(!entity.runtime.alive);
        }
        assert_eq!(runtime.world.flat_alive(), &[thanos, EntityIdx(2), EntityIdx(3)]);
        assert_eq!((runtime.rng.i, runtime.rng.j), (13, 73));
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    }

    #[test]
    fn snap_end_skips_kill_hook_and_remaining_draws_after_linked_deaths() {
        for (summons, initial_skip) in [(1, 3), (3, 5)] {
            let (mut runtime, _) = thanos_against_linked_summons(0, summons);
            let (thanos, summoner) = (EntityIdx(0), EntityIdx(1));
            assert_eq!(runtime.world.flat_alive().len(), 2 + summons);
            for _ in 0..initial_skip {
                runtime.rng.next_u8();
            }
            let mut expected_rng = runtime.rng.clone();
            for _ in 0..3 {
                expected_rng.next_u8();
            }
            assert_eq!(expected_rng.next_i32((1 + summons) as i32), 0);

            let round = runtime.run_minimal_round();
            assert_eq!(round.action.unwrap().actor, thanos);
            assert_eq!(round.winner_team, Some(0));
            assert_eq!(runtime.world.flat_alive(), &[thanos]);
            let frame = round.frame.unwrap();
            let events: Vec<_> = frame.updates.updates.iter().filter(|u| u.message != "\n").collect();
            assert_eq!(events[0].message, get_lang(keys::SKL_THANOS_SNAP));
            assert_eq!(events[1].message, get_lang(keys::SKL_THANOS_DUST));
            assert_eq!(events[1].target, summoner.0 as usize);
            assert_eq!(events.len(), 2 + summons);
            assert!(events[2..].iter().all(|event| event.message == "[1]消失了"));
            // 原版即使候选池仍含已连带亡故的使魔也直接终局，不消耗下一抽或击杀回调的字节。
            assert_eq!(
                (runtime.rng.i, runtime.rng.j, runtime.rng.main_val),
                (expected_rng.i, expected_rng.j, expected_rng.main_val)
            );
        }
    }

    #[test]
    fn snap_picks_with_next_int_and_removes_in_order() {
        let mut runtime = thanos_runtime(5, 100);
        let mut reference = runtime.rng.clone();
        let mut pool: Vec<EntityIdx> = runtime.world.flat_alive().iter().copied().filter(|idx| *idx != EntityIdx(0)).collect();
        let mut expected = Vec::new();
        for _ in 0..snap_target_count(pool.len()) {
            let index = reference.next_i32(pool.len() as i32) as usize;
            expected.push(pool.remove(index));
        }

        let mut updates = RunUpdates::new();
        runtime.drain_plain_thanos_snap_into(EntityIdx(0), PreparedTargetList::new(), &mut updates);

        let victims: Vec<EntityIdx> = updates
            .updates
            .iter()
            .filter(|update| update.message == get_lang(keys::SKL_THANOS_DUST))
            .map(|update| EntityIdx(update.target as u32))
            .collect();
        assert_eq!(victims, expected, "抽取必须等价于 nextInt(len) + 保序 splice");
        assert_eq!(runtime.rng.i, reference.i);
        assert_eq!(runtime.rng.j, reference.j);
    }
}

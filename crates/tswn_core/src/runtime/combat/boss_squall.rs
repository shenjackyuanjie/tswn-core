//! Squall 的十项科研技能；伤害、召唤与死亡仍经正式战斗执行链。
use super::*;
use crate::runtime::update::{RunUpdate, RunUpdateHp};

/// 取自本地聊天记录的测试/模型相关原句，只用于演出台词。
pub const SAMPLE_QUOTES: [&str; 5] = [
    "其实不是判断出来，其实就是测出来的",
    "我提一嘴，不要对模型的准确性抱有任何信心",
    "我还没有进行漏率的检查",
    "只是拟合模型",
    "转换不成实际战力的话看着烧心",
];

/// 红温台词取自本地聊天记录中唯一出现“红温”的原话，发送于 2026-11-06 00:50:43。
pub const RED_QUOTE: &str = "我已经要被 6.1sol 整红温了";

/// 暴政一下的台词取自原话“暴政一下”前 1 分 46 秒的同段对话，语气同样是点名后赶人。
pub const TYRANNY_QUOTE: &str = "所以情商低，下一个";

/// 禁言把敌方魔力直接压到 -64；按每回合 +40 的自然恢复，需要两回合才回到能施法的水平。
pub const TYRANNY_MP_FLOOR: i32 = -64;

/// 技能编号固定用于调度与验证，不追加普通技能熟练度或随机扫描项。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResearchSkill {
    Weighted,
    Taunt,
    Team,
    Prune,
    Iron,
    Quake,
    Heal,
    Tyranny,
}

impl CombatRuntime {
    /// 派生独立的演出随机数，读取当前检查点但不消费战斗 RNG，也不窥视后续随机序列。
    pub fn next_squall_sample_quote(&mut self, actor: EntityIdx) -> &'static str {
        let s = self.entities.get(actor).unwrap().runtime.squall.as_ref().unwrap();
        let mut value = ((self.rng.i as u64) << 32)
            ^ (self.rng.j as u64)
            ^ u64::from(s.turn).wrapping_mul(0x9e3779b97f4a7c15)
            ^ u64::from(actor.0);
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^= value >> 31;
        let index = s.last_sample_quote.map_or((value % 5) as u8, |last| (last + 1 + (value % 4) as u8) % 5);
        self.entities.get_mut(actor).unwrap().runtime.squall.as_mut().unwrap().last_sample_quote = Some(index);
        SAMPLE_QUOTES[index as usize]
    }

    /// 每条发动文案后立即插入独立台词段，效果明细随后沿用原版战报。
    fn squall_say(
        &self,
        actor: EntityIdx,
        message: impl Into<std::borrow::Cow<'static, str>>,
        quote: &'static str,
        updates: &mut RunUpdates,
    ) {
        updates.add_newline();
        updates.add(RunUpdate::new(message, actor.0 as usize, actor.0 as usize, 1).without_replay_target());
        updates.add_newline();
        updates.add(
            RunUpdate::new(quote, actor.0 as usize, actor.0 as usize, 0)
                .as_dialogue()
                .without_replay_target(),
        );
        updates.add_newline();
    }

    fn squall_enemies(&self, actor: EntityIdx, main_only: bool) -> Vec<EntityIdx> {
        let team = self.plain_effective_team(actor);
        self.world
            .flat_alive()
            .iter()
            .copied()
            .filter(|idx| {
                self.entities.get(*idx).is_some_and(|e| {
                    e.runtime.active() && self.plain_effective_team(*idx) != team && (!main_only || !e.runtime.is_minion())
                })
            })
            .collect()
    }

    /// 采集正式结算造成的实际生命损失，过量伤害不计；召唤单位不冒充本体输出。
    pub fn record_squall_damage(&mut self, caster: EntityIdx, target: EntityIdx, amount: i32) {
        if self.squall_bosses.is_empty() || amount <= 0 {
            return;
        }
        let Some(source) = self.entities.get(caster) else {
            return;
        };
        if source.runtime.is_minion() {
            return;
        }
        let source_team = self.plain_effective_team(caster);
        let target_team = self.plain_effective_team(target);
        for boss in self.squall_bosses.clone() {
            let boss_team = self.plain_effective_team(boss);
            if source_team == boss_team || target_team != boss_team {
                continue;
            }
            let Some(e) = self.entities.get_mut(boss) else {
                continue;
            };
            if !e.runtime.active() {
                continue;
            }
            if let Some(s) = &mut e.runtime.squall {
                if let Some((_, damage)) = s.damage.iter_mut().find(|(idx, _)| *idx == caster) {
                    *damage += i64::from(amount);
                } else {
                    s.damage.push((caster, i64::from(amount)));
                }
            }
        }
    }

    /// 嘲讽使用覆盖层，不改写隐匿等状态保存的原数值；解除时仅清除本施法者的覆盖。
    #[allow(clippy::collapsible_if)]
    fn clear_squall_taunt(&mut self, boss: EntityIdx, updates: &mut RunUpdates) {
        let pair = self
            .entities
            .get_mut(boss)
            .and_then(|e| e.runtime.squall.as_mut())
            .and_then(|s| s.taunt.take());
        if let Some((a, b, _)) = pair {
            for idx in [a, b] {
                if let Some(e) = self.entities.get_mut(idx) {
                    if e.runtime.squall_taunt.is_some_and(|(owner, _)| owner == boss) {
                        e.runtime.squall_taunt = None;
                        updates.add_newline();
                        updates.add(RunUpdate::new("[1]的[嘲讽变换]解除了", boss.0 as usize, idx.0 as usize, 0));
                    }
                }
            }
        }
    }

    /// 行动前推进专属计时；冻结未获行动时不调用此入口。
    pub fn begin_squall_action(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(s) = self.entities.get_mut(actor).and_then(|e| e.runtime.squall.as_mut()) else {
            return;
        };
        s.turn = s.turn.saturating_add(1);
        s.exposed = false;
        let expired_taunt = s.taunt.is_some_and(|(_, _, until)| s.turn >= until);
        let iron_ended = s.iron_until != 0 && s.turn >= s.iron_until;
        if iron_ended {
            s.iron_until = 0;
        }
        let prune = if s.prune_at != 0 && s.turn >= s.prune_at {
            s.prune_at = 0;
            std::mem::take(&mut s.pruning)
        } else {
            Vec::new()
        };
        if expired_taunt {
            self.clear_squall_taunt(actor, updates);
        }
        if iron_ended {
            updates.add(RunUpdate::new("[0]的[蓄力铁壁]解除了", actor.0 as usize, actor.0 as usize, 0));
        }
        self.try_squall_red(actor, updates);
        self.try_squall_burn(actor, updates);
        if !prune.is_empty() {
            // 发动行与台词仍成对展示；每个目标的回避、护盾和致死独立结算。
            self.squall_say(actor, "[0]的[Top50剪枝]开始结算", "不是所有过阈值的都要留下。", updates);
            for target in prune {
                if !self.squall_can_continue(actor) {
                    break;
                }
                self.squall_attack(actor, target, 2.0, updates);
            }
        }
    }

    /// 净化同时解除专属正面效果、剪枝标记以及关联的嘲讽交换，样本与冷却不被重置。
    pub fn dispel_squall_effects(&mut self, target: EntityIdx, updates: &mut RunUpdates) {
        if self.squall_bosses.is_empty() {
            return;
        }
        let mut cleared = false;
        if let Some(s) = self.entities.get_mut(target).and_then(|e| e.runtime.squall.as_mut()) {
            cleared = s.iron_until != 0 || s.amplify > 0;
            s.iron_until = 0;
            s.amplify = 0;
        }
        if cleared {
            updates.add_newline();
            updates.add(RunUpdate::new(
                "[1]的[蓄力铁壁]和[蓄势]解除了",
                target.0 as usize,
                target.0 as usize,
                0,
            ));
        }
        for boss in self.squall_bosses.clone() {
            let Some(s) = self.entities.get_mut(boss).and_then(|e| e.runtime.squall.as_mut()) else {
                continue;
            };
            if boss == target {
                s.pruning.clear();
                s.prune_at = 0;
            } else if s.pruning.contains(&target) {
                s.pruning.retain(|idx| *idx != target);
                updates.add_newline();
                updates.add(RunUpdate::new("[1]的[剪枝标记]解除了", boss.0 as usize, target.0 as usize, 0));
            }
            let clear_pair = s.taunt.is_some_and(|(a, b, _)| boss == target || a == target || b == target);
            if clear_pair {
                self.clear_squall_taunt(boss, updates);
            }
        }
    }

    pub fn apply_squall_defense(&self, target: EntityIdx, value: &mut RuntimeDefendValue) {
        let Some(s) = self.entities.get(target).and_then(|e| e.runtime.squall.as_ref()) else {
            return;
        };
        if let Some(damage) = value.damage().filter(|n| *n > 0) {
            let factor = if s.iron_until > s.turn { 0.6 } else { 1.0 } * if s.exposed { 1.25 } else { 1.0 };
            value.set_damage((f64::from(damage) * factor).ceil() as i32);
        }
    }

    /// HP 首次低于 50% 时一次性进入红温：全属性永久提高 50%，持续整场，净化不能解除。
    pub fn try_squall_red(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(e) = self.entities.get_mut(actor) else {
            return;
        };
        if !e.runtime.active() || i64::from(e.runtime.hp) * 2 >= i64::from(e.template.max_hp) {
            return;
        }
        let Some(s) = &mut e.runtime.squall else {
            return;
        };
        if s.red {
            return;
        }
        s.red = true;
        // 同时修改基底，普通状态重算后也保留红温的永久增幅；不登记为普通状态，净化无从解除。
        for (runtime, template) in [
            (&mut e.runtime.attack, &mut e.template.attack),
            (&mut e.runtime.defense, &mut e.template.defense),
            (&mut e.runtime.speed, &mut e.template.speed),
            (&mut e.runtime.agility, &mut e.template.agility),
            (&mut e.runtime.magic, &mut e.template.magic),
            (&mut e.runtime.resistance, &mut e.template.resistance),
            (&mut e.runtime.wisdom, &mut e.template.wisdom),
        ] {
            *runtime = *runtime * 3 / 2;
            *template = *template * 3 / 2;
        }
        self.squall_say(actor, "[0]进入[红温]", RED_QUOTE, updates);
        updates.add(RunUpdate::new("[0]的全属性上升50%", actor.0 as usize, actor.0 as usize, 0));
    }

    /// 阶段转换只触发一次，先清空样本再加成；不锁血，也不拦截致死伤害。
    pub fn try_squall_burn(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(e) = self.entities.get_mut(actor) else {
            return;
        };
        if !e.runtime.active() || i64::from(e.runtime.hp) * 4 > i64::from(e.template.max_hp) {
            return;
        }
        let Some(s) = &mut e.runtime.squall else {
            return;
        };
        if s.burned {
            return;
        }
        s.burned = true;
        let samples = s.samples;
        s.samples = 0;
        // 同时修改基底，普通状态重算后也保留最终阶段的永久增幅。
        let power = 100 + i32::from(samples) * 7;
        let speed = 100 + i32::from(samples) * 9;
        e.runtime.attack = e.runtime.attack * power / 100;
        e.template.attack = e.template.attack * power / 100;
        e.runtime.magic = e.runtime.magic * power / 100;
        e.template.magic = e.template.magic * power / 100;
        e.runtime.speed = e.runtime.speed * speed / 100;
        e.template.speed = e.template.speed * speed / 100;
        self.squall_say(actor, "[0]发动[测完1T烧1T]", "号库我不要了。我的单挑呢？", updates);
        updates.add(RunUpdate::new(
            format!("[0]烧毁{}层[测试样本]", samples),
            actor.0 as usize,
            actor.0 as usize,
            0,
        ));
        updates.add_newline();
        updates.add(RunUpdate::new(
            format!(
                "[0]的攻击、魔力上升{}%，速度上升{}%",
                u32::from(samples) * 7,
                u32::from(samples) * 9
            ),
            actor.0 as usize,
            actor.0 as usize,
            0,
        ));
    }

    /// 普通和 no-capture 对局均在正式行动结束后采样，死亡与魅惑后及时撤销嘲讽覆盖。
    pub fn finish_squall_action(&mut self, actor: Option<EntityIdx>, updates: &mut RunUpdates) {
        if self.squall_bosses.is_empty() {
            return;
        }
        for boss in self.squall_bosses.clone() {
            if !self.entities.get(boss).is_some_and(|e| e.runtime.active()) {
                self.clear_squall_taunt(boss, updates);
                continue;
            }
            // 阵营变化后旧交换不能继续作用于已经成为队友的目标。
            let invalid_pair = self
                .entities
                .get(boss)
                .and_then(|e| e.runtime.squall.as_ref())
                .and_then(|s| s.taunt)
                .is_some_and(|(a, b, _)| {
                    self.plain_effective_team(a) == self.plain_effective_team(boss)
                        || self.plain_effective_team(b) == self.plain_effective_team(boss)
                });
            if invalid_pair {
                self.clear_squall_taunt(boss, updates);
            }
            self.try_squall_red(boss, updates);
            self.try_squall_burn(boss, updates);
            let Some(actor) = actor else {
                continue;
            };
            if actor != boss {
                if self.plain_effective_team(actor) != self.plain_effective_team(boss)
                    && self.entities.get(actor).is_some_and(|e| !e.runtime.is_minion())
                {
                    self.entities.get_mut(boss).unwrap().runtime.squall.as_mut().unwrap().observed = true;
                }
                continue;
            }
            let enemies = self.squall_enemies(boss, true);
            let s = self.entities.get_mut(boss).unwrap().runtime.squall.as_mut().unwrap();
            let marked = s
                .damage
                .iter()
                .filter(|(idx, _)| enemies.contains(idx))
                .max_by_key(|(_, n)| *n)
                .map(|(idx, _)| *idx);
            if marked.is_some() {
                s.marked = marked;
            } else if s.marked.is_some_and(|idx| !enemies.contains(&idx)) {
                s.marked = None;
            }
            s.damage.clear();
            let sample = s.observed && !s.burned && s.samples < 6;
            s.observed = false;
            if sample {
                s.samples += 1;
            }
            let burned = s.burned;
            if sample {
                let quote = self.next_squall_sample_quote(boss);
                self.squall_say(boss, "[0]开始测试，获得1层测试样本", quote, updates);
            }
            if burned {
                let loss = (self.entities.get(boss).unwrap().template.max_hp + 99) / 100;
                updates.add_newline();
                updates.add(RunUpdate::new("[0]因[烧库]损失体力", boss.0 as usize, boss.0 as usize, 0));
                if self.apply_plain_attack_damage_into(boss, boss, loss, updates) {
                    self.drain_plain_lethal_damage_into(boss, boss, updates);
                    if !self.squall_can_continue(boss) {
                        self.clear_squall_taunt(boss, updates);
                    }
                }
            }
        }
    }

    fn squall_can_continue(&self, actor: EntityIdx) -> bool { self.entities.get(actor).is_some_and(|e| e.runtime.active()) }

    /// 各次命中独立走原版魔法回避、防御、护盾、反弹与死亡链。
    fn squall_attack(&mut self, actor: EntityIdx, target: EntityIdx, scale: f64, updates: &mut RunUpdates) {
        if !self.squall_can_continue(actor)
            || !self.entities.get(target).is_some_and(|e| e.runtime.active())
            || self.plain_effective_team(actor) == self.plain_effective_team(target)
        {
            return;
        }
        let atp = self.entities.get(actor).unwrap().runtime.get_at(true, &mut self.rng) * scale;
        updates.add_newline();
        self.drain_plain_attack_with_atp_into(actor, target, true, atp, updates);
    }

    /// 输出与回复增益仅在攻击技能兑现时消耗；倍率封顶二倍。
    pub fn squall_take_amplify(&mut self, actor: EntityIdx) -> f64 {
        self.entities
            .get_mut(actor)
            .and_then(|e| e.runtime.squall.as_mut())
            .map_or(1.0, |s| 1.0 + f64::from(std::mem::take(&mut s.amplify).min(100)) / 100.0)
    }

    pub fn select_research_skill(&self, actor: EntityIdx) -> Option<ResearchSkill> {
        let e = self.entities.get(actor)?;
        let s = e.runtime.squall.as_ref()?;
        let ready = |slot: usize| s.turn >= s.next_skill[slot];
        if e.runtime.hp * 100 < e.template.max_hp * 55 && s.heal_count < 3 && ready(5) {
            return Some(ResearchSkill::Heal);
        }
        // 开局第一动仍留给铁壁；之后只要还有人没被压到 -64 就来一次禁言。
        if ready(6)
            && s.turn >= 1
            && self.squall_enemies(actor, false).iter().any(|idx| {
                self.entities
                    .get(*idx)
                    .is_some_and(|target| target.runtime.magic_point > TYRANNY_MP_FLOOR)
            })
        {
            return Some(ResearchSkill::Tyranny);
        }
        if !s.summoned && s.turn >= 2 {
            return Some(ResearchSkill::Team);
        }
        if ready(2)
            && !self
                .squall_enemies(actor, false)
                .iter()
                .all(|idx| !self.entities.get(*idx).unwrap().runtime.is_minion())
        {
            return Some(ResearchSkill::Prune);
        }
        if ready(4) && s.amplify > 0 {
            return Some(ResearchSkill::Quake);
        }
        if ready(3) {
            return Some(ResearchSkill::Iron);
        }
        if ready(1) && self.squall_enemies(actor, true).len() > 1 {
            return Some(ResearchSkill::Taunt);
        }
        if ready(0) && s.samples >= 2 && !s.burned {
            return Some(ResearchSkill::Weighted);
        }
        if ready(4) {
            return Some(ResearchSkill::Quake);
        }
        None
    }

    pub fn drain_squall_action_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if let Some((owner, role, _)) = self.entities.get(actor).and_then(|e| e.runtime.squall_helper) {
            self.squall_helper_action(actor, owner, role, updates);
            return;
        }
        if !self.squall_can_continue(actor) {
            return;
        }
        let Some(skill) = self.select_research_skill(actor) else {
            if let Some(target) = self.select_plain_default_attack_target(actor, true) {
                let amp = self.squall_take_amplify(actor);
                updates.add(RunUpdate::new("[0]发起攻击", actor.0 as usize, target.0 as usize, 0));
                self.squall_attack(actor, target, amp, updates);
            }
            return;
        };
        let s = self.entities.get_mut(actor).unwrap().runtime.squall.as_mut().unwrap();
        if s.active_count == 3 {
            s.active_count = 0;
            let lost = s.samples.min(2);
            s.samples -= lost;
            s.exposed = true;
            self.squall_say(
                actor,
                "[0]发动[我说的是20%，不是Top20%]",
                "我让你测20%，你怎么开始测Top20%的组合了？",
                updates,
            );
            updates.add(RunUpdate::new(
                format!("[0]失去{}层[测试样本]，陷入[我错了]", lost),
                actor.0 as usize,
                actor.0 as usize,
                0,
            ));
            return;
        }
        s.active_count += 1;
        if s.active_count == 3 {
            updates.add(RunUpdate::new("[0]的[模型]即将交付", actor.0 as usize, actor.0 as usize, 0));
        }
        match skill {
            ResearchSkill::Weighted => self.squall_weighted(actor, updates),
            ResearchSkill::Taunt => self.squall_taunt(actor, updates),
            ResearchSkill::Team => self.squall_team(actor, updates),
            ResearchSkill::Prune => self.squall_prune(actor, updates),
            ResearchSkill::Iron => {
                let s = self.entities.get_mut(actor).unwrap().runtime.squall.as_mut().unwrap();
                s.next_skill[3] = s.turn + 4;
                s.iron_until = s.turn + 3;
                s.amplify = (s.amplify + 50).min(100);
                self.squall_say(actor, "[0]使用[蓄力铁壁]", "我说在队友撑不住前，先来蓄力铁壁。", updates);
                updates.add(RunUpdate::new(
                    "[0]进入[铁壁]状态，受到的直接伤害减少40%",
                    actor.0 as usize,
                    actor.0 as usize,
                    0,
                ));
                updates.add_newline();
                updates.add(RunUpdate::new("[0]开始[蓄势]", actor.0 as usize, actor.0 as usize, 0));
            }
            ResearchSkill::Quake => {
                let s = self.entities.get_mut(actor).unwrap().runtime.squall.as_mut().unwrap();
                s.next_skill[4] = s.turn + 4;
                let amp = self.squall_take_amplify(actor);
                self.squall_say(
                    actor,
                    "[0]使用[1437大帝·无死角地裂]",
                    "这套阵型的灵魂，就是以地裂为主。",
                    updates,
                );
                for _ in 0..3 {
                    let targets = self.squall_enemies(actor, false);
                    let divisor = targets.len() as f64 + 0.6000000238418579;
                    for target in targets {
                        if !self.squall_can_continue(actor) {
                            break;
                        }
                        self.squall_attack(actor, target, 2.440000057220459 / divisor * 0.7 * amp, updates);
                    }
                }
            }
            ResearchSkill::Heal => self.squall_heal(actor, updates),
            ResearchSkill::Tyranny => self.squall_tyranny(actor, updates),
        }
    }

    pub fn squall_weighted(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let enemies = self.squall_enemies(actor, true);
        if enemies.is_empty() {
            return;
        }
        let s = self.entities.get_mut(actor).unwrap().runtime.squall.as_mut().unwrap();
        s.samples -= 2;
        s.next_skill[0] = s.turn + 2;
        let target = s.marked.filter(|idx| enemies.contains(idx)).unwrap_or(enemies[0]);
        let other = enemies.iter().copied().find(|idx| *idx != target).unwrap_or(target);
        let amp = self.squall_take_amplify(actor);
        self.squall_say(
            actor,
            "[0]使用[加权平均]",
            "你这个分数很高，但实际打下来不是这么回事。",
            updates,
        );
        updates.add(RunUpdate::new("[0]消耗2层[测试样本]", actor.0 as usize, actor.0 as usize, 0));
        for picked in [target, target, other] {
            if !self.squall_can_continue(actor) {
                break;
            }
            let t = if self.entities.get(picked).is_some_and(|e| e.runtime.active()) {
                Some(picked)
            } else {
                self.squall_enemies(actor, false).first().copied()
            };
            if let Some(t) = t {
                self.squall_attack(actor, t, 0.62 * amp, updates);
            }
        }
    }

    pub fn squall_taunt(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        self.clear_squall_taunt(actor, updates);
        let mut enemies = self.squall_enemies(actor, true);
        // 比较真实索敌评分，而非通常相同的原始吸引力常数。
        enemies.sort_by(|a, b| {
            Self::plain_smart_attack_score(&self.world, *a, &self.entities.get(*a).unwrap().runtime).total_cmp(
                &Self::plain_smart_attack_score(&self.world, *b, &self.entities.get(*b).unwrap().runtime),
            )
        });
        if enemies.len() < 2 {
            return;
        }
        let a = enemies[0];
        let b = *enemies.last().unwrap();
        let low = Self::plain_smart_attack_score(&self.world, a, &self.entities.get(a).unwrap().runtime).max(0.001);
        let high = Self::plain_smart_attack_score(&self.world, b, &self.entities.get(b).unwrap().runtime).max(0.001);
        let a_bits = (self.entities.get(a).unwrap().runtime.attract() * high / low).to_bits();
        let b_bits = (self.entities.get(b).unwrap().runtime.attract() * low / high).to_bits();
        self.entities.get_mut(a).unwrap().runtime.squall_taunt = Some((actor, a_bits));
        self.entities.get_mut(b).unwrap().runtime.squall_taunt = Some((actor, b_bits));
        let s = self.entities.get_mut(actor).unwrap().runtime.squall.as_mut().unwrap();
        s.next_skill[1] = s.turn + 4;
        s.taunt = Some((a, b, s.turn + 3));
        self.squall_say(
            actor,
            "[0]使用[嘲讽变换]",
            "你把人海放到高嘲讽位，把配件放在低嘲讽位了？",
            updates,
        );
        for idx in [a, b] {
            updates.add_newline();
            updates.add(RunUpdate::new(
                format!(
                    "[1]的嘲讽变为{:.0}",
                    Self::plain_smart_attack_score(&self.world, idx, &self.entities.get(idx).unwrap().runtime)
                ),
                actor.0 as usize,
                idx.0 as usize,
                0,
            ));
        }
    }

    pub fn squall_team(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        self.entities.get_mut(actor).unwrap().runtime.squall.as_mut().unwrap().summoned = true;
        self.squall_say(
            actor,
            "[0]使用[现在只有我在做真正的三人科研]",
            "三个人的关系，并没有三个相加那么简单。",
            updates,
        );
        let owner = self.entities.get(actor).unwrap();
        let hp = (owner.template.max_hp * 30 / 100).max(1);
        let team = owner.runtime.team;
        let kind = self
            .registry
            .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
            .expect("core summon kind missing");
        for (role, name) in [(0, "高位辅助"), (1, "低位配件")] {
            let mut template = PlayerTemplate::with_kind(0, name, kind, team, hp, 128);
            template.magic = 128;
            template.defense = 128;
            template.resistance = 128;
            template.speed = 212;
            template.agility = 128;
            template.wisdom = 128;
            template.magic_point = 64;
            // 高位辅助的嘲讽要真正拉走攻击；低位配件靠极高潜行躲开索敌。
            template.attract_bits = if role == 0 { 1.0e12_f64 } else { 1024.0_f64 }.to_bits();
            if role == 1
                && let Some(hide) = self.registry.skill_id_by_export_name(DEFAULT_CORE_HIDE_SKILL_EXPORT)
            {
                template = template.with_skill_loadout(SkillLoadout::from_skill_levels([(hide, 200)]));
            }
            template.id_key_name = format!("Squall·{}@Squall", name);
            template.display_name = name.to_owned();
            let idx = self.entities.next_spawn_idx(&template);
            self.effects.push(QueuedEffect::SpawnWithMessage {
                caster: actor,
                template,
                message: "召唤出[1]".to_owned(),
            });
            self.drain_effects_into(updates);
            let helper = self.entities.get_mut(idx).expect("research helper missing");
            // 召唤即满血。
            helper.runtime.hp = helper.template.max_hp;
            helper.runtime.squall = None;
            helper.runtime.squall_helper = Some((actor, role, 0));
            helper.runtime.policies.damage_share = DamageSharePolicy::None;
        }
    }

    fn squall_helper_action(&mut self, actor: EntityIdx, owner: EntityIdx, role: u8, updates: &mut RunUpdates) {
        // 助手只继承角色分工，不复制科研、烧库或召唤能力。
        let (_, _, turn) = self.entities.get(actor).unwrap().runtime.squall_helper.unwrap();
        self.entities.get_mut(actor).unwrap().runtime.squall_helper = Some((owner, role, turn + 1));
        if role == 0
            && self
                .entities
                .get(owner)
                .is_some_and(|e| e.runtime.active() && self.plain_effective_team(actor) == self.plain_effective_team(owner))
        {
            if turn % 2 == 0 {
                self.drain_plain_iron_skill_into(actor, updates);
            } else {
                self.entities.get_mut(owner).unwrap().runtime.protect_from.retain(|p| p.owner != actor);
                self.entities
                    .get_mut(owner)
                    .unwrap()
                    .runtime
                    .protect_from
                    .push(ProtectLinkRuntime { owner: actor, level: 64 });
                self.entities.get_mut(actor).unwrap().runtime.protect_to = Some(owner);
                updates.add(RunUpdate::new("[0]开始[守护][1]", actor.0 as usize, owner.0 as usize, 0));
            }
        } else if role == 1 {
            let targets = self.squall_enemies(actor, false);
            if turn % 2 == 0 {
                for t in targets {
                    self.squall_attack(actor, t, 0.6, updates);
                }
            } else if let Some(t) = targets.first() {
                self.drain_plain_slow_skill_into(actor, *t, updates);
            }
        } else if let Some(t) = self.squall_enemies(actor, false).first().copied() {
            self.squall_attack(actor, t, 1.0, updates);
        }
    }

    pub fn squall_prune(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let mut targets: Vec<_> = self
            .squall_enemies(actor, false)
            .into_iter()
            .filter(|i| self.entities.get(*i).unwrap().runtime.is_minion())
            .collect();
        targets.sort_by_key(|i| self.entities.get(*i).unwrap().runtime.hp);
        targets.truncate(3);
        let s = self.entities.get_mut(actor).unwrap().runtime.squall.as_mut().unwrap();
        s.next_skill[2] = s.turn + 3;
        s.prune_at = s.turn + 1;
        s.pruning = targets.clone();
        self.squall_say(actor, "[0]使用[Top50剪枝]", "不是所有过阈值的都要留下。先留Top50。", updates);
        for target in targets {
            updates.add_newline();
            updates.add(RunUpdate::new("[1]被标记为[剪枝目标]", actor.0 as usize, target.0 as usize, 0));
        }
    }

    pub fn squall_heal(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let e = self.entities.get_mut(actor).unwrap();
        let before = e.runtime.hp;
        e.runtime.hp = (before + e.template.max_hp * 15 / 100).min(e.template.max_hp);
        let healed = e.runtime.hp - before;
        // 每次只解除一个负面条目，并重算其影响的属性与控制关系。
        let key = e
            .states
            .entries()
            .iter()
            .find(|entry| entry.is_negative_state())
            .map(|entry| entry.legacy_order_key);
        let release = key.and_then(|key| e.states.entry(key)).map(|entry| match entry.payload {
            StatePayload::Ice { .. } => "[0]从[冰冻]中解除",
            StatePayload::Curse { .. } => "[0]从[诅咒]中解除",
            StatePayload::Poison { .. } => "[0]从[中毒]中解除",
            StatePayload::Berserk { .. } => "[0]从[狂暴]中解除",
            StatePayload::Charm { .. } => "[0]从[魅惑]中解除",
            StatePayload::Slow { .. } => "[0]从[迟缓]中解除",
            _ => "[0]的[灼烧]解除了",
        });
        if let Some(key) = key {
            e.states.clear_legacy_key(key);
            e.refresh_runtime_stats_from_template();
        }
        let s = e.runtime.squall.as_mut().unwrap();
        s.next_skill[5] = s.turn + 4;
        s.heal_count += 1;
        s.amplify = (s.amplify + 30).min(100);
        self.squall_say(actor, "[0]使用[聚气治愈，续航才是王道]", "聚气治愈的续航才是王道。", updates);
        let mut line = RunUpdate::new("[0]回复体力[2]点", actor.0 as usize, actor.0 as usize, 0);
        line.param = Some(healed as u32);
        line.hp_changes = Some(
            vec![RunUpdateHp {
                player: actor.0 as usize,
                before,
                after: before + healed,
            }]
            .into_boxed_slice(),
        );
        updates.add(line);
        updates.add_newline();
        if let Some(release) = release {
            updates.add(RunUpdate::new(release, actor.0 as usize, actor.0 as usize, 0));
            updates.add_newline();
        }
        updates.add(RunUpdate::new("[0]开始[蓄势]", actor.0 as usize, actor.0 as usize, 0));
    }

    /// 暴政一下：把每个敌方角色的魔力直接设为 -64，并按地裂那样逐目标各占一行战报。
    pub fn squall_tyranny(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let targets = self.squall_enemies(actor, false);
        if targets.is_empty() {
            return;
        }
        let s = self.entities.get_mut(actor).unwrap().runtime.squall.as_mut().unwrap();
        s.next_skill[6] = s.turn + 4;
        self.squall_say(actor, "[0]使用[暴政一下]", TYRANNY_QUOTE, updates);
        for target in targets {
            if let Some(e) = self.entities.get_mut(target) {
                e.runtime.magic_point = TYRANNY_MP_FLOOR;
            }
            updates.add_newline();
            updates.add(RunUpdate::new("[1]被[0]禁言1分钟", actor.0 as usize, target.0 as usize, 0));
        }
    }
}

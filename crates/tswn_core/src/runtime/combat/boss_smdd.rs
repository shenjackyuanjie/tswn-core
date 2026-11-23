//! smdd（被做局者）Boss：十个围绕“做局、卖牢、猴儿、算力”等群聊梗的科研型技能。
use super::*;
use crate::runtime::entity::SmddRuntime;

#[cfg(test)]
#[path = "boss_smdd_tests.rs"]
mod tests;

/// 台词与技能均取自本地群聊里 3023226682 的原话。
pub const ID: &str = "smdd";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SmddSkill {
    /// 抛开事实不谈：无视防御的真实伤害。
    Fact,
    /// 虎星恒等式2=0：清掉目标两层正面状态。
    Tiger,
    /// 卿言猴儿：召唤一只猴儿。
    Monkey,
    /// 孤常卖牢：永久降防提攻。
    Sell,
    /// 矿机测号：四段小额普攻。
    Mining,
    /// 卷测速：提速并追加一次普攻。
    Speed,
    /// 京爷式嘲讽：吸引攻击并减伤。
    Taunt,
    /// 随机数处理好：随机三选一。
    Random,
    /// 平A到底：五连普攻。
    Flat,
    /// 做局：叠加「局」层数并强化下一次真实伤害。
    Setup,
}

impl CombatRuntime {
    fn next_smdd_skill(&self, actor: EntityIdx) -> SmddSkill {
        let Some(entity) = self.entities.get(actor) else {
            return SmddSkill::Flat;
        };
        let Some(state) = entity.runtime.smdd.as_ref() else {
            return SmddSkill::Flat;
        };
        // 十个技能按行动次数轮转，保证每种风格都能出现；卖牢最多两次、猴儿只召唤一次。
        match state.turn % 10 {
            1 => SmddSkill::Setup,
            2 => {
                if state.summoned {
                    SmddSkill::Flat
                } else {
                    SmddSkill::Monkey
                }
            }
            3 => SmddSkill::Mining,
            4 => {
                if state.stacks >= 2 {
                    SmddSkill::Tiger
                } else {
                    SmddSkill::Setup
                }
            }
            5 => {
                if state.sold < 2 {
                    SmddSkill::Sell
                } else {
                    SmddSkill::Random
                }
            }
            6 => SmddSkill::Flat,
            7 => SmddSkill::Speed,
            8 => SmddSkill::Taunt,
            9 => SmddSkill::Random,
            0 => {
                if state.stacks >= 2 {
                    SmddSkill::Fact
                } else {
                    SmddSkill::Setup
                }
            }
            _ => SmddSkill::Setup,
        }
    }

    /// 被敌方本体命中一次，「局」加一层；上限 8。
    pub(super) fn record_smdd_hit(&mut self, target: EntityIdx) {
        let Some(entity) = self.entities.get_mut(target) else {
            return;
        };
        if let Some(state) = entity.runtime.smdd.as_mut() {
            state.stacks = (state.stacks + 1).min(8);
        }
    }

    pub(super) fn drain_smdd_action_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if !self.entities.get(actor).is_some_and(|e| e.runtime.active()) {
            return;
        }
        {
            let Some(state) = self.entities.get_mut(actor).and_then(|e| e.runtime.smdd.as_mut()) else {
                return;
            };
            state.turn = state.turn.saturating_add(1);
        }
        let skill = self.next_smdd_skill(actor);
        let target = self.select_plain_default_attack_target(actor, true);
        match skill {
            SmddSkill::Setup => self.smdd_setup(actor, updates),
            SmddSkill::Fact => self.smdd_fact(actor, target, updates),
            SmddSkill::Tiger => self.smdd_tiger(actor, target, updates),
            SmddSkill::Monkey => self.smdd_monkey(actor, updates),
            SmddSkill::Sell => self.smdd_sell(actor, updates),
            SmddSkill::Mining => self.smdd_flurry(actor, 4, 0.45, updates),
            SmddSkill::Speed => self.smdd_speed(actor, target, updates),
            SmddSkill::Taunt => self.smdd_taunt(actor, updates),
            SmddSkill::Random => self.smdd_random(actor, target, updates),
            SmddSkill::Flat => self.smdd_flurry(actor, 5, 0.6, updates),
        }
    }

    fn smdd_line(&self, actor: EntityIdx, message: &'static str, quote: &'static str, updates: &mut RunUpdates) {
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

    fn smdd_setup(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let stacks = {
            let Some(state) = self.entities.get_mut(actor).and_then(|e| e.runtime.smdd.as_mut()) else {
                return;
            };
            state.stacks = (state.stacks + 1).min(8);
            state.next_skill[9] = state.turn + 1;
            state.stacks
        };
        self.smdd_line(actor, "[0]开始做局", "怎么入库又猴儿了", updates);
        updates.add(RunUpdate::new(
            format!("[0]累积{stacks}层[局]"),
            actor.0 as usize,
            actor.0 as usize,
            0,
        ));
    }

    fn smdd_fact(&mut self, actor: EntityIdx, target: Option<EntityIdx>, updates: &mut RunUpdates) {
        let Some(target) = target else {
            self.smdd_setup(actor, updates);
            return;
        };
        let stacks = self.smdd_state(actor).map_or(0, |state| state.stacks);
        let amount = 40 + i32::from(stacks) * 25;
        if let Some(state) = self.entities.get_mut(actor).and_then(|e| e.runtime.smdd.as_mut()) {
            state.stacks = 0;
            state.next_skill[0] = state.turn + 3;
        }
        self.smdd_line(actor, "[0]使用[抛开事实不谈]", "抛开事实不谈你确实挺牢的", updates);
        // 真实伤害：直接扣血，不经过防御、护盾与回避。
        updates.add(RunUpdate::new(
            format!("[1]受到{amount}点真实伤害"),
            actor.0 as usize,
            target.0 as usize,
            0,
        ));
        if self.apply_damage_into(actor, target, amount, updates) {
            self.drain_plain_lethal_damage_into(actor, target, updates);
        }
    }

    fn smdd_tiger(&mut self, actor: EntityIdx, target: Option<EntityIdx>, updates: &mut RunUpdates) {
        let Some(target) = target else {
            return;
        };
        let cleared = self.clear_two_buffs(target);
        if let Some(state) = self.entities.get_mut(actor).and_then(|e| e.runtime.smdd.as_mut()) {
            state.next_skill[1] = state.turn + 3;
        }
        self.smdd_line(actor, "[0]使用[虎星恒等式2=0]", "虎星恒等式2=0", updates);
        updates.add(RunUpdate::new(
            format!("[1]的{cleared}层增益被清零"),
            actor.0 as usize,
            target.0 as usize,
            0,
        ));
        self.drain_plain_attack_with_atp_into(actor, target, true, self.plain_at(actor, 1.2, updates), updates);
    }

    fn clear_two_buffs(&mut self, target: EntityIdx) -> u8 {
        let Some(entity) = self.entities.get_mut(target) else {
            return 0;
        };
        let keys: smallvec::SmallVec<[u32; 2]> = entity
            .states
            .entries()
            .iter()
            .filter(|entry| entry.is_positive_state())
            .map(|entry| entry.legacy_order_key)
            .take(2)
            .collect();
        for key in &keys {
            entity.states.clear_legacy_key(*key);
        }
        entity.refresh_runtime_stats_from_template();
        keys.len() as u8
    }

    fn smdd_monkey(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(owner) = self.entities.get(actor) else {
            return;
        };
        let (team, max_hp) = (owner.runtime.team, (owner.template.max_hp * 20 / 100).max(1));
        let kind = self
            .registry
            .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
            .expect("core summon kind missing");
        let mut template = PlayerTemplate::with_kind(0, "猴儿", kind, team, max_hp, 90);
        template.speed = 120;
        template.magic = 90;
        template.defense = 70;
        template.resistance = 70;
        template.agility = 90;
        template.wisdom = 90;
        template.id_key_name = "smdd-monkey".to_owned();
        template.display_name = "猴儿".to_owned();
        if let Some(state) = self.entities.get_mut(actor).and_then(|e| e.runtime.smdd.as_mut()) {
            state.summoned = true;
        }
        self.smdd_line(actor, "[0]使用[卿言猴儿，孰若孤]", "卿言猴儿，孰若孤？", updates);
        self.effects.push(QueuedEffect::SpawnWithMessage {
            caster: actor,
            template,
            message: "召唤出[1]".to_owned(),
        });
        self.drain_effects_into(updates);
    }

    fn smdd_sell(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(entity) = self.entities.get_mut(actor) else {
            return;
        };
        entity.runtime.attack = entity.runtime.attack * 130 / 100;
        entity.template.attack = entity.template.attack * 130 / 100;
        entity.runtime.defense = entity.runtime.defense * 80 / 100;
        entity.template.defense = entity.template.defense * 80 / 100;
        if let Some(state) = entity.runtime.smdd.as_mut() {
            state.sold += 1;
        }
        self.smdd_line(
            actor,
            "[0]使用[孤常卖牢]",
            "卿言猴儿，孰若孤？孤常卖牢，自以为大有所益",
            updates,
        );
        updates.add(RunUpdate::new(
            "[0]攻击上升30%，防御下降20%",
            actor.0 as usize,
            actor.0 as usize,
            0,
        ));
    }

    fn smdd_flurry(&mut self, actor: EntityIdx, hits: u8, scale: f64, updates: &mut RunUpdates) {
        let target = self.select_plain_default_attack_target(actor, true);
        let Some(target) = target else {
            return;
        };
        self.smdd_line(actor, "[0]使用[平A到底]", "30以下ban平A", updates);
        for _ in 0..hits {
            if !self.entities.get(actor).is_some_and(|e| e.runtime.active()) {
                break;
            }
            let Some(current) = self.select_plain_default_attack_target(actor, true) else {
                break;
            };
            self.drain_plain_attack_with_atp_into(actor, current, false, self.plain_at(actor, scale, updates), updates);
        }
        let _ = target;
    }

    fn smdd_speed(&mut self, actor: EntityIdx, target: Option<EntityIdx>, updates: &mut RunUpdates) {
        if let Some(entity) = self.entities.get_mut(actor) {
            entity.runtime.speed = entity.runtime.speed * 160 / 100;
            entity.template.speed = entity.template.speed * 160 / 100;
            if let Some(state) = entity.runtime.smdd.as_mut() {
                state.next_skill[5] = state.turn + 4;
            }
        }
        self.smdd_line(actor, "[0]使用[卷测速]", "可能比留在cn卷竞赛好", updates);
        updates.add(RunUpdate::new("[0]速度上升60%", actor.0 as usize, actor.0 as usize, 0));
        if let Some(target) = target {
            updates.add_newline();
            self.drain_plain_attack_with_atp_into(actor, target, false, self.plain_at(actor, 1.0, updates), updates);
        }
    }

    fn smdd_taunt(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if let Some(entity) = self.entities.get_mut(actor) {
            entity.runtime.attract_bits = 1.0e9_f64.to_bits();
            if let Some(state) = entity.runtime.smdd.as_mut() {
                state.next_skill[6] = state.turn + 5;
                state.taunt_until = state.turn + 1;
            }
        }
        self.smdd_line(actor, "[0]使用[京爷式嘲讽]", "因为托腮这种京爷看不上我们这群贱民", updates);
        updates.add(RunUpdate::new("[0]吸引了全部攻击", actor.0 as usize, actor.0 as usize, 0));
    }

    fn smdd_random(&mut self, actor: EntityIdx, target: Option<EntityIdx>, updates: &mut RunUpdates) {
        if let Some(state) = self.entities.get_mut(actor).and_then(|e| e.runtime.smdd.as_mut()) {
            state.next_skill[7] = state.turn + 2;
        }
        self.smdd_line(actor, "[0]使用[随机数处理好]", "随机数处理好再跑只用花eps时间", updates);
        match self.rng.next_u8() % 3 {
            0 => {
                if let Some(target) = target {
                    updates.add(RunUpdate::new("[1]受到大额伤害", actor.0 as usize, target.0 as usize, 0));
                    self.drain_plain_attack_with_atp_into(actor, target, true, self.plain_at(actor, 2.0, updates), updates);
                }
            }
            1 => {
                let (before, healed) = {
                    let Some(entity) = self.entities.get_mut(actor) else {
                        return;
                    };
                    let before = entity.runtime.hp;
                    entity.runtime.hp = (before + entity.template.max_hp / 4).min(entity.template.max_hp);
                    (before, entity.runtime.hp - before)
                };
                let mut line = RunUpdate::new("[0]回复体力[2]点", actor.0 as usize, actor.0 as usize, 0);
                line.param = Some(healed as u32);
                line.hp_changes = Some(
                    vec![crate::runtime::update::RunUpdateHp {
                        player: actor.0 as usize,
                        before,
                        after: before + healed,
                    }]
                    .into_boxed_slice(),
                );
                updates.add(line);
            }
            _ => {
                if let Some(entity) = self.entities.get_mut(actor) {
                    entity.runtime.speed += 40;
                    entity.template.speed += 40;
                }
                updates.add(RunUpdate::new("[0]速度上升", actor.0 as usize, actor.0 as usize, 0));
            }
        }
    }

    fn smdd_state(&self, actor: EntityIdx) -> Option<&SmddRuntime> { self.entities.get(actor)?.runtime.smdd.as_deref() }

    fn plain_at(&self, actor: EntityIdx, scale: f64, _updates: &mut RunUpdates) -> f64 {
        self.entities.get(actor).map_or(0.0, |entity| f64::from(entity.runtime.attack) * scale)
    }
}

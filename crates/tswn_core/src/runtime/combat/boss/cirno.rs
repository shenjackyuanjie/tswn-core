//! 琪露诺：冰属性攻击、逐技能概率与使用后变化的绝对零度。
//! 来源：KODU16（`kodu16/main`，`cf460c84`）。
use super::super::*;

pub const ID: &str = "cirno";

impl CombatRuntime {
    fn cirno_targets(&mut self, actor: EntityIdx, limit: usize) -> Vec<EntityIdx> {
        let team = self.plain_effective_team(actor);
        let mut targets: Vec<_> = self
            .world
            .flat_alive()
            .iter()
            .copied()
            .filter(|id| self.plain_effective_team(*id) != team && !self.quilt_untargetable(*id))
            .collect();
        self.rng.sort_list(&mut targets);
        targets.truncate(limit);
        targets
    }

    fn cirno_say(&self, actor: EntityIdx, message: &'static str, updates: &mut RunUpdates) {
        updates.add_newline();
        updates.add(RunUpdate::new(message, actor.0 as usize, actor.0 as usize, 1).without_replay_target());
        updates.add_newline();
    }

    fn cirno_atp(&mut self, actor: EntityIdx, magic: bool) -> f64 {
        self.entities.get(actor).map_or(0.0, |entity| entity.runtime.get_at(magic, &mut self.rng))
    }

    /// 沿原版的逐技能概率扫描；未发动或蓝量不足时仍会普攻。
    pub fn drain_cirno_action_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if !self.entities.get(actor).is_some_and(|entity| entity.is_active()) {
            return;
        }
        let cost = i32::from(self.rng.next_u8() & 15) + 8;
        let zero_level = self
            .entities
            .get(actor)
            .and_then(|e| e.runtime.boss.as_deref().and_then(|b| b.cirno.as_ref()))
            .map_or(90, |state| state.zero_level);
        if self.entities.get(actor).unwrap().runtime.magic_point >= cost {
            // 参考初始化中 k1 的顺序为被动、冰瀑、钻石风暴、绝对零度、完美冻结。
            for (skill, level) in [(1, 55), (2, 60), (4, zero_level), (3, 60)] {
                if level <= 0 || self.rng.r127() as i32 >= level {
                    continue;
                }
                self.entities.get_mut(actor).unwrap().runtime.magic_point -= cost;
                match skill {
                    1 => self.cirno_hyousen(actor, updates),
                    2 => self.cirno_diamond(actor, updates),
                    3 => self.cirno_perfect_freeze(actor, updates),
                    _ => self.cirno_absolute_zero(actor, updates),
                }
                return;
            }
        }
        let Some(target) = self.select_plain_default_attack_target(actor, true) else {
            return;
        };
        let runtime = &mut self.entities.get_mut(actor).unwrap().runtime;
        let magic_cost = (runtime.magic - runtime.attack) >> 2;
        let magic = runtime.magic > runtime.attack && runtime.magic_point >= magic_cost;
        if magic {
            runtime.magic_point -= magic_cost;
        }
        self.drain_plain_default_attack_into(actor, target, magic, updates);
    }

    /// 冰瀑读取物理攻击威力，但按魔法抗性结算；不附加冰冻状态。
    fn cirno_hyousen(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(target) = self.select_plain_default_attack_target(actor, true) else {
            return;
        };
        self.cirno_say(actor, "[0]使用[冰符「冰瀑」]", updates);
        let frozen = self.entities.get(target).is_some_and(|e| e.states.is_frozen());
        let atp = self.cirno_atp(actor, false) * 1.1 * if frozen { 1.3 } else { 1.0 };
        self.drain_plain_attack_with_atp_and_on_damage_into(
            actor,
            target,
            true,
            atp,
            DamageKind::Ice,
            PlainAttackOnDamage::None,
            updates,
        );
    }

    /// 钻石风暴至多命中三个敌人；冰冻目标会累积放大后续目标的伤害。
    fn cirno_diamond(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let targets = self.cirno_targets(actor, 3);
        if targets.is_empty() {
            return;
        }
        self.cirno_say(actor, "[0]使用[雪符「钻石风暴」]", updates);
        let mut atp = self.cirno_atp(actor, true) * 0.7;
        for target in targets {
            if !self.entities.get(target).is_some_and(|e| e.is_active()) {
                continue;
            }
            if self.entities.get(target).unwrap().states.is_frozen() {
                atp *= 1.3;
            }
            self.drain_plain_attack_with_atp_and_on_damage_into(
                actor,
                target,
                true,
                atp,
                DamageKind::Ice,
                PlainAttackOnDamage::None,
                updates,
            );
        }
    }

    /// 完美冻结最多三段；任一段回避便结束，命中值为自身敏捷加 100。
    fn cirno_perfect_freeze(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(target) = self.select_plain_default_attack_target(actor, true) else {
            return;
        };
        self.cirno_say(actor, "[0]使用[冻符「完美冻结」]", updates);
        let accuracy = 100 + self.entities.get(actor).unwrap().runtime.agility;
        for _ in 0..3 {
            if !self.entities.get(actor).is_some_and(|e| e.is_active())
                || !self.entities.get(target).is_some_and(|e| e.is_active())
            {
                break;
            }
            let defender = &self.entities.get(target).unwrap().runtime;
            if PlayerRuntime::dodge(accuracy, defender.resistance + defender.agility, &mut self.rng) {
                updates.add(RunUpdate::new("[0][回避]了攻击", target.0 as usize, actor.0 as usize, 20));
                break;
            }
            let frozen = self.entities.get(target).unwrap().states.is_frozen();
            let atp = self.cirno_atp(actor, true) * 0.4 * if frozen { 1.3 } else { 1.0 };
            self.drain_plain_attack_after_dodge_into(
                actor,
                target,
                true,
                atp,
                DamageKind::Ice,
                None,
                PlainAttackOnDamage::None,
                updates,
            );
        }
    }

    /// 绝对零度至多九个目标，冰冻目标逐次乘 1.6；施放后魔力加九、智慧除十一。
    pub fn cirno_absolute_zero(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let targets = self.cirno_targets(actor, 9);
        if targets.is_empty() {
            return;
        }
        self.cirno_say(actor, "[0]使用[冻符「绝对零度」]", updates);
        let mut atp = self.cirno_atp(actor, true) * 0.99;
        if let Some(state) = self.entities.get_mut(actor).and_then(|e| e.runtime.boss_mut().cirno.as_mut()) {
            state.zero_level = state.zero_level.saturating_sub(10);
        }
        for target in targets {
            if !self.entities.get(target).is_some_and(|e| e.is_active()) {
                continue;
            }
            if self.entities.get(target).unwrap().states.is_frozen() {
                atp *= 1.6;
            }
            self.drain_plain_attack_with_atp_and_on_damage_into(
                actor,
                target,
                true,
                atp,
                DamageKind::Ice,
                PlainAttackOnDamage::None,
                updates,
            );
        }
        self.cirno_say(actor, "[0]变[⑨]了", updates);
        if let Some(entity) = self.entities.get_mut(actor) {
            entity.runtime.magic = entity.runtime.magic.saturating_add(9);
            entity.runtime.wisdom /= 11;
        }
    }
}

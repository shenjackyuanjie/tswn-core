//! 第三批东方 Boss 的行动计时、减伤、招架、受伤和复活钩子。
//! 来源：KODU16（`kodu16/main`，`cf460c84`）。
use super::super::*;
impl CombatRuntime {
    pub fn begin_pcb_action(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if self.is_rosmontic_boss(actor, "chen") {
            let enter = self.rng.next_u8() & 15 == 0;
            let leave = self.rng.next_u8() & 7 == 0;
            let cat = self.ros_state(actor).cat;
            if (!cat && enter) || (cat && leave) {
                self.ros_state(actor).cat = !cat;
                let factor = if cat { 0.5 } else { 2.0 };
                self.ros_stat(actor, 2, factor, 0);
                self.ros_stat(actor, 3, factor, 0);
                self.entities.get_mut(actor).unwrap().apply_runtime_stats_from_template(false);
                self.rosmontic_message(actor, if cat { "[0]变回人形" } else { "[0][化猫]" }, updates);
            }
        }
        if self.is_rosmontic_boss(actor, "youmu") && self.ros_state(actor).guard == 1 {
            self.ros_state(actor).guard = 0;
        }
        if self.is_rosmontic_boss(actor, "phantom") {
            let e = self.entities.get(actor).unwrap();
            let members = e.runtime.boss.as_deref().and_then(|b| b.rosmontic.as_ref()).unwrap().phantom_members;
            let count = members.count_ones();
            let factor = if count == 3 { 0.66 } else { 0.5 };
            if count > 1 && f64::from(e.runtime.hp) < f64::from(e.template.max_hp) * factor {
                let choices: Vec<_> = (0..3).filter(|i| members & (1 << i) != 0).collect();
                let lost = choices[self.rng.next_u8() as usize % choices.len()];
                let e = self.entities.get_mut(actor).unwrap();
                e.template.max_hp = (f64::from(e.template.max_hp) * factor) as i32;
                let s = e.runtime.boss_mut().rosmontic.as_mut().unwrap();
                s.phantom_members &= !(1 << lost);
                s.levels[0] = 0;
                let remaining: Vec<_> = (0..3).filter(|i| s.phantom_members & (1 << i) != 0).collect();
                for (n, i) in remaining.into_iter().enumerate() {
                    s.levels[i + 1] = if count == 2 {
                        192
                    } else if n == 0 {
                        67
                    } else {
                        130
                    };
                }
                self.rosmontic_message(
                    actor,
                    [
                        "[0]失去了[露娜萨·普莉兹姆利巴]",
                        "[0]失去了[梅露兰·普莉兹姆利巴]",
                        "[0]失去了[莉莉卡·普莉兹姆利巴]",
                    ][lost],
                    updates,
                );
            }
        }
        if self.is_rosmontic_boss(actor, "yuyuko") {
            let s = self.ros_state(actor);
            let prepared = s.butterfly_prepared;
            let revived = s.butterfly_revived;
            if prepared {
                self.ros_cleanse(actor);
            }
            if revived {
                let turns = self.ros_state(actor).butterfly_turns;
                self.ros_state(actor).butterfly_turns = (turns - 1).max(-1);
            } else if prepared {
                let s = self.ros_state(actor);
                s.butterfly_turns = (s.butterfly_turns + 1).min(3);
                let e = self.entities.get_mut(actor).unwrap();
                // 参考版使用浮点 HP；整数引擎保留最后一点，避免缩血绕过死亡链。
                e.runtime.hp = ((f64::from(e.runtime.hp) * 0.8) as i32).max(1);
                e.template.max_hp = ((f64::from(e.template.max_hp) * 0.8) as i32).max(1);
                self.rosmontic_message(actor, "[0]的生命、生命上限减少20%", updates);
            }
        }
    }

    /// 神现只按受益者自己的实际行动倒计时；施放当回合也算一回合。
    pub fn end_pcb_action(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(s) = self.entities.get_mut(actor).and_then(|e| e.runtime.boss_mut().rosmontic.as_mut()) else {
            return;
        };
        if s.divine_turns > 0 {
            s.divine_turns -= 1;
            if s.divine_turns == 0 {
                self.entities.get_mut(actor).unwrap().apply_runtime_stats_from_template(false);
                self.rosmontic_message(actor, "[0]从[神现]中解除", updates);
            }
        }
    }

    /// 在普通护盾前处理常驻防御；先耗掉招架再反击，防止双方招架递归。
    pub fn pcb_defend(&mut self, target: EntityIdx, value: &mut RuntimeDefendValue, updates: &mut RunUpdates) -> bool {
        let Some(damage) = value.damage().filter(|d| *d > 0) else {
            return false;
        };
        if self.is_rosmontic_boss(target, "letty") && value.kind() == DamageKind::Ice {
            value.set_damage(0);
            self.rosmontic_message(target, "[0][免疫]冰冻伤害", updates);
            return true;
        }
        if self.is_rosmontic_boss(target, "phantom") {
            let count = self.ros_state(target).phantom_members.count_ones() as i32;
            value.set_damage(damage * (100 - 15 * count) / 100);
        }
        if self.is_rosmontic_boss(target, "yuyuko") {
            let s = self.ros_state(target);
            if s.butterfly_revived && s.butterfly_turns >= 0 {
                value.set_damage(0);
                return true;
            }
        }
        if self.is_rosmontic_boss(target, "youmu")
            && self.ros_state(target).guard == 1
            && !self
                .entities
                .get(value.caster())
                .and_then(|e| e.runtime.boss.as_deref().and_then(|b| b.rosmontic.as_ref()))
                .is_some_and(|s| s.parry_countering)
        {
            self.ros_state(target).guard = 2;
            self.entities.get_mut(target).unwrap().runtime.move_state.speed_points += 2048;
            value.set_damage(0);
            self.rosmontic_message(target, "[0][招架]，发动[空观剑「六根清净斩」]", updates);
            self.ros_state(target).parry_countering = true;
            for _ in 0..3 {
                self.ros_hit(target, value.caster(), false, 15360.0, true, PlainAttackOnDamage::None, updates);
            }
            self.ros_state(target).parry_countering = false;
            return true;
        }
        false
    }

    /// 结晶护盾在攻击防御前生成，按受击前缺失生命计算。
    pub fn pcb_letty_shield(&mut self, target: EntityIdx, updates: &mut RunUpdates) {
        if self.is_rosmontic_boss(target, "letty") && self.rng.next_u8() < 128 {
            let e = self.entities.get_mut(target).unwrap();
            if e.runtime.hp > 0 {
                e.runtime.shield = (140.0 * f64::from(e.template.max_hp - e.runtime.hp) / f64::from(e.template.max_hp)) as i32;
                self.rosmontic_message(target, "[0]触发[「结晶之银」]，获得护盾", updates);
            }
        }
    }

    pub fn pcb_after_damage(&mut self, target: EntityIdx, updates: &mut RunUpdates) {
        if self.is_rosmontic_boss(target, "yuyuko") {
            let s = self.ros_state(target);
            if s.break_on_damage && s.charged_skill.take().is_some() {
                s.break_on_damage = false;
                s.energy = 0;
                self.rosmontic_message(target, "[0]被打断了", updates);
            }
        }
    }

    pub fn revive_pcb_into(&mut self, target: EntityIdx, updates: &mut RunUpdates) -> bool {
        if !self.is_rosmontic_boss(target, "yuyuko") {
            return false;
        }
        let e = self.entities.get_mut(target).unwrap();
        if e.runtime.hp > 0 {
            return false;
        }
        let s = e.runtime.boss_mut().rosmontic.as_mut().unwrap();
        if s.butterfly_revived {
            return false;
        }
        s.butterfly_revived = true;
        s.levels[0] = 100;
        s.levels[5] = 0;
        e.runtime.hp = 10;
        self.rosmontic_message(target, "[0]触发[「反魂蝶」]", updates);
        true
    }

    /// 式神死亡只向直接召唤者返还一次属性，重复死亡检查不得叠加。
    pub fn pcb_child_died(&mut self, target: EntityIdx) {
        let Some(e) = self.entities.get_mut(target) else {
            return;
        };
        if e.runtime.hp > 0 || !e.runtime.alive {
            return;
        }
        let Some(s) = e.runtime.boss_mut().rosmontic.as_mut() else {
            return;
        };
        if s.child_death_notified || !(20..=22).contains(&s.child_kind) {
            return;
        }
        s.child_death_notified = true;
        let role = s.child_kind;
        let owner = e.runtime.owner;
        if !self.entities.get(owner).is_some_and(|e| e.runtime.alive && e.runtime.hp > 0) {
            return;
        }
        for (index, add) in match role {
            20 => &[(1, 10), (5, 10)][..],
            21 => &[(0, 20)][..],
            _ => &[(2, 12), (3, 12)][..],
        } {
            self.ros_stat(owner, *index, 1.0, *add);
        }
    }

    /// 迟缓叠层与行动延迟分开指定，以对应笼中鸟、诱蛾灯和伦敦人偶。
    pub fn pcb_slow(&mut self, actor: EntityIdx, target: EntityIdx, layers: i32, delay: i32, updates: &mut RunUpdates) {
        let id = self.registry.state_id_by_export_name(DEFAULT_CORE_SLOW_STATE_EXPORT).unwrap();
        let priority = self.registry.state(id).unwrap().priority;
        let e = self.entities.get_mut(target).unwrap();
        let step = e.states.entry(78).and_then(StateEntry::slow_value).unwrap_or(0) + layers;
        if e.states.entry(78).is_some() {
            e.states.set_payload(78, StatePayload::Slow { step });
        } else {
            e.states.add_entry(StateEntry::slow(78, id, step, priority));
            e.refresh_runtime_stats_from_template();
        }
        e.runtime.move_state.speed_points -= delay;
        self.ros_message(actor, target, "[1]进入[迟缓]状态", updates);
    }

    pub fn pcb_destroy(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let e = self.entities.get(target).unwrap();
        if e.runtime.hp > 0 && e.runtime.hp * 4 < e.template.max_hp {
            self.ros_message(actor, target, "[0]触发[「掌中的破坏者」]，[1]被毁灭了", updates);
            self.entities.get_mut(target).unwrap().runtime.hp = 0;
            self.drain_plain_lethal_damage_into(actor, target, updates);
        }
    }
}

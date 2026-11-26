//! 帕秋莉五行连锁、咲夜时停连击和蕾米莉亚的鲜红印记。
//! 来源：KODU16（`kodu16/main`，`cf460c84`）。
use super::super::*;
impl CombatRuntime {
    pub fn ros_patchouli(&mut self, actor: EntityIdx, skill: usize, targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        const TEXT: [&str; 11] = [
            "[0]使用[金符「银龙」]",
            "[0]使用[土金符「翡翠巨石」]",
            "[0]使用[木符「翠绿风暴」]",
            "[0]使用[水木符「水精灵」]",
            "[0]使用[水符「湖葬」]",
            "[0]使用[金水符「水银之毒」]",
            "[0]使用[火符「火神的光辉」]",
            "[0]使用[木火符「森林大火」]",
            "[0]使用[土符「三石塔的震动」]",
            "[0]使用[火土符「环状熔岩带」]",
            "[0]使用[火水木金土符「贤者之石」]",
        ];
        self.rosmontic_message(actor, TEXT[skill], updates);
        if skill == 10 {
            if self.ros_state(actor).elements != 31 {
                return;
            }
            let atp = self.ros_atp(actor, true) * 3.6;
            for t in targets {
                self.ros_hit(actor, t, true, atp, false, PlainAttackOnDamage::None, updates);
            }
            return;
        }
        let element = skill / 2;
        self.ros_state(actor).elements |= 1 << element;
        self.ros_stat(actor, [2, 3, 5, 4, 1][element], 1.12, 0);
        let magical_atp = matches!(skill, 1 | 8 | 9);
        let atp = self.ros_atp(actor, magical_atp);
        match skill {
            0 | 1 | 8 | 9 => {
                let multiplier = match skill {
                    0 => 1.3,
                    1 => 0.95,
                    _ => 0.7,
                };
                let effect = if skill == 9 {
                    PlainAttackOnDamage::RosmonticBurn
                } else {
                    PlainAttackOnDamage::None
                };
                for t in targets {
                    self.ros_hit(actor, t, true, atp * multiplier, false, effect, updates);
                }
            }
            2 | 6 | 7 => {
                let times = if skill == 6 { 1 } else { 2 };
                let multiplier = match skill {
                    2 => 0.65,
                    6 => 1.0,
                    _ => 0.75,
                };
                let effect = if skill == 2 {
                    PlainAttackOnDamage::None
                } else {
                    PlainAttackOnDamage::RosmonticBurn
                };
                for _ in 0..times {
                    self.ros_hit(actor, targets[0], true, atp * multiplier, false, effect, updates);
                }
            }
            _ => {
                let target = targets[0];
                let times = if skill == 3 { 2 } else { 1 };
                for _ in 0..times {
                    if !self.entities.get(target).unwrap().is_active() {
                        break;
                    }
                    let magic = self.entities.get(actor).unwrap().runtime.magic;
                    let resistance = self.entities.get(target).unwrap().runtime.resistance;
                    if self.ros_immune(target, "slow") || self.roll_plain_dodge(target, magic, resistance, false) {
                        break;
                    }
                    // 每段水系只叠一层迟缓；通用减速默认叠两层，施加后扣回差值。
                    self.apply_plain_slow_into(actor, target, false, updates);
                    let e = self.entities.get_mut(target).unwrap();
                    if let Some(step) = e.states.entry(PLAIN_SLOW_STATE_KEY).and_then(StateEntry::slow_value) {
                        e.states.set_payload(PLAIN_SLOW_STATE_KEY, StatePayload::Slow { step: step - 1 });
                    }
                    let hit_atp = self.ros_atp(actor, false) * if skill == 5 { 1.7 } else { 0.7 };
                    self.ros_hit(actor, target, true, hit_atp, true, PlainAttackOnDamage::None, updates);
                }
            }
        }
    }

    /// 燃烧首击维持一轮；再次点燃叠加未结算威力并重置为两轮。
    pub fn apply_rosmontic_burn(&mut self, caster: EntityIdx, target: EntityIdx, damage: i32, updates: &mut RunUpdates) {
        if damage <= 4 || !self.entities.get(target).is_some_and(|e| e.runtime.hp > 0) || self.ros_immune(target, "firex") {
            return;
        }
        let atp = self.ros_atp(caster, true) * 1.2;
        let s = self.ros_state(target);
        let previous = f64::from_bits(s.burn_atp_bits);
        s.burn_atp_bits = (if s.burn_turns > 0 { previous + atp } else { atp }).to_bits();
        s.burn_turns = if s.burn_turns > 0 { 2 } else { 1 };
        s.burn_owner = Some(caster);
        self.ros_message(caster, target, "[1]被[点燃]了", updates);
    }

    pub fn ros_sakuya(&mut self, actor: EntityIdx, skill: usize, targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        match skill {
            0 => {
                self.ros_state(actor).levels[0] = 0;
                self.ros_state(actor).time_stop = 1;
                self.rosmontic_message(actor, "[0]使用[幻世「世界」]", updates);
                for t in targets {
                    if self.is_rosmontic_boss(t, "sakuya") || self.is_rosmontic_boss(t, "remilia") {
                        continue;
                    }
                    let r = &mut self.entities.get_mut(t).unwrap().runtime;
                    r.move_state.speed_points -= 8192 + r.speed * 10;
                    self.ros_message(actor, t, "[1]陷入[时停]", updates);
                }
            }
            1 => {
                self.rosmontic_message(actor, "[0]发动[奇术「幻惑误导」]", updates);
                let child = self.ros_spawn(actor, 3, updates);
                self.ros_state(actor).child = Some(child);
                self.entities.get_mut(actor).unwrap().runtime.move_state.speed_points += 1600;
            }
            _ => {
                let target = targets[0];
                let stopped = self.ros_state(actor).time_stop > 0;
                let atp = self.ros_atp(actor, false) * if skill == 2 { 0.21 } else { 0.14 };
                let (defense, agility) = {
                    let r = &self.entities.get(target).unwrap().runtime;
                    (r.defense, r.agility)
                };
                if skill == 3 {
                    let r = &mut self.entities.get_mut(target).unwrap().runtime;
                    r.defense = if stopped {
                        0
                    } else {
                        (f64::from(defense) * 0.5).round() as i32
                    };
                    if stopped {
                        r.agility = 0;
                    }
                }
                self.ros_message(
                    actor,
                    target,
                    if skill == 2 {
                        "[0]使用[银符「银之跳跃」]"
                    } else {
                        "[0]使用[伤魂「灵魂雕塑」]"
                    },
                    updates,
                );
                let hits = if skill == 2 { if stopped { 14 } else { 7 } } else { 11 };
                for _ in 0..hits {
                    self.ros_hit(actor, target, false, atp, stopped, PlainAttackOnDamage::None, updates);
                }
                // 无论中途是否击杀，都恢复临时移除的防御和敏捷。
                if skill == 3 {
                    let r = &mut self.entities.get_mut(target).unwrap().runtime;
                    r.defense = defense;
                    r.agility = agility;
                }
            }
        }
    }

    pub fn ros_remilia(&mut self, actor: EntityIdx, skill: usize, targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        match skill {
            1 => {
                let charge = self.entities.get(actor).unwrap().runtime.charge.active;
                let s = self.ros_state(actor);
                s.levels[1] = s.levels[1] * 3 / 10;
                s.aura = true;
                s.aura_bonus = charge;
                s.scarlet += 1;
                self.entities.get_mut(actor).unwrap().apply_runtime_stats_from_template(false);
                self.rosmontic_message(actor, "[0]使用[夜王「德古拉的摇篮」]", updates);
                self.rosmontic_message(actor, "[0]：[可怜鬼们，见识一下斯卡蕾特的威严吧！]", updates);
            }
            2 => {
                self.ros_message(actor, targets[0], "[0]使用[必杀「碎心」]", updates);
                let atp = self.ros_atp(actor, false) * 1.25;
                self.ros_hit(
                    actor,
                    targets[0],
                    false,
                    atp,
                    true,
                    PlainAttackOnDamage::RosmonticAbsorbFifth,
                    updates,
                );
            }
            3 => {
                self.rosmontic_message(actor, "[0]使用[红符「绯红之击」]", updates);
                let atp = self.ros_atp(actor, true) * 0.7;
                for t in targets {
                    self.ros_hit(actor, t, true, atp, false, PlainAttackOnDamage::Absorb, updates);
                }
            }
            _ => {
                self.ros_message(actor, targets[0], "[0]使用[神枪「冈格尼尔之枪」]", updates);
                let atp = self.ros_atp(actor, false) * 3.2;
                if self.rng.next_u8() & 63 > 31 {
                    self.ros_hit(
                        actor,
                        targets[0],
                        false,
                        atp,
                        true,
                        PlainAttackOnDamage::RosmonticAbsorbFifth,
                        updates,
                    );
                } else {
                    self.rosmontic_message(actor, "[0]的攻击落空了", updates);
                }
            }
        }
    }
}

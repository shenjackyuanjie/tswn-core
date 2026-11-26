//! 灵梦的结界和梦想天生，魔理沙的八卦炉、偷取与两段彗星。
//! 来源：KODU16（`kodu16/main`，`cf460c84`）。
use super::super::*;
impl CombatRuntime {
    pub fn ros_reimu(&mut self, actor: EntityIdx, skill: usize, targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        match skill {
            1 => {
                if self.ros_state(actor).dream == 0 {
                    let s = self.ros_state(actor);
                    s.dream = 1;
                    s.levels[1] = 700;
                    self.rosmontic_message(actor, "[0] [ᗜ˰ᗜ#]", updates);
                } else {
                    let s = self.ros_state(actor);
                    s.dream = 0;
                    s.levels[1] = 0;
                    self.rosmontic_message(actor, "[0]使用[「梦想天生」]", updates);
                    let atp = self.ros_atp(actor, true) * 7.0;
                    for target in targets {
                        if !self.is_rosmontic_boss(target, "reimu") {
                            self.ros_hit(actor, target, true, atp, true, PlainAttackOnDamage::None, updates);
                        }
                    }
                }
            }
            2 => {
                // 包含施放当回合在内，维持四次自身行动；小伤害只扣 1，不耗结界容量。
                self.ros_state(actor).barrier_turns = 4;
                self.rosmontic_message(actor, "[0]发动[梦符「二重结界」]", updates);
            }
            3 => {
                let target = targets[0];
                self.ros_message(actor, target, "[0]使用[灵符「梦想封印」]", updates);
                let atp = self.ros_atp(actor, true);
                if self.ros_immune(target, "disperse") {
                    return;
                }
                self.dispel_rosmontic_into(target, updates);
                let e = self.entities.get_mut(target).unwrap();
                let keys: Vec<_> = e
                    .states
                    .entries()
                    .iter()
                    .filter(|s| matches!(s.payload, StatePayload::Iron { .. } | StatePayload::ShieldValue(_)))
                    .map(|s| s.legacy_order_key)
                    .collect();
                for key in keys {
                    e.states.clear_legacy_key(key);
                }
                e.runtime.shield = 0;
                e.states
                    .clear_compressed_legacy_state(crate::runtime::entity::CompressedLegacyState::Shield);
                let multiplier = if e.runtime.is_combat_minion() { 2.0 } else { 1.0 };
                self.ros_hit(
                    actor,
                    target,
                    true,
                    atp * multiplier,
                    false,
                    PlainAttackOnDamage::Disperse,
                    updates,
                );
            }
            _ => {
                self.rosmontic_message(actor, "[0]使用[神技「八方龙杀阵」]", updates);
                let atp = self.ros_atp(actor, true) * 0.8;
                for target in targets {
                    let multiplier = if self.entities.get(target).unwrap().runtime.is_combat_minion() {
                        2.0
                    } else {
                        1.0
                    };
                    self.ros_hit(actor, target, true, atp * multiplier, true, PlainAttackOnDamage::None, updates);
                }
            }
        }
    }

    pub fn ros_marisa(&mut self, actor: EntityIdx, skill: usize, targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        let target = targets[0];
        match skill {
            0 => {
                self.ros_state(actor).levels[0] = 0;
                self.ros_state(actor).energy = 6;
                self.ros_message(actor, target, "[0]：[等我死了再还你~]", updates);
                let index = (self.rng.next_u8() & 7) as usize;
                let amount = i32::from(self.rng.next_u8() & 63) / 2 + 33;
                if index < 7 {
                    let r = &self.entities.get(target).unwrap().runtime;
                    let value = [r.attack, r.defense, r.speed, r.agility, r.magic, r.resistance, r.wisdom][index];
                    let stolen = amount.min((value - 1).max(0));
                    self.ros_stat(target, index, 1.0, -stolen);
                    // 参考实现低属性分支先把目标设为 1，再读取，因此不会给自己加属性。
                    if value > amount {
                        self.ros_stat(actor, index, 1.0, stolen);
                    }
                    const TEXT: [&str; 7] = [
                        "[0]偷走了[1]的攻击",
                        "[0]偷走了[1]的防御",
                        "[0]偷走了[1]的速度",
                        "[0]偷走了[1]的敏捷",
                        "[0]偷走了[1]的法力",
                        "[0]偷走了[1]的法抗",
                        "[0]偷走了[1]的智力",
                    ];
                    self.ros_message(actor, target, TEXT[index], updates);
                } else {
                    self.rosmontic_message(actor, "[0] [大失败]", updates);
                }
                self.entities.get_mut(actor).unwrap().runtime.move_state.speed_points += 1600;
            }
            1 => {
                if self.ros_state(actor).levels[1] != 0 {
                    // 蓄势阶段锁定下一次行动，不在下一回合重复支付 MP。
                    self.ros_state(actor).levels[1] = 0;
                    self.ros_state(actor).charged_skill = Some(1);
                    self.ros_stat(actor, 2, 1.5, 0);
                    self.rosmontic_message(actor, "[0][加速]", updates);
                } else {
                    self.ros_message(actor, target, "[0]使用[彗星「炽热飞星」]", updates);
                    let atp = (self.ros_atp(actor, false) * 4.5).round();
                    self.ros_hit(actor, target, false, atp, false, PlainAttackOnDamage::None, updates);
                    let advance = (self.rng.next_u8() & 63) > 31;
                    self.entities.get_mut(actor).unwrap().runtime.move_state.speed_points += if advance { 2048 } else { -2048 };
                    self.rosmontic_message(
                        actor,
                        if advance {
                            "[0]行动提前了"
                        } else {
                            "[0]失误，行动延后了"
                        },
                        updates,
                    );
                }
            }
            2 => {
                self.ros_state(actor).energy = self.ros_state(actor).energy.saturating_sub(1);
                self.ros_message(actor, target, "[0]使用[恋符「非定向光线」]", updates);
                let atp = self.ros_atp(actor, false) * 1.8;
                self.ros_hit(actor, target, true, atp, false, PlainAttackOnDamage::None, updates);
            }
            skill => {
                let energy = self.ros_state(actor).energy;
                self.ros_state(actor).energy = if skill == 4 { 0 } else { energy.saturating_sub(2) };
                self.rosmontic_message(
                    actor,
                    if skill == 4 {
                        "[0]使用[魔炮「究极火花」]"
                    } else {
                        "[0]使用[恋符「极限火花」]"
                    },
                    updates,
                );
                let atp = self.ros_atp(actor, true) * if skill == 4 { 0.7 * f64::from(energy) } else { 1.6 };
                for target in targets {
                    self.ros_hit(actor, target, true, atp, false, PlainAttackOnDamage::None, updates);
                }
            }
        }
    }
}

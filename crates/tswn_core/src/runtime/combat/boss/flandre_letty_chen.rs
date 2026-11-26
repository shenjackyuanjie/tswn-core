//! 芙兰的分身与破坏、蕾蒂的冰冻、橙的化猫连击。
use super::super::*;
impl CombatRuntime {
    pub(super) fn ros_flandre(&mut self, actor: EntityIdx, skill: usize, targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        match skill {
            0 => {
                self.ros_state(actor).levels[0] = 0;
                self.rosmontic_message(actor, "[0]发动[禁忌「四重存在」]", updates);
                let e = self.entities.get(actor).unwrap();
                let r = &e.runtime;
                let attrs = [
                    (f64::from(e.template.max_hp) * 0.4) as i32,
                    (f64::from(r.attack) * 0.6) as i32,
                    (f64::from(r.defense) * 0.6) as i32,
                    (f64::from(r.speed) * 0.6) as i32,
                    (f64::from(r.speed) * 0.6) as i32,
                    (f64::from(r.magic) * 0.6) as i32,
                    (f64::from(r.resistance) * 0.6) as i32,
                    r.wisdom,
                ];
                let hp = (f64::from(r.hp) * 0.4) as i32;
                for _ in 0..3 {
                    let child = self.pcb_spawn(actor, 4, "芙兰朵露的分身", attrs, -1024, 1.0, updates);
                    self.entities.get_mut(child).unwrap().runtime.hp = hp.max(1);
                }
                let e = self.entities.get_mut(actor).unwrap();
                e.template.max_hp = (f64::from(e.template.max_hp) * 0.6) as i32;
                e.runtime.hp = (f64::from(e.runtime.hp) * 0.6) as i32;
            }
            1 => self.pcb_caged_bird(actor, targets[0], true, updates),
            2 => self.pcb_forbidden_game(actor, targets[0], true, updates),
            _ => {
                if self.ros_state(actor).charged_skill.is_none() && self.ros_state(actor).energy == 0 {
                    self.ros_state(actor).charged_skill = Some(3);
                    self.ros_state(actor).energy = 1;
                    self.rosmontic_message(actor, "[0][举剑]", updates);
                    return;
                }
                self.ros_state(actor).energy = 0;
                self.rosmontic_message(actor, "[0]使用[禁忌「莱瓦汀」]", updates);
                let e = self.entities.get(actor).unwrap();
                let factor = 3.9 - f64::from(e.runtime.hp) / f64::from(e.template.max_hp);
                let atp = (factor * self.ros_atp(actor, false)).floor();
                self.ros_hit(
                    actor,
                    targets[0],
                    false,
                    atp,
                    false,
                    PlainAttackOnDamage::RosmonticBurn,
                    updates,
                );
                self.pcb_destroy(actor, targets[0], updates);
            }
        }
    }

    pub(super) fn pcb_caged_bird(&mut self, actor: EntityIdx, target: EntityIdx, destroy: bool, updates: &mut RunUpdates) {
        self.ros_message(actor, target, "[0]使用[禁忌「笼中鸟」]", updates);
        let magic = self.entities.get(actor).unwrap().runtime.magic;
        let resistance = self.entities.get(target).unwrap().runtime.resistance;
        if self.ros_immune(target, "slow") || self.roll_plain_dodge(target, magic, resistance, false) {
            return;
        }
        self.pcb_slow(actor, target, 1, 0, updates);
        let speed = self.entities.get(target).unwrap().runtime.speed;
        if speed > 11 {
            self.ros_stat(target, 2, 1.0, -11);
        }
        let atp = self.ros_atp(actor, false) * 0.55;
        self.ros_hit(actor, target, true, atp, true, PlainAttackOnDamage::None, updates);
        if destroy {
            self.pcb_destroy(actor, target, updates);
        }
    }

    pub(super) fn pcb_forbidden_game(&mut self, actor: EntityIdx, target: EntityIdx, destroy: bool, updates: &mut RunUpdates) {
        self.ros_message(actor, target, "[0]使用[禁忌「被禁止的游戏」]", updates);
        let atp = self.ros_atp(actor, false) * 2.1;
        self.ros_hit(actor, target, false, atp, false, PlainAttackOnDamage::None, updates);
        let e = self.entities.get_mut(actor).unwrap();
        e.runtime.hp = (f64::from(e.runtime.hp) * 0.85).ceil() as i32;
        if destroy {
            self.pcb_destroy(actor, target, updates);
        }
    }

    pub(super) fn ros_letty(&mut self, actor: EntityIdx, skill: usize, targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        let (text, factor, magic_atp, frost) = match skill {
            2 => ("[0]使用[寒符「寒流」]", 0.34, true, self.rng.next_u8() & 1),
            3 => ("[0]使用[寒符「延长的冬日」]", 1.29, false, 13),
            4 => ("[0]使用[冬符「花之凋零」]", 0.78, false, 1),
            _ => ("[0]使用[怪符「桌灵转」]", 0.5, true, 0),
        };
        self.rosmontic_message(actor, text, updates);
        self.ros_state(actor).frost_mode = frost;
        let atp = self.ros_atp(actor, magic_atp) * factor;
        let curse = skill == 5 && self.rng.next_u8() & 1 != 0;
        for target in targets {
            if skill == 5 {
                self.ros_hit(
                    actor,
                    target,
                    true,
                    atp,
                    false,
                    if curse {
                        PlainAttackOnDamage::Curse
                    } else {
                        PlainAttackOnDamage::None
                    },
                    updates,
                );
            } else {
                self.drain_plain_attack_with_atp_and_on_damage_into(
                    actor,
                    target,
                    true,
                    atp,
                    DamageKind::Ice,
                    PlainAttackOnDamage::Ice,
                    updates,
                );
            }
        }
        if skill == 3 {
            self.ros_state(actor).levels[3] = 0;
            self.ros_stat(actor, 3, 0.7, 0);
            let e = self.entities.get_mut(actor).unwrap();
            e.runtime.hp = (f64::from(e.runtime.hp) * 0.7).ceil() as i32;
        } else {
            self.ros_state(actor).frost_mode = 0;
        }
    }

    pub(super) fn ros_chen(&mut self, actor: EntityIdx, skill: usize, targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        match skill {
            1 => {
                self.rosmontic_message(actor, "[0]使用[翔符「飞翔韦驮天」]", updates);
                self.ros_stat(actor, 2, 1.3, 0);
                self.ros_stat(actor, 3, 1.3, 0);
                self.ros_state(actor).levels[1] = (self.ros_state(actor).levels[1] - 10).max(0);
            }
            2 => {
                self.ros_message(actor, targets[0], "[0]使用[仙符「凤凰卵」]", updates);
                let hits = 2 + (self.rng.next_u8() & 3);
                self.pcb_flurry(actor, targets[0], hits, 0.4, 0, true, updates);
            }
            3 => {
                self.rosmontic_message(actor, "[0]使用[童符「护法天童乱舞」]", updates);
                let hits = 1 + (self.rng.next_u8() & 3);
                let factor = f64::from(5 + (self.rng.next_u8() & 3)) / 8.0;
                let atp = self.ros_atp(actor, true) * factor;
                self.ros_state(actor).levels[3] = self.ros_state(actor).levels[3] * 3 / 2;
                for target in targets {
                    for _ in 0..hits {
                        self.ros_hit(actor, target, false, atp, false, PlainAttackOnDamage::None, updates);
                    }
                }
                self.entities.get_mut(actor).unwrap().runtime.move_state.speed_points += 4096;
            }
            _ => {
                self.rosmontic_message(actor, "[0]使用[式符「飞翔晴明」]", updates);
                let atp = self.ros_atp(actor, true) * 0.5;
                for target in targets {
                    self.ros_hit(actor, target, false, atp, false, PlainAttackOnDamage::None, updates);
                }
                self.entities.get_mut(actor).unwrap().runtime.move_state.speed_points += 1024;
            }
        }
    }

    /// 特殊连击逐段做命中判定，首次回避即终止；命中后仍走防御和死亡链。
    pub(super) fn pcb_flurry(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        hits: u8,
        factor: f64,
        decay: i32,
        resistance_check: bool,
        updates: &mut RunUpdates,
    ) {
        let mut accuracy = self.entities.get(actor).unwrap().runtime.agility + 100;
        for _ in 0..hits {
            if !self.entities.get(actor).is_some_and(|e| e.is_active())
                || !self.entities.get(target).is_some_and(|e| e.runtime.hp > 0)
            {
                break;
            }
            let r = &self.entities.get(target).unwrap().runtime;
            let dodge = r.agility + if resistance_check { r.resistance } else { 0 };
            if self.roll_plain_dodge(target, accuracy, dodge, false) {
                break;
            }
            accuracy -= decay;
            let atp = self.ros_atp(actor, false) * factor;
            self.ros_hit(actor, target, true, atp, true, PlainAttackOnDamage::None, updates);
        }
    }
}

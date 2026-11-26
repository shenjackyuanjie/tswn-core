//! 幽灵乐团的声部、妖梦的招架连斩和幽幽子的反魂蝶。
//! 来源：KODU16（`kodu16/main`，`cf460c84`）。
use super::super::*;
impl CombatRuntime {
    pub fn ros_phantom(&mut self, actor: EntityIdx, skill: usize, targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        let texts = [
            "[0]使用[大合葬「灵车大协奏曲」]",
            "[0]使用[神弦「斯特拉迪瓦里」]",
            "[0]使用[冥管「灵之克里福德」]",
            "[0]使用[键灵「贝森朵夫神奏」]",
        ];
        self.rosmontic_message(actor, texts[skill], updates);
        self.ros_state(actor).levels[0] += if skill == 0 { -25 } else { 25 };
        let team = self.plain_effective_team(actor);
        let heal = if skill <= 1 {
            (self.ros_atp(actor, true) / 100.0).ceil() as i32
        } else {
            0
        };
        for target in targets {
            if self.plain_effective_team(target) == team {
                if skill <= 1 {
                    self.apply_plain_absorb_heal_into(actor, target, heal, updates);
                }
                let e = self.entities.get_mut(target).unwrap();
                if skill == 0 || skill == 2 {
                    e.runtime.shield = 80;
                }
                if skill == 0 || skill == 3 {
                    e.runtime.move_state.speed_points += 1024;
                }
            } else {
                if skill == 2 {
                    let atp = self.ros_atp(actor, true) * 0.25;
                    self.ros_hit(actor, target, false, atp, false, PlainAttackOnDamage::Berserk, updates);
                    continue;
                }
                let power = self.entities.get(actor).unwrap().runtime.magic;
                let e = self.entities.get(target).unwrap();
                let dodge = e.runtime.resistance + if skill == 0 { 0 } else { e.runtime.wisdom };
                if self.roll_plain_dodge(target, power, dodge, false) {
                    continue;
                }
                if skill <= 1 {
                    let e = self.entities.get_mut(target).unwrap();
                    e.runtime.hp = ((f64::from(e.runtime.hp) * if skill == 0 { 0.8 } else { 0.85 }) as i32).max(1);
                    self.ros_message(actor, target, "[1]的生命减少了", updates);
                }
                if skill == 0 {
                    let atp = self.ros_atp(actor, true) * 0.25;
                    self.ros_hit(actor, target, false, atp, true, PlainAttackOnDamage::Berserk, updates);
                }
                if skill == 0 || skill == 3 {
                    self.entities.get_mut(target).unwrap().runtime.move_state.speed_points -=
                        if skill == 0 { 1536 } else { 1024 };
                }
            }
        }
    }

    pub fn ros_youmu(&mut self, actor: EntityIdx, skill: usize, targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        if skill == 0 {
            self.rosmontic_message(actor, "[0]：[心定神宁]！", updates);
            self.ros_state(actor).guard = 1;
            self.entities.get_mut(actor).unwrap().runtime.move_state.speed_points -= 2048;
            return;
        }
        let repeats = if self.rng.next_u8() & 3 == 3 { 2 } else { 1 };
        match skill {
            1 => {
                for _ in 0..repeats {
                    self.ros_message(actor, targets[0], "[0]使用[人鬼「未来永劫斩」]", updates);
                    self.pcb_flurry(actor, targets[0], 6, 0.36, 15, false, updates);
                }
            }
            2 => {
                let atp = self.ros_atp(actor, false) * 1.1;
                let effect = if self.rng.next_u8() & 1 == 0 {
                    PlainAttackOnDamage::None
                } else {
                    PlainAttackOnDamage::Disperse
                };
                for _ in 0..repeats {
                    self.ros_message(actor, targets[0], "[0]使用[断迷剑「迷津慈航斩」]", updates);
                    self.ros_hit(actor, targets[0], false, atp, true, effect, updates);
                }
            }
            _ => {
                self.ros_state(actor).levels[3] /= 2;
                for _ in 0..repeats {
                    self.rosmontic_message(actor, "[0]使用[剑伎「樱花闪闪」]", updates);
                    let atp = self.ros_atp(actor, true) * 1.2;
                    for &target in &targets {
                        let faster =
                            self.entities.get(actor).unwrap().runtime.speed > self.entities.get(target).unwrap().runtime.speed;
                        self.ros_hit(
                            actor,
                            target,
                            true,
                            atp * if faster { 1.5 } else { 1.0 },
                            faster,
                            PlainAttackOnDamage::None,
                            updates,
                        );
                    }
                }
            }
        }
    }

    pub fn ros_yuyuko(&mut self, actor: EntityIdx, skill: usize, targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        match skill {
            0 => {
                if self.ros_state(actor).energy == 0 {
                    let e = self.entities.get(actor).unwrap();
                    let low = e.runtime.hp * 2 < e.template.max_hp;
                    let s = self.ros_state(actor);
                    s.energy = 1;
                    s.charged_skill = Some(0);
                    s.break_on_damage = !low;
                    if low {
                        self.entities.get_mut(actor).unwrap().runtime.move_state.speed_points += 1024;
                    }
                    self.rosmontic_message(actor, "[0]：[飞舞，摇坠，抑或如此凋亡？]", updates);
                } else {
                    let s = self.ros_state(actor);
                    s.energy = 0;
                    s.break_on_damage = false;
                    self.rosmontic_message(actor, "[0]使用[死蝶「华胥的永眠」]", updates);
                    let e = self.entities.get(actor).unwrap();
                    let factor = if e.runtime.hp * 2 < e.template.max_hp { 1.08 } else { 0.72 };
                    let atp = self.ros_atp(actor, true) * factor;
                    for target in targets.into_iter().take(3) {
                        for _ in 0..4 {
                            self.ros_hit(actor, target, true, atp, false, PlainAttackOnDamage::None, updates);
                        }
                    }
                }
            }
            1 => {
                self.ros_state(actor).levels[1] = 0;
                self.ros_state(actor).butterfly_prepared = true;
                self.rosmontic_message(actor, "[0]使用[幽曲「埋骨于弘川」]", updates);
            }
            2 => {
                self.rosmontic_message(actor, "[0]使用[樱花「樱吹雪花小町」]", updates);
                let atp = self.ros_atp(actor, true) * 0.18;
                for target in targets {
                    for _ in 0..4 {
                        self.ros_hit(actor, target, true, atp, false, PlainAttackOnDamage::None, updates);
                    }
                }
            }
            _ => {
                let target = targets[0];
                self.ros_message(actor, target, "[0]使用[幽雅「通向黄泉的诱蛾灯」]", updates);
                let e = self.entities.get(target).unwrap();
                let layers = if e.states.entry(78).is_some() { 3 } else { 1 };
                let delay = e.runtime.speed + 128;
                self.pcb_slow(actor, target, layers, delay, updates);
            }
        }
    }
}

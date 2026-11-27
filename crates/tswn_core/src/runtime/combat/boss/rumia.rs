//! 露米娅：黑雾反击、两次月影增防与濒死毁灭模式。
//! 来源：KODU16（`kodu16/main`，`cf460c84`）。
use super::super::*;
impl CombatRuntime {
    pub fn reflect_rumia_into(&mut self, target: EntityIdx, caster: EntityIdx, updates: &mut RunUpdates) {
        if !self.is_rosmontic_boss(target, "rumia") || !self.entities.get(caster).is_some_and(|e| e.is_active()) {
            return;
        }
        let r = &self.entities.get(target).unwrap().runtime;
        if r.hp <= 0 {
            return;
        }
        let atp = 16.0 * f64::from(r.defense + r.resistance);
        self.rosmontic_message(target, "[0]触发[黑雾]", updates);
        self.drain_plain_attack_after_dodge_into(
            target,
            caster,
            false,
            atp,
            DamageKind::Default,
            None,
            PlainAttackOnDamage::None,
            updates,
        );
    }

    /// 毁灭模式只在存活且 HP 不超过 100 时触发一次，并立即推进一个行动周期。
    pub fn upgrade_rumia_into(&mut self, target: EntityIdx, updates: &mut RunUpdates) {
        if !self.is_rosmontic_boss(target, "rumia") {
            return;
        }
        let r = &mut self.entities.get_mut(target).unwrap().runtime;
        if r.hp <= 0 || r.hp > 100 || r.boss.as_deref().and_then(|b| b.rosmontic.as_ref()).unwrap().rumia_destroy {
            return;
        }
        let s = r.boss_mut().rosmontic.as_mut().unwrap();
        s.rumia_destroy = true;
        s.rumia_def_bonus *= 5;
        r.attack *= 5;
        r.defense *= 5;
        r.magic *= 5;
        r.resistance *= 5;
        r.wisdom *= 5;
        r.move_state.speed_points += 4096;
        self.rosmontic_message(target, "[0]的[封印]解除了", updates);
        self.rosmontic_message(target, "[0]：[!毁灭模式!]，启动", updates);
    }

    pub fn drain_rumia_action_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let cost = i32::from(self.rng.next_u8() & 15) + 8;
        let can_cast = self.entities.get(actor).unwrap().runtime.magic_point >= cost;
        let moons = self
            .entities
            .get(actor)
            .unwrap()
            .runtime
            .boss
            .as_deref()
            .and_then(|b| b.rosmontic.as_ref())
            .unwrap()
            .rumia_moons;
        let mut selected = 0;
        if can_cast {
            for (skill, level) in [(2, 85), (1, 45), (3, 200i32 - (moons as i32) * 100)] {
                if level > 0 && (self.rng.r127() as i32) < level {
                    selected = skill;
                    break;
                }
            }
        }
        let Some(target) = self.select_plain_default_attack_target(actor, true) else {
            return;
        };
        if selected == 0 {
            self.drain_plain_default_attack_into(actor, target, false, updates);
            return;
        }
        self.entities.get_mut(actor).unwrap().runtime.magic_point -= cost;
        let physical = selected != 1;
        let atp = self.entities.get(actor).unwrap().runtime.get_at(!physical, &mut self.rng);
        match selected {
            1 => {
                self.rosmontic_message(actor, "[0]使用[暗符「境界线」]", updates);
                let team = self.plain_effective_team(actor);
                let mut targets: Vec<_> = self
                    .world
                    .flat_alive()
                    .iter()
                    .copied()
                    .filter(|id| self.plain_effective_team(*id) != team && !self.quilt_untargetable(*id))
                    .collect();
                self.rng.sort_list(&mut targets);
                let count = 2 + self.rng.next_u8() as usize % 3;
                targets.truncate(count);
                for target in targets {
                    self.drain_plain_attack_with_atp_and_on_damage_into(
                        actor,
                        target,
                        true,
                        atp * 0.9,
                        DamageKind::Default,
                        PlainAttackOnDamage::None,
                        updates,
                    );
                }
            }
            2 => {
                self.rosmontic_message(actor, "[0]使用[夜符「夜盲症」]", updates);
                self.drain_plain_attack_with_atp_and_on_damage_into(
                    actor,
                    target,
                    false,
                    atp * 1.2,
                    DamageKind::Default,
                    PlainAttackOnDamage::Berserk,
                    updates,
                );
            }
            _ => {
                self.rosmontic_message(actor, "[0]使用[暗符「月的阴暗面」]", updates);
                self.entities
                    .get_mut(actor)
                    .unwrap()
                    .runtime
                    .boss_mut()
                    .rosmontic
                    .as_mut()
                    .unwrap()
                    .rumia_moons += 1;
                self.drain_plain_attack_with_atp_and_on_damage_into(
                    actor,
                    target,
                    true,
                    atp,
                    DamageKind::Default,
                    PlainAttackOnDamage::None,
                    updates,
                );
                let r = &mut self.entities.get_mut(actor).unwrap().runtime;
                r.boss_mut().rosmontic.as_mut().unwrap().rumia_def_bonus += 50;
                r.defense += 50;
                r.resistance += 50;
            }
        }
    }
}

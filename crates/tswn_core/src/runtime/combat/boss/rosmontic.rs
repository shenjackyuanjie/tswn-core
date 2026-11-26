//! 冴月麟的八段台词与 megia 的两阶段剑技。
use super::super::*;

impl CombatRuntime {
    pub(crate) fn is_rosmontic_boss(&self, actor: EntityIdx, name: &str) -> bool {
        self.entities
            .get(actor)
            .is_some_and(|e| e.runtime.flags.contains(PlayerKindFlags::BOSS) && e.template.name == name)
    }

    pub(crate) fn rosmontic_message(&self, actor: EntityIdx, text: &'static str, updates: &mut RunUpdates) {
        updates.add(RunUpdate::new(text, actor.0 as usize, actor.0 as usize, 1).without_replay_target());
    }

    /// 冴月麟不攻击；第八次行动按自我退场流程结算。
    pub fn drain_rosmontic_action_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if self.is_rosmontic_boss(actor, "rin") {
            let entity = self.entities.get_mut(actor).unwrap();
            let state = entity.runtime.rosmontic.as_mut().unwrap();
            state.actions += 1;
            let turn = state.actions;
            let text = match turn {
                2 => "[0]: 世界  一台机器  正暗响孤鸣  多少个世纪",
                4 => "[0]: 岛屿  罗盘样的形体  在大洋穹上  在星辰海底",
                6 => "[0]: 壳里  黝锈的齿轮  和着浪语  明晰的困噫",
                8 => "[0]: 等等  它沉没或浮起  怎不到达  梦的沙堤",
                _ => "[0]: ......",
            };
            self.rosmontic_message(actor, text, updates);
            if turn == 8 {
                self.entities.get_mut(actor).unwrap().runtime.hp = 0;
                self.drain_summon_explode_self_death_into(actor, updates);
            }
            return;
        }
        if self.is_rosmontic_boss(actor, "rumia") {
            self.drain_rumia_action_into(actor, updates);
            return;
        }
        if self.is_rosmontic_boss(actor, "megia") {
            self.drain_megia_action_into(actor, updates);
        } else {
            self.drain_touhou_action_into(actor, updates);
        }
    }

    /// 蓄力后下一次行动锁定原目标；斩击从三次威力抽样中取最大值。
    fn drain_megia_action_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let previous = self
            .entities
            .get_mut(actor)
            .unwrap()
            .runtime
            .rosmontic
            .as_mut()
            .unwrap()
            .charge_target
            .take();
        if let Some(target) = previous {
            if !self.entities.get(target).is_some_and(|e| e.is_active()) {
                return;
            }
            let runtime = &self.entities.get(actor).unwrap().runtime;
            let low = runtime.hp <= 199;
            let atp = (0..3).map(|_| runtime.get_at(true, &mut self.rng)).fold(0.0, f64::max);
            if low {
                self.rosmontic_message(actor, "[0]:[我将在此成为英雄！]", updates);
            }
            self.rosmontic_message(
                actor,
                if low {
                    "[0]发动[肃清的一刀]"
                } else {
                    "[0]发动[真实的斩击]"
                },
                updates,
            );
            self.drain_plain_attack_after_dodge_into(
                actor,
                target,
                true,
                atp * if low { 12.0 } else { 6.0 },
                DamageKind::Default,
                None,
                PlainAttackOnDamage::None,
                updates,
            );
            return;
        }
        let Some(target) = self.select_plain_default_attack_target(actor, true) else {
            return;
        };
        let entity = self.entities.get_mut(actor).unwrap();
        let cost = i32::from(self.rng.next_u8() & 15) + 8;
        if entity.runtime.magic_point < cost {
            self.drain_plain_default_attack_into(actor, target, false, updates);
            return;
        }
        entity.runtime.magic_point -= cost;
        let state = entity.runtime.rosmontic.as_mut().unwrap();
        state.charge_target = Some(target);
        state.break_on_damage = entity.runtime.hp > 199;
        entity.runtime.move_state.speed_points += entity.runtime.magic * 3;
        if entity.runtime.hp <= 199 {
            entity.runtime.move_state.speed_points += 1600;
        }
        self.rosmontic_message(actor, "[0]发动[虚伪的剑闪]", updates);
    }

    /// 蓄力开始时挂了中断钩子，就算受击后进入低体力阶段也仍会被打断。
    pub(crate) fn interrupt_megia_into(&mut self, target: EntityIdx, updates: &mut RunUpdates) {
        if !self.is_rosmontic_boss(target, "megia") {
            return;
        }
        let entity = self.entities.get_mut(target).unwrap();
        let state = entity.runtime.rosmontic.as_mut().unwrap();
        if state.break_on_damage && state.charge_target.take().is_some() {
            self.rosmontic_message(target, "[0]的剑技被打断了", updates);
        }
    }

    pub(crate) fn revive_megia_into(&mut self, target: EntityIdx, updates: &mut RunUpdates) -> bool {
        if !self.is_rosmontic_boss(target, "megia") {
            return false;
        }
        let entity = self.entities.get_mut(target).unwrap();
        let state = entity.runtime.rosmontic.as_mut().unwrap();
        if entity.runtime.hp > 0 || state.resolve_used {
            return false;
        }
        state.resolve_used = true;
        entity.runtime.hp = 199;
        self.rosmontic_message(target, "[0]触发[无名的决意]", updates);
        true
    }
}

//! 沙冰鱼（Shabby_fish）Boss：净化、魔高压制与全体普攻。
use super::*;
use crate::runtime::entity::FishRuntime;

#[cfg(test)]
#[path = "boss_fish_tests.rs"]
mod tests;

/// Boss 名字。
pub const ID: &str = "Shabby_fish";

impl CombatRuntime {
    fn fish_enemies(&self, actor: EntityIdx) -> Vec<EntityIdx> {
        let team = self.plain_effective_team(actor);
        self.world
            .flat_alive()
            .iter()
            .copied()
            .filter(|idx| self.plain_effective_team(*idx) != team)
            .collect()
    }

    pub(super) fn drain_fish_action_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if !self.entities.get(actor).is_some_and(|e| e.runtime.active()) {
            return;
        }
        self.begin_fish_turn(actor, updates);
        let turn = {
            let Some(state) = self.entities.get_mut(actor).and_then(|e| e.runtime.fish.as_mut()) else {
                return;
            };
            state.turn = state.turn.saturating_add(1);
            state.turn
        };
        if turn == 1 {
            // 净化熟练度 200。
            let purify = self
                .registry
                .skill_id_by_export_name("core.skill.disperse")
                .or_else(|| self.registry.skill_id_by_export_name("core.skill.purify"));
            if let Some(purify) = purify {
                if let Some(entity) = self.entities.get_mut(actor) {
                    entity.template.skills = crate::runtime::entity::SkillLoadout::from_skill_levels([(purify, 200)]);
                }
            }
        }
        if turn % 2 == 1 {
            self.fish_magic_zero(actor, updates);
        } else {
            self.fish_attack_all(actor, updates);
        }
    }

    fn fish_say(&self, actor: EntityIdx, message: &'static str, updates: &mut RunUpdates) {
        updates.add_newline();
        updates.add(RunUpdate::new(message, actor.0 as usize, actor.0 as usize, 1).without_replay_target());
        updates.add_newline();
    }

    /// 魔高：随机两名敌方角色魔力归零 2 回合。
    fn fish_magic_zero(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let mut enemies = self.fish_enemies(actor);
        if enemies.is_empty() {
            return;
        }
        self.fish_say(actor, "[0]使用[魔高]", updates);
        let first = usize::from(self.rng.next_u8()) % enemies.len();
        let target_a = enemies.remove(first);
        let target_b = if enemies.is_empty() {
            None
        } else {
            let index = usize::from(self.rng.next_u8()) % enemies.len();
            Some(enemies[index])
        };
        for target in std::iter::once(target_a).chain(target_b) {
            let magic = self.entities.get(target).map_or(0, |e| e.runtime.magic);
            if let Some(entity) = self.entities.get_mut(target) {
                entity.runtime.magic = 0;
            }
            if let Some(state) = self.entities.get_mut(actor).and_then(|e| e.runtime.fish.as_mut()) {
                let turn = state.turn;
                state.magic_zero.push((target, turn + 2, magic));
            }
            updates.add(RunUpdate::new("[1]的魔力归零2回合", actor.0 as usize, target.0 as usize, 0));
        }
    }

    /// 本来大家各种类型打成一片，沙冰鱼直接吊打：对全体敌人各进行一次普攻。
    fn fish_attack_all(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let enemies = self.fish_enemies(actor);
        if enemies.is_empty() {
            return;
        }
        self.fish_say(actor, "[0]使用[本来大家各种类型打成一片，沙冰鱼直接吊打]", updates);
        for target in enemies {
            if !self.entities.get(target).is_some_and(|e| e.runtime.active()) {
                continue;
            }
            let atp = self.entities.get(actor).map_or(0.0, |e| f64::from(e.runtime.attack));
            self.drain_plain_attack_with_atp_into(actor, target, false, atp, updates);
        }
    }

    /// 回合开始恢复到期的魔力压制。
    pub(super) fn begin_fish_turn(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let mut expired: smallvec::SmallVec<[(EntityIdx, i32); 4]> = smallvec::SmallVec::new();
        {
            let Some(state) = self.entities.get_mut(actor).and_then(|e| e.runtime.fish.as_mut()) else {
                return;
            };
            let turn = state.turn;
            state.magic_zero.retain(|(target, until, magic)| {
                if turn >= *until {
                    expired.push((*target, *magic));
                    false
                } else {
                    true
                }
            });
        }
        for (target, magic) in expired {
            if let Some(entity) = self.entities.get_mut(target) {
                entity.runtime.magic = magic;
            }
            updates.add(RunUpdate::new("[1]的魔力恢复了", actor.0 as usize, target.0 as usize, 0));
        }
    }
}

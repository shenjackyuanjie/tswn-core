//! 咦 Boss 与衍生物拜谢：闪避反击、猴咦射日与 MP 压制。
use super::*;
use crate::runtime::entity::{BaixieRuntime, YiRuntime};

#[cfg(test)]
#[path = "boss_yi_tests.rs"]
mod tests;

/// Boss 名字。
pub const ID: &str = "咦";
/// 衍生物名字。
pub const BAIXIE: &str = "拜谢";

impl CombatRuntime {
    fn yi_enemies(&self, actor: EntityIdx) -> Vec<EntityIdx> {
        let team = self.plain_effective_team(actor);
        self.world
            .flat_alive()
            .iter()
            .copied()
            .filter(|idx| self.plain_effective_team(*idx) != team)
            .collect()
    }

    fn yi_say(&self, actor: EntityIdx, message: &'static str, updates: &mut RunUpdates) {
        updates.add_newline();
        updates.add(RunUpdate::new(message, actor.0 as usize, actor.0 as usize, 1).without_replay_target());
        updates.add_newline();
    }

    pub(super) fn drain_yi_action_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if !self.entities.get(actor).is_some_and(|e| e.runtime.active()) {
            return;
        }
        let turn = {
            let Some(state) = self.entities.get_mut(actor).and_then(|e| e.runtime.yi.as_mut()) else {
                return;
            };
            state.turn = state.turn.saturating_add(1);
            state.turn
        };
        if turn == 1 {
            self.yi_say(actor, "[0]使用[咦拜涂地]", updates);
            self.yi_spawn_baixie(actor, updates);
            self.yi_spawn_baixie(actor, updates);
            return;
        }
        // 场上拜谢最多保留两个；不足两个且掷中时才再召唤，其余回合改为猴咦射日。
        let alive_baixie = self
            .entities
            .iter()
            .filter(|(_, entity)| {
                entity.runtime.alive
                    && entity.template.name == BAIXIE
                    && entity.runtime.owner == actor
                    && entity.runtime.is_minion()
            })
            .count();
        if alive_baixie < 2 && self.rng.next_u8() % 3 == 0 {
            self.yi_say(actor, "[0]使用[咦拜涂地]", updates);
            self.yi_spawn_baixie(actor, updates);
        } else {
            self.yi_sun(actor, updates);
        }
    }

    fn yi_spawn_baixie(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(owner) = self.entities.get(actor) else {
            return;
        };
        let team = owner.runtime.team;
        let kind = self
            .registry
            .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
            .expect("core summon kind missing");
        let mut template = PlayerTemplate::with_kind(0, BAIXIE, kind, team, 356, 76);
        template.speed = 68;
        template.agility = 69;
        template.defense = 99;
        template.magic = 78;
        template.resistance = 98;
        template.wisdom = 56;
        template.magic_point = 40;
        template.id_key_name = "yi-baixie".to_owned();
        template.display_name = BAIXIE.to_owned();
        let idx = self.entities.next_spawn_idx(&template);
        self.effects.push(QueuedEffect::SpawnWithMessage {
            caster: actor,
            template,
            message: format!("{BAIXIE}从虚空中出现了"),
        });
        self.drain_effects_into(updates);
        if let Some(spawned) = self.entities.get_mut(idx) {
            spawned.template.id_key_name = "yi-baixie".to_owned();
            spawned.runtime.baixie = Some(Box::new(BaixieRuntime::default()));
        }
    }

    /// 猴咦射日：一次 150% 魔攻，之后攻击力永久 +50%。
    fn yi_sun(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(target) = self.select_plain_default_attack_target(actor, true) else {
            return;
        };
        self.yi_say(actor, "[0]使用[猴咦射日]", updates);
        let atp = self.entities.get(actor).map_or(0.0, |entity| f64::from(entity.runtime.magic) * 1.5);
        self.drain_plain_attack_with_atp_into(actor, target, true, atp, updates);
        if let Some(entity) = self.entities.get_mut(actor) {
            entity.runtime.attack = entity.runtime.attack * 3 / 2;
            entity.template.attack = entity.template.attack * 3 / 2;
        }
        updates.add(RunUpdate::new("[0]攻击力永久上升50%", actor.0 as usize, actor.0 as usize, 0));
    }

    /// 咦？：成功闪避后对 1~x-2 名敌人反击。
    pub(super) fn yi_dodge_counter(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let enemies = self.yi_enemies(actor);
        let x = enemies.len();
        if x <= 2 {
            return;
        }
        let count = 1 + usize::from(self.rng.next_u8()) % (x - 2);
        self.yi_say(actor, "[0]说了一声[咦]", updates);
        for _ in 0..count {
            let enemies = self.yi_enemies(actor);
            if enemies.is_empty() {
                break;
            }
            let target = enemies[usize::from(self.rng.next_u8()) % enemies.len()];
            let atp = self.entities.get(actor).map_or(0.0, |entity| f64::from(entity.runtime.magic) * 0.6);
            self.drain_plain_attack_with_atp_into(actor, target, true, atp, updates);
        }
    }

    /// 拜谢的行动：随机对 1~2 名未拜谢过的敌人扣 20 MP 并标记；全部拜过后为咦回血。
    pub(super) fn drain_baixie_action_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if !self.entities.get(actor).is_some_and(|e| e.runtime.active()) {
            return;
        }
        let owner = self.entities.get(actor).map(|e| e.runtime.owner);
        let enemies = self.yi_enemies(actor);
        let mut candidates: Vec<EntityIdx> = enemies
            .into_iter()
            .filter(|target| {
                !self
                    .entities
                    .get(actor)
                    .and_then(|e| e.runtime.baixie.as_ref())
                    .is_some_and(|state| state.thanked.contains(target))
            })
            .collect();
        if candidates.is_empty() {
            // 全场都拜过：改为给咦回血。
            if let Some(owner) = owner {
                let heal = self.entities.get(owner).map_or(0, |e| (e.template.max_hp / 10).max(1));
                if let Some(entity) = self.entities.get_mut(owner) {
                    entity.runtime.hp = (entity.runtime.hp + heal).min(entity.template.max_hp);
                }
                updates.add(RunUpdate::new("[0]为[1]回复体力", actor.0 as usize, owner.0 as usize, 0));
            }
            return;
        }
        let count = 1 + usize::from(self.rng.next_u8()) % 2;
        self.yi_say(actor, "[0]使用[！？拜拜？！]", updates);
        for _ in 0..count {
            if candidates.is_empty() {
                break;
            }
            let index = usize::from(self.rng.next_u8()) % candidates.len();
            let target = candidates.remove(index);
            if let Some(entity) = self.entities.get_mut(target) {
                entity.runtime.magic_point = (entity.runtime.magic_point - 20).max(0);
            }
            if let Some(state) = self.entities.get_mut(actor).and_then(|e| e.runtime.baixie.as_mut()) {
                state.thanked.push(target);
            }
            updates.add(RunUpdate::new(
                "[1]被[0]拜谢，失去20点MP",
                actor.0 as usize,
                target.0 as usize,
                0,
            ));
        }
    }

    /// 目标当前是否被存活的拜谢压低了 MP 恢复。
    pub(super) fn baixie_mp_scale(&self, target: EntityIdx) -> f64 {
        let mut thanked = false;
        for (_, entity) in self.entities.iter() {
            if !entity.runtime.alive {
                continue;
            }
            if let Some(state) = entity.runtime.baixie.as_ref()
                && state.thanked.contains(&target)
            {
                thanked = true;
                break;
            }
        }
        if thanked { 0.5 } else { 1.0 }
    }
}

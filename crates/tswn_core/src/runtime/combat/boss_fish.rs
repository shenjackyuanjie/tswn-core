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

    /// 施了遗忘咒：随机敌方最高熟练度技能归零。
    fn fish_forget(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let enemies = self.fish_enemies(actor);
        if enemies.is_empty() {
            return;
        }
        let target = enemies[usize::from(self.rng.next_u8()) % enemies.len()];
        self.fish_say(actor, "[0]使用[并对你施了遗忘咒]", updates);
        let mut cleared = String::from("最高");
        if let Some(entity) = self.entities.get_mut(target) {
            let best = (0..40)
                .filter_map(|lane| entity.template.skills.level_at(lane).map(|level| (lane, level)))
                .max_by_key(|(_, level)| *level);
            if let Some((lane, _)) = best {
                if let Some(spec) = entity.template.skills.skills().get(lane).and_then(|id| self.registry.skill(*id)) {
                    cleared = spec.name.clone();
                }
                entity.template.skills.set_level_at(lane, 0);
            }
        }
        updates.add(RunUpdate::new(
            format!("[1]的{cleared}熟练度归零"),
            actor.0 as usize,
            target.0 as usize,
            0,
        ));
    }

    /// 猴儿：召唤一只猴区（与被子相同）。
    fn fish_monkey(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        self.fish_say(actor, "[0]使用[猴儿]", updates);
        self.quilt_spawn(actor, crate::runtime::combat::boss_quilt::Child::Monkey, updates);
    }

    /// 所以说我现在要发11.7T帮测：一次不可反弹的普攻，未回避则直接击杀，伤害外显 11.7T。
    fn fish_eleven_tera(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(target) = self.select_plain_default_attack_target(actor, true) else {
            return;
        };
        self.fish_say(actor, "[0]使用[所以说我现在要发11.7T帮测]", updates);
        let accuracy = self.entities.get(actor).map_or(0, |e| e.runtime.attack);
        let dodge = self.entities.get(target).map_or(0, |e| e.runtime.agility);
        if target != actor && self.roll_plain_dodge(target, accuracy, dodge, false) {
            updates.add(RunUpdate::new("[1][回避]了攻击", actor.0 as usize, target.0 as usize, 0));
            return;
        }
        // 11.7 万亿（117 后面跟 11 个 0）；超出 u32，直接写进文本。
        updates.add(RunUpdate::new(
            "[1]受到11700000000000点伤害[s_dmg160]",
            actor.0 as usize,
            target.0 as usize,
            0,
        ));
        if let Some(entity) = self.entities.get_mut(target) {
            entity.runtime.hp = 0;
            entity.runtime.alive = false;
            let team = entity.runtime.team;
            self.mark_dead_with_linked_minions_into(target, team, updates);
        }
    }

    /// 晋级锦标赛的，记得换号：把随机敌人的名字换成一串随机字符，
    /// 并按照这个新名字重建它的属性、图标与技能熟练度。
    fn fish_rename(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let enemies = self.fish_enemies(actor);
        if enemies.is_empty() {
            return;
        }
        let target = enemies[usize::from(self.rng.next_u8()) % enemies.len()];
        let mut new_name = String::new();
        const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz0123456789";
        for _ in 0..8 {
            let index = usize::from(self.rng.next_u8()) % CHARS.len();
            new_name.push(CHARS[index] as char);
        }
        self.fish_say(actor, "[0]使用[晋级锦标赛的，记得换号]", updates);
        self.rebuild_player_from_name(target, &new_name);
        updates.add(RunUpdate::new(
            format!("[1]被更换为{new_name}"),
            actor.0 as usize,
            target.0 as usize,
            0,
        ));
    }

    /// 按名字重建一个战斗单位的模板与运行态（属性、图标、技能熟练度全部换新）。
    fn rebuild_player_from_name(&mut self, target: EntityIdx, name: &str) -> bool {
        let Some(old) = self.entities.get(target) else {
            return false;
        };
        let team = old.runtime.team;
        let id = old.template.id;
        let Some(mut template) = crate::runtime::prepared_init::template_from_name(&self.registry, name, team) else {
            return false;
        };
        template.id = id;
        template.team = team;
        if let Some(entity) = self.entities.get_mut(target) {
            entity.template = template;
            entity.refresh_runtime_stats_from_template();
            entity.runtime.hp = entity.template.max_hp;
            entity.runtime.alive = true;
        }
        true
    }
    /// 哎呀舒服呀：回复 25% 体力，全属性 +10%。
    fn fish_comfort(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        self.fish_say(actor, "[0]使用[哎呀舒服呀]", updates);
        if let Some(entity) = self.entities.get_mut(actor) {
            let heal = (entity.template.max_hp / 4).max(1);
            let before = entity.runtime.hp;
            entity.runtime.hp = (before + heal).min(entity.template.max_hp);
            let healed = entity.runtime.hp - before;
            for (runtime, template) in [
                (&mut entity.runtime.attack, &mut entity.template.attack),
                (&mut entity.runtime.defense, &mut entity.template.defense),
                (&mut entity.runtime.speed, &mut entity.template.speed),
                (&mut entity.runtime.agility, &mut entity.template.agility),
                (&mut entity.runtime.magic, &mut entity.template.magic),
                (&mut entity.runtime.resistance, &mut entity.template.resistance),
                (&mut entity.runtime.wisdom, &mut entity.template.wisdom),
            ] {
                *runtime = *runtime * 11 / 10;
                *template = *template * 11 / 10;
            }
            if healed > 0 {
                let mut line = RunUpdate::new("[0]回复体力[1]点", actor.0 as usize, actor.0 as usize, 0);
                line.param = Some(healed as u32);
                updates.add(line);
            }
        }
        updates.add(RunUpdate::new("[0]全属性上升10%", actor.0 as usize, actor.0 as usize, 0));
    }

    /// 令人咦：与猴咦射日同效果，只是名字不同。
    fn fish_lingrenyi(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(target) = self.select_plain_default_attack_target(actor, true) else {
            return;
        };
        self.fish_say(actor, "[0]使用[令人咦]", updates);
        let atp = self.entities.get(actor).map_or(0.0, |e| f64::from(e.runtime.magic) * 1.5);
        self.drain_plain_attack_with_atp_into(actor, target, true, atp, updates);
        if let Some(entity) = self.entities.get_mut(actor) {
            entity.runtime.attack = entity.runtime.attack * 3 / 2;
            entity.template.attack = entity.template.attack * 3 / 2;
        }
        updates.add(RunUpdate::new("[0]攻击力永久上升50%", actor.0 as usize, actor.0 as usize, 0));
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
        self.fish_say(actor, "[0]使用[本体本体也不需要魔高也不需要魔高]", updates);
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

            self.drain_plain_default_attack_into(actor, target, false, updates);
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

//! TigerStar（豹星）Boss：魅惑、我是M、猎豹自爆、红温与幻术。
use super::*;
use crate::runtime::entity::TigerRuntime;

#[cfg(test)]
#[path = "boss_tiger_tests.rs"]
mod tests;

/// Boss 名字。
pub const ID: &str = "TigerStar";
/// 猎豹显示名。
pub const LEOPARD: &str = "猎豹";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TigerSkill {
    Grass,
    Masochist,
    Everyone,
    ThanksLeopard,
    Boom,
    Red,
    Shadow,
    Small,
    Leopards,
    Buff,
}

impl CombatRuntime {
    fn tiger_enemies(&self, actor: EntityIdx) -> Vec<EntityIdx> {
        let team = self.plain_effective_team(actor);
        self.world
            .flat_alive()
            .iter()
            .copied()
            .filter(|idx| self.plain_effective_team(*idx) != team)
            .collect()
    }

    fn tiger_leopards(&self, actor: EntityIdx) -> Vec<EntityIdx> {
        let team = self.plain_effective_team(actor);
        self.world
            .flat_alive()
            .iter()
            .copied()
            .filter(|idx| {
                *idx != actor
                    && self.plain_effective_team(*idx) == team
                    && self.entities.get(*idx).is_some_and(|e| e.template.name == LEOPARD)
            })
            .collect()
    }

    fn next_tiger_skill(&self, actor: EntityIdx) -> TigerSkill {
        let Some(state) = self.entities.get(actor).and_then(|e| e.runtime.tiger.as_ref()) else {
            return TigerSkill::Buff;
        };
        if !state.red && self.entities.get(actor).is_some_and(|e| e.runtime.hp * 2 < e.template.max_hp) {
            return TigerSkill::Red;
        }
        match state.turn % 10 {
            1 => TigerSkill::Grass,
            2 => TigerSkill::Masochist,
            3 => TigerSkill::Everyone,
            4 => TigerSkill::ThanksLeopard,
            5 => {
                if self.tiger_leopards(actor).is_empty() {
                    TigerSkill::Leopards
                } else {
                    TigerSkill::Boom
                }
            }
            6 => TigerSkill::Shadow,
            7 => TigerSkill::Small,
            8 => TigerSkill::Leopards,
            // 「我知道微弱的优势…」发动概率提升到 2/10（第 9、10 回合）。
            0 | 9 => TigerSkill::Buff,
            _ => TigerSkill::Grass,
        }
    }

    pub(super) fn drain_tiger_action_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if !self.entities.get(actor).is_some_and(|e| e.runtime.active()) {
            return;
        }
        {
            let Some(state) = self.entities.get_mut(actor).and_then(|e| e.runtime.tiger.as_mut()) else {
                return;
            };
            state.turn = state.turn.saturating_add(1);
            if state.m_turns > 0 {
                state.m_turns -= 1;
            }
        }
        match self.next_tiger_skill(actor) {
            TigerSkill::Grass => self.tiger_grass(actor, updates),
            TigerSkill::Masochist => self.tiger_masochist(actor, updates),
            TigerSkill::Everyone => self.tiger_everyone(actor, updates),
            TigerSkill::ThanksLeopard => self.tiger_thanks_leopard(actor, updates),
            TigerSkill::Boom => self.tiger_boom(actor, updates),
            TigerSkill::Red => self.tiger_red(actor, updates),
            TigerSkill::Shadow => self.tiger_shadow(actor, updates),
            TigerSkill::Small => self.tiger_small(actor, updates),
            TigerSkill::Leopards => self.tiger_leopards_skill(actor, updates),
            TigerSkill::Buff => self.tiger_buff(actor, updates),
        }
    }

    fn tiger_say(&self, actor: EntityIdx, message: &'static str, updates: &mut RunUpdates) {
        updates.add_newline();
        updates.add(RunUpdate::new(message, actor.0 as usize, actor.0 as usize, 1).without_replay_target());
        updates.add_newline();
    }

    fn tiger_pick_enemy(&mut self, actor: EntityIdx) -> Option<EntityIdx> {
        let enemies = self.tiger_enemies(actor);
        if enemies.is_empty() {
            return None;
        }
        let index = usize::from(self.rng.next_u8()) % enemies.len();
        Some(enemies[index])
    }

    /// 草你：无法回避的 100% 魔攻 + 扣 64 MP + 两回合魅惑。
    fn tiger_grass(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(target) = self.tiger_pick_enemy(actor) else {
            return;
        };
        self.tiger_say(actor, "[0]使用[草你]", updates);
        let amount = self.entities.get(actor).map_or(0.0, |e| f64::from(e.runtime.magic));
        if self.apply_plain_attack_damage_into(actor, target, amount as i32, updates) {
            self.drain_plain_lethal_damage_into(actor, target, updates);
        }
        if self.entities.get(target).is_some_and(|e| e.runtime.active()) {
            if let Some(entity) = self.entities.get_mut(target) {
                entity.runtime.magic_point = (entity.runtime.magic_point - 64).max(0);
            }
            updates.add(RunUpdate::new("[1]失去64点MP", actor.0 as usize, target.0 as usize, 0));
            self.drain_plain_charm_skill_into(actor, target, updates);
        }
    }

    /// 我是M：两回合内受伤时随机属性提升等量数值。
    fn tiger_masochist(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if let Some(state) = self.entities.get_mut(actor).and_then(|e| e.runtime.tiger.as_mut()) {
            state.m_turns = 2;
        }
        self.tiger_say(actor, "[0]使用[我是M]", updates);
        updates.add(RunUpdate::new("[0]进入[我是M]两回合", actor.0 as usize, actor.0 as usize, 0));
    }

    /// 全在搞我：全体魅惑一回合。
    fn tiger_everyone(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let enemies = self.tiger_enemies(actor);
        if enemies.is_empty() {
            return;
        }
        self.tiger_say(actor, "[0]使用[全在搞我]", updates);
        for target in enemies {
            updates.add_newline();
            self.drain_plain_charm_skill_into(actor, target, updates);
        }
    }

    /// 拜谢者，出神豹：随机敌方获得拜谢压制，并召唤一只猎豹。
    fn tiger_thanks_leopard(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(target) = self.tiger_pick_enemy(actor) else {
            return;
        };
        self.tiger_say(actor, "[0]使用[拜谢者，出神豹]", updates);
        if let Some(state) = self.entities.get_mut(actor).and_then(|e| e.runtime.tiger.as_mut()) {
            state.thanks.push(target);
        }
        updates.add(RunUpdate::new("[1]被拜谢", actor.0 as usize, target.0 as usize, 0));
        self.tiger_spawn_leopard(actor, updates);
    }

    /// 我豹了/对的对的直接抬走猎豹：猎豹自爆，伤害固定为魔的 400%。
    fn tiger_boom(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let leopards = self.tiger_leopards(actor);
        let Some(leopard) = leopards.first().copied() else {
            self.tiger_leopards_skill(actor, updates);
            return;
        };
        let label = if self.rng.next_u8() % 2 == 0 {
            "[0]使用[我豹了]"
        } else {
            "[0]使用[对的对的直接抬走猎豹]"
        };
        self.tiger_say(actor, label, updates);
        let Some(target) = self.tiger_pick_enemy(actor) else {
            return;
        };
        // 与使魔自爆一致：猎豹使用「自爆」，对敌人造成固定伤害后消失。
        let amount = self.entities.get(actor).map_or(0, |e| e.runtime.magic * 4);
        updates.add(RuntimeFrame::replay_update(
            leopard.0 as usize,
            target.0 as usize,
            "[0]使用[自爆]",
            0,
        ));
        if self.apply_legacy_damage_into(leopard, target, amount, updates) {
            self.drain_plain_lethal_damage_into(leopard, target, updates);
        }
        // 猎豹自爆后消失。
        self.drain_summon_explode_self_death_into(leopard, updates);
    }
    /// 红温：HP 低于 50% 时全属性 ×1.5，持续整场。
    fn tiger_red(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(entity) = self.entities.get_mut(actor) else {
            return;
        };
        if entity.runtime.tiger.as_ref().is_some_and(|state| state.red) {
            return;
        }
        if let Some(state) = entity.runtime.tiger.as_mut() {
            state.red = true;
        }
        for (runtime, template) in [
            (&mut entity.runtime.attack, &mut entity.template.attack),
            (&mut entity.runtime.defense, &mut entity.template.defense),
            (&mut entity.runtime.speed, &mut entity.template.speed),
            (&mut entity.runtime.agility, &mut entity.template.agility),
            (&mut entity.runtime.magic, &mut entity.template.magic),
            (&mut entity.runtime.resistance, &mut entity.template.resistance),
            (&mut entity.runtime.wisdom, &mut entity.template.wisdom),
        ] {
            *runtime = *runtime * 3 / 2;
            *template = *template * 3 / 2;
        }
        self.tiger_say(actor, "[0]进入[红温]", updates);
        updates.add(RunUpdate::new("[0]的全属性上升50%", actor.0 as usize, actor.0 as usize, 0));
    }

    /// 幻术：召唤与「涵虚不等式 PFVKEUPBU@TigerStar」同款的幻影。
    fn tiger_shadow(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        self.tiger_say(actor, "[0]使用[幻术]", updates);
        self.drain_plain_shadow_skill_into(actor, 0, updates);
    }

    /// 显得我小：分身一次，技能熟练度保持不变。
    fn tiger_small(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(source) = self.entities.get(actor) else {
            return;
        };
        let (team, attack, magic, defense, resistance, hp, skills) = (
            source.runtime.team,
            source.template.attack,
            source.template.magic,
            source.template.defense,
            source.template.resistance,
            source.template.max_hp,
            source.template.skills.clone(),
        );
        let kind = self
            .registry
            .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
            .expect("core summon kind missing");
        let mut template = PlayerTemplate::with_kind(0, "分身", kind, team, hp, attack);
        template.magic = magic;
        template.defense = defense;
        template.resistance = resistance;
        template.speed = 160;
        template.agility = 106;
        template.wisdom = 106;
        template.magic_point = 40;
        template.skills = skills;
        template.clone_build = self.entities.get(actor).and_then(|e| e.template.clone_build.clone());
        template.id_key_name = "tiger-clone".to_owned();
        template.display_name = "分身".to_owned();
        self.effects.push(QueuedEffect::SpawnWithMessage {
            caster: actor,
            template,
            message: "[1]加入了己方".to_owned(),
        });
        self.drain_effects_into(updates);
        self.tiger_say(actor, "[0]使用[显得我小]", updates);
    }

    /// 豹星刚刚 猎豹了：召唤 1~3 只猎豹。
    fn tiger_leopards_skill(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let count = 1 + usize::from(self.rng.next_u8()) % 3;
        self.tiger_say(actor, "[0]使用[豹星刚刚 猎豹了]", updates);
        for _ in 0..count {
            self.tiger_spawn_leopard(actor, updates);
        }
    }

    /// 加成之后完全恐怖：全属性 +10%，可叠加。
    fn tiger_buff(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if let Some(entity) = self.entities.get_mut(actor) {
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
        }
        self.tiger_say(
            actor,
            "[0]使用[我知道微弱的优势不能保证每场赢，但我一定要在输的那一场红温]",
            updates,
        );
    }

    fn tiger_spawn_leopard(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(owner) = self.entities.get(actor) else {
            return;
        };
        let team = owner.runtime.team;
        let kind = self
            .registry
            .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
            .expect("core summon kind missing");
        let mut template = PlayerTemplate::with_kind(0, LEOPARD, kind, team, 100, 130);
        template.speed = 320;
        template.magic = 130;
        template.defense = 80;
        template.resistance = 80;
        template.agility = 200;
        template.wisdom = 90;
        template.magic_point = 40;
        template.clone_build = self.entities.get(actor).and_then(|e| e.template.clone_build.clone());
        template.id_key_name = "smdd-leopard".to_owned();
        template.display_name = LEOPARD.to_owned();
        let idx = self.entities.next_spawn_idx(&template);
        self.effects.push(QueuedEffect::SpawnWithMessage {
            caster: actor,
            template,
            message: "[1]加入了己方".to_owned(),
        });
        self.drain_effects_into(updates);
        if let Some(spawned) = self.entities.get_mut(idx) {
            spawned.template.id_key_name = "smdd-leopard".to_owned();
        }
    }

    /// 我是M 的受伤反弹：随机属性提升等量数值。
    pub(super) fn record_tiger_damage(&mut self, target: EntityIdx, damage: i32) {
        if damage <= 0 {
            return;
        }
        let Some(entity) = self.entities.get_mut(target) else {
            return;
        };
        let Some(state) = entity.runtime.tiger.as_ref() else {
            return;
        };
        if state.m_turns == 0 {
            return;
        }
        let slot = usize::from(self.rng.next_u8()) % 7;
        let attr = match slot {
            0 => (&mut entity.runtime.attack, &mut entity.template.attack),
            1 => (&mut entity.runtime.defense, &mut entity.template.defense),
            2 => (&mut entity.runtime.speed, &mut entity.template.speed),
            3 => (&mut entity.runtime.agility, &mut entity.template.agility),
            4 => (&mut entity.runtime.magic, &mut entity.template.magic),
            5 => (&mut entity.runtime.resistance, &mut entity.template.resistance),
            _ => (&mut entity.runtime.wisdom, &mut entity.template.wisdom),
        };
        *attr.0 += damage;
        *attr.1 += damage;
    }

    /// 回蓝总倍率：被拜谢（拜谢衍生物或 TigerStar）时减半。
    pub(super) fn mp_regen_scale(&self, target: EntityIdx) -> f64 {
        if self.tiger_thanked(target) || self.baixie_mp_scale(target) < 1.0 {
            0.5
        } else {
            1.0
        }
    }

    /// 目标是否被 TigerStar 拜谢（自然回蓝减半）。
    pub(super) fn tiger_thanked(&self, target: EntityIdx) -> bool {
        self.entities.iter().any(|(_, entity)| {
            entity.runtime.alive && entity.runtime.tiger.as_ref().is_some_and(|state| state.thanks.contains(&target))
        })
    }
}

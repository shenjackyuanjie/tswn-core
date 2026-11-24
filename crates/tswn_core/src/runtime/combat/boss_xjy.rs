//! 新纪元（xjy）Boss：以召唤猎豹、群体真伤与“拜谢”控制为核心的轮转型 Boss。
use super::*;
use crate::runtime::entity::XjyRuntime;

#[cfg(test)]
#[path = "boss_xjy_tests.rs"]
mod tests;

/// 角色名（小写键名）。
pub const ID: &str = "新纪元";
/// 猎豹显示名。
pub const LEOPARD: &str = "猎豹";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum XjySkill {
    /// 谁还没几个猎豹：人数劣势时补猎豹到对方两倍。
    LeopardFill,
    /// 一堆人海cqp60：全体 60 点无视防御伤害，可被闪避。
    Tide,
    /// 三选二：我 rp 大／组队好／xp 还 6。
    ThreeOfTwo,
    /// 我似乎确实有些猎豹了：随机猎豹全属性加倍。
    LeopardBuff,
    /// 我将制霸/我是神/…：铁壁 + 护盾 + 攻魔翻倍。
    Iron,
    /// 他动不动拜谢我…：随机敌方拜谢两回合。
    Thanks,
    /// 那是因为我就是豹：血量低于猎豹时换血并召唤。
    Swap,
}

impl CombatRuntime {
    fn xjy_units(&self, actor: EntityIdx) -> Vec<EntityIdx> {
        let team = self.plain_effective_team(actor);
        self.world
            .flat_alive()
            .iter()
            .copied()
            .filter(|idx| *idx != actor && self.plain_effective_team(*idx) == team)
            .collect()
    }

    fn xjy_enemies(&self, actor: EntityIdx) -> Vec<EntityIdx> {
        let team = self.plain_effective_team(actor);
        self.world
            .flat_alive()
            .iter()
            .copied()
            .filter(|idx| self.plain_effective_team(*idx) != team)
            .collect()
    }

    fn xjy_leopards(&self, actor: EntityIdx) -> Vec<EntityIdx> {
        self.xjy_units(actor)
            .into_iter()
            .filter(|idx| self.entities.get(*idx).is_some_and(|e| e.template.name == LEOPARD))
            .collect()
    }

    fn next_xjy_skill(&self, actor: EntityIdx) -> XjySkill {
        let Some(state) = self.entities.get(actor).and_then(|e| e.runtime.xjy.as_ref()) else {
            return XjySkill::Tide;
        };
        let allies = self.xjy_units(actor).len() + 1;
        let enemies = self.xjy_enemies(actor).len();
        if enemies > allies && enemies > 0 {
            return XjySkill::LeopardFill;
        }
        if !self.xjy_leopards(actor).is_empty() && state.turn % 4 == 0 {
            return XjySkill::LeopardBuff;
        }
        if state.turn % 5 == 0 {
            return XjySkill::Iron;
        }
        if state.turn % 3 == 0 {
            return XjySkill::Thanks;
        }
        if self.leopard_hp_max(actor) > self.entities.get(actor).map_or(0, |e| e.runtime.hp) {
            return XjySkill::Swap;
        }
        if state.turn % 2 == 0 {
            return XjySkill::ThreeOfTwo;
        }
        XjySkill::Tide
    }

    fn leopard_hp_max(&self, actor: EntityIdx) -> i32 {
        self.xjy_leopards(actor)
            .iter()
            .filter_map(|idx| self.entities.get(*idx).map(|e| e.runtime.hp))
            .max()
            .unwrap_or(0)
    }

    pub(super) fn drain_xjy_action_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if !self.entities.get(actor).is_some_and(|e| e.runtime.active()) {
            return;
        }
        self.begin_xjy_turn(actor, updates);
        {
            let Some(state) = self.entities.get_mut(actor).and_then(|e| e.runtime.xjy.as_mut()) else {
                return;
            };
            state.turn = state.turn.saturating_add(1);
        }
        match self.next_xjy_skill(actor) {
            XjySkill::LeopardFill => self.xjy_leopard_fill(actor, updates),
            XjySkill::Tide => self.xjy_tide(actor, updates),
            XjySkill::ThreeOfTwo => self.xjy_three_of_two(actor, updates),
            XjySkill::LeopardBuff => self.xjy_leopard_buff(actor, updates),
            XjySkill::Iron => self.xjy_iron(actor, updates),
            XjySkill::Thanks => self.xjy_thanks(actor, updates),
            XjySkill::Swap => self.xjy_swap(actor, updates),
        }
    }

    fn xjy_say(&self, actor: EntityIdx, message: &'static str, updates: &mut RunUpdates) {
        updates.add_newline();
        updates.add(RunUpdate::new(message, actor.0 as usize, actor.0 as usize, 1).without_replay_target());
        updates.add_newline();
    }

    fn xjy_spawn_leopard(&mut self, actor: EntityIdx, updates: &mut RunUpdates) -> Option<EntityIdx> {
        let owner = self.entities.get(actor)?;
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
        Some(idx)
    }

    /// 谁还没几个猎豹：把己方总人数补到敌方的两倍。
    fn xjy_leopard_fill(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        self.xjy_say(actor, "[0]使用[谁还没几个猎豹]", updates);
        for _ in 0..8 {
            let allies = self.xjy_units(actor).len() + 1;
            let enemies = self.xjy_enemies(actor).len();
            if enemies == 0 || allies >= enemies * 2 {
                break;
            }
            self.xjy_spawn_leopard(actor, updates);
        }
    }

    /// 一堆人海cqp60：全体 60 真伤，可闪避，不可防御/反击/守护。
    fn xjy_tide(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let enemies = self.xjy_enemies(actor);
        if enemies.is_empty() {
            return;
        }
        self.xjy_say(actor, "[0]使用[一堆人海cqp60]", updates);
        for target in enemies {
            if !self.entities.get(target).is_some_and(|e| e.runtime.active()) {
                continue;
            }
            updates.add_newline();
            if self.rng.next_u8() < 64 {
                updates.add(RunUpdate::new("[1][回避]了攻击", actor.0 as usize, target.0 as usize, 0));
                continue;
            }
            if self.apply_damage_into(actor, target, 60, updates) {
                self.drain_plain_lethal_damage_into(actor, target, updates);
            }
        }
    }

    /// 发动时抽取三者中的两个效果。
    fn xjy_three_of_two(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        self.xjy_say(actor, "[0]使用[我 rp 大/组队好/xp 还 6]", updates);
        let first = usize::from(self.rng.next_u8()) % 3;
        let second = (first + 1 + usize::from(self.rng.next_u8()) % 2) % 3;
        for pick in [first, second] {
            match pick {
                0 => self.xjy_rp(actor, updates),
                1 => self.xjy_clone(actor, updates),
                _ => self.xjy_xp(actor, updates),
            }
        }
    }

    fn xjy_rp(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
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
                *runtime = *runtime * 6 / 5;
                *template = *template * 6 / 5;
            }
        }
        updates.add(RunUpdate::new("[0]全属性上升20%", actor.0 as usize, actor.0 as usize, 0));
    }

    fn xjy_clone(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(source) = self.entities.get(actor) else {
            return;
        };
        let (team, attack, magic, defense, resistance, hp) = (
            source.runtime.team,
            source.template.attack,
            source.template.magic,
            source.template.defense,
            source.template.resistance,
            source.template.max_hp,
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
        template.id_key_name = "xjy-clone".to_owned();
        template.display_name = "分身".to_owned();
        self.effects.push(QueuedEffect::SpawnWithMessage {
            caster: actor,
            template,
            message: "[1]加入了己方".to_owned(),
        });
        self.drain_effects_into(updates);
        updates.add(RunUpdate::new("[0]召唤了[分身]", actor.0 as usize, actor.0 as usize, 0));
    }

    fn xjy_xp(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let enemies = self.xjy_enemies(actor);
        for target in enemies {
            if let Some(entity) = self.entities.get_mut(target) {
                for (runtime, template) in [
                    (&mut entity.runtime.attack, &mut entity.template.attack),
                    (&mut entity.runtime.defense, &mut entity.template.defense),
                    (&mut entity.runtime.speed, &mut entity.template.speed),
                    (&mut entity.runtime.agility, &mut entity.template.agility),
                    (&mut entity.runtime.magic, &mut entity.template.magic),
                    (&mut entity.runtime.resistance, &mut entity.template.resistance),
                    (&mut entity.runtime.wisdom, &mut entity.template.wisdom),
                ] {
                    *runtime = (*runtime - 6).max(1);
                    *template = (*template - 6).max(1);
                }
            }
        }
        updates.add(RunUpdate::new("对方全体属性下降6点", actor.0 as usize, actor.0 as usize, 0));
    }

    /// 我似乎确实有些猎豹了：随机猎豹全属性 ×2。
    fn xjy_leopard_buff(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let leopards = self.xjy_leopards(actor);
        if leopards.is_empty() {
            return;
        }
        let index = usize::from(self.rng.next_u8()) % leopards.len();
        let target = leopards[index];
        if let Some(entity) = self.entities.get_mut(target) {
            for (runtime, template) in [
                (&mut entity.runtime.attack, &mut entity.template.attack),
                (&mut entity.runtime.defense, &mut entity.template.defense),
                (&mut entity.runtime.speed, &mut entity.template.speed),
                (&mut entity.runtime.agility, &mut entity.template.agility),
                (&mut entity.runtime.magic, &mut entity.template.magic),
                (&mut entity.runtime.resistance, &mut entity.template.resistance),
                (&mut entity.runtime.wisdom, &mut entity.template.wisdom),
            ] {
                *runtime *= 2;
                *template *= 2;
            }
        }
        self.xjy_say(actor, "[0]使用[我似乎确实有些猎豹了]", updates);
        updates.add(RunUpdate::new("[1]全属性翻倍", actor.0 as usize, target.0 as usize, 0));
    }

    /// 我将制霸/我是神/…：两回合铁壁、100 护盾、攻魔翻倍。
    fn xjy_iron(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let name = match self.rng.next_u8() % 5 {
            0 => "[0]使用[我将制霸]",
            1 => "[0]使用[我是神]",
            2 => "[0]使用[因为很神]",
            3 => "[0]使用[我重开了]",
            _ => "[0]使用[我赢了好多]",
        };
        self.xjy_say(actor, name, updates);
        if let Some(entity) = self.entities.get_mut(actor) {
            entity.runtime.shield += 100;
            entity.runtime.attack *= 2;
            entity.template.attack *= 2;
            entity.runtime.magic *= 2;
            entity.template.magic *= 2;
            let turn = entity.runtime.xjy.as_ref().map_or(0, |state| state.turn);
            if let Some(state) = entity.runtime.xjy.as_mut() {
                state.iron_until = turn + 2;
                state.iron_turn = turn;
            }
        }
        updates.add(RunUpdate::new(
            "[0]进入[铁壁]，获得100点护盾",
            actor.0 as usize,
            actor.0 as usize,
            0,
        ));
    }

    /// 他动不动拜谢我：随机敌方进入拜谢状态两回合，攻魔归零。
    fn xjy_thanks(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let enemies = self.xjy_enemies(actor);
        if enemies.is_empty() {
            return;
        }
        let index = usize::from(self.rng.next_u8()) % enemies.len();
        let target = enemies[index];
        self.xjy_say(actor, "[0]使用[他动不动拜谢我，其实是一种看不起我的表现]", updates);
        if let Some(entity) = self.entities.get_mut(target) {
            let attack = entity.runtime.attack;
            let magic = entity.runtime.magic;
            entity.runtime.attack = 0;
            entity.runtime.magic = 0;
            if let Some(state) = self.entities.get_mut(actor).and_then(|e| e.runtime.xjy.as_mut()) {
                let turn = state.turn;
                state.thanks.push((target, turn + 2, attack, magic));
            }
        }
        updates.add(RunUpdate::new(
            "[1]进入[拜谢]，攻击与魔力归零",
            actor.0 as usize,
            target.0 as usize,
            0,
        ));
    }

    /// 那是因为我就是豹：血量低于猎豹时换血并召唤猎豹。
    fn xjy_swap(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(leopard) = self.xjy_spawn_leopard(actor, updates) else {
            return;
        };
        self.xjy_say(actor, "[0]使用[那是因为我就是豹]", updates);
        let (own_hp, leopard_hp) = {
            let own = self.entities.get(actor).map_or(0, |e| e.runtime.hp);
            let other = self.entities.get(leopard).map_or(0, |e| e.runtime.hp);
            (own, other)
        };
        if let Some(entity) = self.entities.get_mut(actor) {
            entity.runtime.hp = leopard_hp;
        }
        if let Some(entity) = self.entities.get_mut(leopard) {
            entity.runtime.hp = own_hp;
        }
        updates.add(RunUpdate::new("[0]与[1]交换了体力", actor.0 as usize, leopard.0 as usize, 0));
    }

    /// 每回合开始恢复铁壁增益与拜谢状态。
    pub(super) fn begin_xjy_turn(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let mut expired_iron = false;
        let mut expired: smallvec::SmallVec<[(EntityIdx, i32, i32); 4]> = smallvec::SmallVec::new();
        {
            let Some(entity) = self.entities.get_mut(actor) else {
                return;
            };
            let Some(state) = entity.runtime.xjy.as_mut() else {
                return;
            };
            if state.iron_until != 0 && state.turn > state.iron_until {
                state.iron_until = 0;
                expired_iron = true;
            }
            let turn = state.turn;
            state.thanks.retain(|(target, until, attack, magic)| {
                if turn >= *until {
                    expired.push((*target, *attack, *magic));
                    false
                } else {
                    true
                }
            });
        }
        if expired_iron {
            if let Some(entity) = self.entities.get_mut(actor) {
                entity.runtime.attack = (entity.runtime.attack / 2).max(1);
                entity.template.attack = (entity.template.attack / 2).max(1);
                entity.runtime.magic = (entity.runtime.magic / 2).max(1);
                entity.template.magic = (entity.template.magic / 2).max(1);
            }
            updates.add(RunUpdate::new("[0]的[铁壁]解除了", actor.0 as usize, actor.0 as usize, 0));
        }
        for (target, attack, magic) in expired {
            if let Some(entity) = self.entities.get_mut(target) {
                entity.runtime.attack = attack;
                entity.runtime.magic = magic;
            }
            updates.add(RunUpdate::new("[1]从[拜谢]中解除", actor.0 as usize, target.0 as usize, 0));
        }
    }
}

//! 被子（quilt）Boss：不可被选中的后排本体，靠猴区与鱼承受并传导伤害。
//!
//! 本体不攻击，行动完全由脚本决定：钻出猴区、多重魂合并、猴区宣布占领此群、鱼我所欲也。
//! 衍生物受到的伤害有一半经 `DamageSharePolicy::ShareToOwner` 转给本体。
use super::*;
use crate::runtime::entity::QuiltMinionState;

#[cfg(test)]
#[path = "boss_quilt_tests.rs"]
mod tests;

/// Boss 名字（小写键名）。
pub const ID: &str = "quilt";
/// 显示名。
#[allow(dead_code)]
pub const DISPLAY_NAME: &str = "被子";
/// 猴区显示名。
pub const MONKEY_NAME: &str = "猴区";
/// 鱼显示名。
pub const FISH_NAME: &str = "鱼";
/// 猴区宣布占领此群所需的在场数量；比原来的 3 只翻倍。
pub const OCCUPY_MONKEYS: usize = 6;
/// 场上猴区上限。
pub const MONKEY_CAP: usize = 10;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Child {
    Monkey,
    Fish,
}

impl CombatRuntime {
    /// 本体是否为被子。
    pub(super) fn is_quilt(&self, idx: EntityIdx) -> bool { self.entities.get(idx).is_some_and(|e| e.template.name == ID) }

    /// 有存活衍生物时，本体不可被选中；没有衍生物时允许攻击本体，避免战斗卡住。
    pub(super) fn quilt_untargetable(&self, idx: EntityIdx) -> bool {
        self.is_quilt(idx)
            && self
                .quilt_children(idx)
                .iter()
                .any(|child| self.entities.get(*child).is_some_and(|entity| entity.runtime.alive))
    }

    /// 衍生物受伤时应当分摊伤害的被子本体。
    pub(super) fn quilt_share_owner(&self, target: EntityIdx) -> Option<EntityIdx> {
        let entity = self.entities.get(target)?;
        if !matches!(entity.runtime.policies.damage_share, DamageSharePolicy::ShareToOwner) {
            return None;
        }
        let owner = entity.runtime.owner;
        (owner != target && self.is_quilt(owner)).then_some(owner)
    }

    /// 被子的存活衍生物。
    fn quilt_children(&self, boss: EntityIdx) -> Vec<EntityIdx> {
        self.entities
            .iter()
            .filter_map(|(idx, entity)| {
                (entity.runtime.alive && entity.runtime.owner == boss && entity.runtime.is_minion()).then_some(idx)
            })
            .collect()
    }

    fn quilt_monkeys(&self, boss: EntityIdx) -> Vec<EntityIdx> {
        self.quilt_children(boss)
            .into_iter()
            .filter(|idx| self.entities.get(*idx).is_some_and(|e| e.template.name == MONKEY_NAME))
            .collect()
    }

    fn quilt_fish(&self, boss: EntityIdx) -> Vec<EntityIdx> {
        self.quilt_children(boss)
            .into_iter()
            .filter(|idx| self.entities.get(*idx).is_some_and(|e| e.template.name == FISH_NAME))
            .collect()
    }

    /// 被子每回合的脚本：先保证有人在场，再按回合数合并、占领或继续钻猴区。
    pub(super) fn drain_quilt_action_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if !self.entities.get(actor).is_some_and(|e| e.runtime.active()) {
            return;
        }
        let monkeys = self.quilt_monkeys(actor);
        let fish = self.quilt_fish(actor);
        let turn = u32::try_from(self.round).unwrap_or(u32::MAX);
        if fish.is_empty() && monkeys.len() >= 2 {
            self.quilt_spawn(actor, Child::Fish, updates);
            return;
        }
        if monkeys.is_empty() {
            self.quilt_spawn(actor, Child::Monkey, updates);
            return;
        }
        if monkeys.len() >= OCCUPY_MONKEYS && turn % 4 == 0 {
            self.quilt_occupy(actor, updates);
            return;
        }
        if monkeys.len() >= 2 && turn % 3 == 0 {
            self.quilt_merge(actor, updates);
            return;
        }
        if monkeys.len() < MONKEY_CAP {
            self.quilt_spawn(actor, Child::Monkey, updates);
        } else {
            self.quilt_occupy(actor, updates);
        }
    }

    /// 从被子里钻出一只猴区或一条鱼，衍生物受伤时一半传给被子。
    fn quilt_spawn(&mut self, actor: EntityIdx, child: Child, updates: &mut RunUpdates) {
        let owner = self.entities.get(actor).expect("quilt owner missing");
        let team = owner.runtime.team;
        let kind = self
            .registry
            .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
            .expect("core summon kind missing");
        let index = match child {
            Child::Monkey => self.quilt_monkeys(actor).len() + 1,
            Child::Fish => self.quilt_fish(actor).len() + 1,
        };
        let (name, tag, hp, attack, defense, speed, agility, magic, resistance, wisdom, mp, attract) = match child {
            Child::Monkey => ("猴区", "quilt-monkey", 260, 130, 55, 160, 70, 130, 55, 70, 60, 1_048_576.0_f64),
            Child::Fish => ("鱼", "quilt-fish", 320, 70, 110, 150, 60, 70, 110, 60, 40, 4096.0_f64),
        };
        let mut template = PlayerTemplate::with_kind(0, name, kind, team, hp, 60);
        // 第六个参数是攻击力；速度必须单独写，否则召唤物速度为 0 永远轮不到行动。
        template.speed = speed;
        template.attack = attack;
        template.defense = defense;
        template.agility = agility;
        template.magic = magic;
        template.resistance = resistance;
        template.wisdom = wisdom;
        template.magic_point = mp;
        template.attract_bits = attract.to_bits();
        template.id_key_name = tag.to_owned();
        template.display_name = format!("{name} #{index}");
        let idx = self.entities.next_spawn_idx(&template);
        let message = format!("[1]从被子里钻出来了");
        updates.add(RunUpdate::new(
            match child {
                Child::Monkey => "[0]使用[钻出被子]",
                Child::Fish => "[0]使用[鱼我所欲也]",
            },
            actor.0 as usize,
            actor.0 as usize,
            0,
        ));
        updates.add_newline();
        self.effects.push(QueuedEffect::SpawnWithMessage {
            caster: actor,
            template,
            message,
        });
        self.drain_effects_into(updates);
        if let Some(spawned) = self.entities.get_mut(idx) {
            spawned.runtime.policies.damage_share = DamageSharePolicy::ShareToOwner;
            // 召唤会继承 Boss 的 `@!` 徽章名，这里改回衍生物自己的键，前端才能用 emoji 图标。
            spawned.template.id_key_name = tag.to_owned();
            spawned.runtime.quilt_minion = Some(Box::new(QuiltMinionState {
                owner: Some(actor),
                ..QuiltMinionState::default()
            }));
        }
    }

    /// 多重魂：把两只猴区的属性叠到其中一只身上，另一只消失。
    fn quilt_merge(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let monkeys = self.quilt_monkeys(actor);
        if monkeys.len() < 2 {
            return;
        }
        let (keep, eaten) = (monkeys[0], monkeys[1]);
        let Some(source) = self.entities.get(eaten) else {
            return;
        };
        let (attack, defense, speed, agility, magic, resistance, wisdom, max_hp, hp) = (
            source.runtime.attack,
            source.runtime.defense,
            source.runtime.speed,
            source.runtime.agility,
            source.runtime.magic,
            source.runtime.resistance,
            source.runtime.wisdom,
            source.template.max_hp,
            source.runtime.hp,
        );
        if let Some(target) = self.entities.get_mut(keep) {
            target.runtime.attack += attack;
            target.template.attack += attack;
            target.runtime.defense += defense;
            target.template.defense += defense;
            target.runtime.speed += speed;
            target.template.speed += speed;
            target.runtime.agility += agility;
            target.template.agility += agility;
            target.runtime.magic += magic;
            target.template.magic += magic;
            target.runtime.resistance += resistance;
            target.template.resistance += resistance;
            target.runtime.wisdom += wisdom;
            target.template.wisdom += wisdom;
            target.template.max_hp += max_hp;
            target.runtime.hp += hp;
        }
        self.kill_entity_without_damage_into(eaten, updates);
        let index = self.quilt_monkeys(actor).len();
        if let Some(target) = self.entities.get_mut(keep) {
            target.template.display_name = format!("{MONKEY_NAME} #{index}");
        }
        updates.add(RunUpdate::new(
            format!("被子 使用多重魂合并猴区 #{index} 与 #{index} 产生新的猴区 #{index}"),
            actor.0 as usize,
            keep.0 as usize,
            0,
        ));
        updates.add_newline();
    }

    /// 猴区宣布占领此群：全体敌方减速。
    fn quilt_occupy(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let targets = self.quilt_enemies(actor);
        if targets.is_empty() {
            return;
        }
        updates.add(RunUpdate::new("猴区宣布占领此群。", actor.0 as usize, actor.0 as usize, 0));
        for target in targets {
            if !self.entities.get(target).is_some_and(|e| e.runtime.active()) {
                continue;
            }
            updates.add_newline();
            let before = updates.updates.len();
            self.drain_plain_slow_skill_into(actor, target, updates);
            // 只保留“进入迟缓状态”，去掉“使用减速术”那一行。
            if updates.updates.get(before).is_some_and(|update| update.message.contains("使用[减速术]")) {
                updates.updates.remove(before);
            }
        }
    }

    /// 被子的敌方本体。
    fn quilt_enemies(&self, actor: EntityIdx) -> Vec<EntityIdx> {
        let team = self.plain_effective_team(actor);
        self.world
            .flat_alive()
            .iter()
            .copied()
            .filter(|idx| *idx != actor && self.plain_effective_team(*idx) != team)
            .collect()
    }
}

/// 被子衍生物的行动种类。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MinionKind {
    Monkey,
    Fish,
}

impl CombatRuntime {
    /// 衍生物种类；非衍生物返回 None。
    pub(super) fn quilt_minion_kind(&self, idx: EntityIdx) -> Option<MinionKind> {
        let entity = self.entities.get(idx)?;
        entity.runtime.quilt_minion.as_ref()?;
        match entity.template.name.as_str() {
            MONKEY_NAME => Some(MinionKind::Monkey),
            FISH_NAME => Some(MinionKind::Fish),
            _ => None,
        }
    }

    fn minion_state(&self, idx: EntityIdx) -> Option<&QuiltMinionState> {
        self.entities.get(idx)?.runtime.quilt_minion.as_deref()
    }

    fn minion_state_mut(&mut self, idx: EntityIdx) -> Option<&mut QuiltMinionState> {
        self.entities.get_mut(idx)?.runtime.quilt_minion.as_deref_mut()
    }

    fn quilt_owner_of(&self, idx: EntityIdx) -> Option<EntityIdx> { self.minion_state(idx).and_then(|state| state.owner) }

    /// 衍生物是否处于睡5觉/钻被/猴儿冰冻的免伤回合。
    pub(super) fn quilt_minion_sleeping(&self, idx: EntityIdx) -> bool {
        self.minion_state(idx).is_some_and(|state| state.sleeping)
    }

    fn has_ice_state(&self, idx: EntityIdx) -> bool {
        self.entities.get(idx).is_some_and(|entity| {
            entity
                .states
                .entries()
                .iter()
                .any(|entry| matches!(entry.payload, StatePayload::Ice { .. }))
        })
    }

    /// 衍生物的行动：先处理睡5觉与钻被，再按种类执行各自技能。
    pub(super) fn drain_quilt_minion_action_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let Some(kind) = self.quilt_minion_kind(actor) else {
            return;
        };
        if self.minion_state(actor).is_some_and(|state| state.sleeping) {
            if let Some(state) = self.minion_state_mut(actor) {
                state.sleeping = false;
            }
            updates.add(RunUpdate::new("[0]睡醒了", actor.0 as usize, actor.0 as usize, 0));
            return;
        }
        // 猴儿冰冻：冰冻解除后的下一次行动钻进被子一回合。
        let frozen = self.has_ice_state(actor);
        let was_frozen = self.minion_state(actor).is_some_and(|state| state.was_frozen);
        if was_frozen && !frozen {
            if let Some(state) = self.minion_state_mut(actor) {
                state.was_frozen = false;
                state.sleeping = true;
            }

            updates.add(RunUpdate::new("[0]钻进了被子里", actor.0 as usize, actor.0 as usize, 0));
            return;
        }
        if let Some(state) = self.minion_state_mut(actor) {
            state.was_frozen = frozen;
        }
        if self.minion_state(actor).is_some_and(|state| state.pending_spawn) {
            if let Some(state) = self.minion_state_mut(actor) {
                state.pending_spawn = false;
            }
            if let Some(owner) = self.quilt_owner_of(actor) {
                self.quilt_spawn(owner, Child::Monkey, updates);
            }
            return;
        }
        let round = u32::try_from(self.round).unwrap_or(u32::MAX);
        if round > 0 && round % 5 == 0 {
            self.quilt_minion_sleep(actor, updates);
            return;
        }
        match kind {
            MinionKind::Monkey => self.quilt_monkey_action(actor, target, updates),
            MinionKind::Fish => self.quilt_fish_action(actor, target, updates),
        }
    }

    /// 睡5觉：本回合不行动，受击为 0，直到下次行动。
    fn quilt_minion_sleep(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if let Some(state) = self.minion_state_mut(actor) {
            state.sleeping = true;
        }

        updates.add(RunUpdate::new("[0]开始睡5觉", actor.0 as usize, actor.0 as usize, 0));
        updates.add_newline();
        updates.add(RunUpdate::new("[0]钻进了被子里", actor.0 as usize, actor.0 as usize, 0));
    }

    /// 猴区：得意、逮捕、克隆吞噬，其余回合普通攻击。
    fn quilt_monkey_action(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let round = u32::try_from(self.round).unwrap_or(u32::MAX);
        if round > 0 && round % 4 == 0 {
            self.quilt_minion_haste(actor, updates);
            return;
        }
        if round > 0 && round % 3 == 0 {
            self.quilt_arrest(actor, target, updates);
            return;
        }
        if round > 0 && round % 6 == 0 {
            self.quilt_clone_and_devour(actor, updates);
            return;
        }
        self.drain_plain_default_attack_into(actor, target, false, updates);
    }

    /// 得意猴区：加速 + 聚气，可叠层；净化解除时沿用状态自身的解除文案。
    fn quilt_minion_haste(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        updates.add(RunUpdate::new("[0]进入[得意]", actor.0 as usize, actor.0 as usize, 0));
        updates.add_newline();
        self.drain_plain_haste_skill_into(actor, actor, updates);
        self.drain_plain_accumulate_skill_into(actor, updates);
        if let Some(state) = self.minion_state_mut(actor) {
            state.buffs = state.buffs.saturating_add(1);
        }
    }

    /// 逮捕：给目标挂上类似投毒的持续伤害，这只猴区随后消失。
    fn quilt_arrest(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        if !self.entities.get(target).is_some_and(|entity| entity.runtime.active()) {
            return;
        }
        updates.add(RunUpdate::new("[1]被[0]逮捕了", actor.0 as usize, target.0 as usize, 0));
        updates.add_newline();
        self.apply_poison_on_damage(actor, target, 40, updates);
        self.kill_entity_without_damage_into(actor, updates);
    }

    /// 克隆体吞掉原本的猴区：新个体继承属性，旧的消失。
    fn quilt_clone_and_devour(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(source) = self.entities.get(actor) else {
            return;
        };
        let (team, attack, magic, defense, resistance, hp, owner) = (
            source.runtime.team,
            source.template.attack,
            source.template.magic,
            source.template.defense,
            source.template.resistance,
            source.template.max_hp,
            source.runtime.owner,
        );
        let kind = self
            .registry
            .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
            .expect("core summon kind missing");
        let index = self.quilt_monkeys(owner).len() + 1;
        let mut template = PlayerTemplate::with_kind(0, MONKEY_NAME, kind, team, hp, 60);
        template.speed = 160;
        template.attack = attack;
        template.magic = magic;
        template.defense = defense;
        template.resistance = resistance;
        template.id_key_name = "quilt-monkey".to_owned();
        template.display_name = format!("{MONKEY_NAME} #{index}");
        updates.add(RunUpdate::new(
            "[0]使用[houqu -twin stroke of the end-]，[1]钻出来了",
            actor.0 as usize,
            actor.0 as usize,
            0,
        ));
        updates.add_newline();
        let idx = self.entities.next_spawn_idx(&template);
        self.effects.push(QueuedEffect::SpawnWithMessage {
            caster: actor,
            template,
            message: "[1]从被子里钻出来了".to_owned(),
        });
        self.drain_effects_into(updates);
        if let Some(spawned) = self.entities.get_mut(idx) {
            spawned.runtime.policies.damage_share = DamageSharePolicy::ShareToOwner;
            spawned.template.id_key_name = "quilt-monkey".to_owned();
            spawned.runtime.quilt_minion = Some(Box::new(QuiltMinionState {
                owner: Some(owner),
                ..QuiltMinionState::default()
            }));
        }
        self.kill_entity_without_damage_into(actor, updates);
    }

    /// 鱼：钻被、恋之歌、被飞翔猴区逮捕，之后普通攻击。
    fn quilt_fish_action(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let Some(source) = self.entities.get(actor) else {
            return;
        };
        let (team, owner, dives) = (
            source.runtime.team,
            source.runtime.owner,
            self.minion_state(actor).map_or(0, |state| state.dives),
        );
        let monkeys = self.quilt_monkeys(owner);
        let flying = self.minion_state(actor).is_some_and(|state| state.flying);
        if dives < 3 && !flying {
            if let Some(state) = self.minion_state_mut(actor) {
                state.dives = state.dives.saturating_add(1);
                state.pending_spawn = true;
                state.sleeping = true;
            }

            updates.add(RunUpdate::new(
                "[0]上睡觉掀开被子全都是猴区",
                actor.0 as usize,
                actor.0 as usize,
                0,
            ));
            if dives + 1 >= 3 {
                self.quilt_fish_evolve(actor, updates);
            }
            return;
        }
        if !monkeys.is_empty() && !flying {
            updates.add(RunUpdate::new(
                "[0]唱起[さかなホチ恋の歌]",
                actor.0 as usize,
                actor.0 as usize,
                0,
            ));
            for monkey in monkeys {
                self.apply_plain_attack_damage_into(actor, monkey, 10, updates);
                if self.entities.get(monkey).is_some_and(|entity| entity.runtime.active()) {
                    self.quilt_minion_haste(monkey, updates);
                }
            }
            return;
        }
        if !flying {
            if let Some(state) = self.minion_state_mut(actor) {
                state.flying = true;
            }
            updates.add(RunUpdate::new("[0]被飞翔猴区逮捕了", actor.0 as usize, actor.0 as usize, 0));
            self.apply_poison_on_damage(actor, actor, 30, updates);
            return;
        }
        let _ = team;
        self.drain_plain_default_attack_into(actor, target, false, updates);
    }

    /// 用进废退：鱼变成猴区，保留当前属性与状态。
    fn quilt_fish_evolve(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(owner) = self.entities.get(actor).map(|entity| entity.runtime.owner) else {
            return;
        };
        let index = self.quilt_monkeys(owner).len() + 1;
        let Some(entity) = self.entities.get_mut(actor) else {
            return;
        };
        entity.template.name = MONKEY_NAME.to_owned();
        entity.template.display_name = format!("{MONKEY_NAME} #{index}");
        entity.template.id_key_name = "quilt-monkey".to_owned();
        updates.add(RunUpdate::new("[0]用进废退成为猴区", actor.0 as usize, actor.0 as usize, 0));
    }

    /// 猴区被击倒时按概率 HOUQU_BLAST 分裂成两只更小的猴区。
    pub(super) fn drain_quilt_split_into(&mut self, target: EntityIdx, updates: &mut RunUpdates) {
        let Some(entity) = self.entities.get(target) else {
            return;
        };
        let Some(state) = entity.runtime.quilt_minion.as_ref() else {
            return;
        };
        if state.no_split || entity.template.name != MONKEY_NAME || entity.runtime.hp > 0 {
            return;
        }
        if self.rng.next_u8() >= 128 {
            return;
        }
        let owner = state.owner.unwrap_or(entity.runtime.owner);
        updates.add(RunUpdate::new("[0]使用 HOUQU_BLAST", target.0 as usize, target.0 as usize, 0));
        for _ in 0..2 {
            self.quilt_spawn_split_child(owner, target, updates);
        }
    }

    /// HOUQU_BLAST 的子体：血量与属性约为母体的一半，不再继续分裂。
    fn quilt_spawn_split_child(&mut self, owner: EntityIdx, parent: EntityIdx, updates: &mut RunUpdates) {
        let (team, attack, magic, defense, resistance, hp) = {
            let parent = self.entities.get(parent).unwrap();
            (
                parent.runtime.team,
                (parent.template.attack / 2).max(1),
                (parent.template.magic / 2).max(1),
                (parent.template.defense / 2).max(1),
                (parent.template.resistance / 2).max(1),
                (parent.template.max_hp / 2).max(1),
            )
        };
        let kind = self
            .registry
            .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
            .expect("core summon kind missing");
        let index = self.quilt_monkeys(owner).len() + 1;
        let mut template = PlayerTemplate::with_kind(0, MONKEY_NAME, kind, team, hp, 60);
        template.speed = 160;
        template.attack = attack;
        template.magic = magic;
        template.defense = defense;
        template.resistance = resistance;
        template.id_key_name = "quilt-monkey".to_owned();
        template.display_name = format!("{MONKEY_NAME} #{index}");
        let idx = self.entities.next_spawn_idx(&template);
        self.effects.push(QueuedEffect::SpawnWithMessage {
            caster: owner,
            template,
            message: "[1]从被子里钻出来了".to_owned(),
        });
        self.drain_effects_into(updates);
        if let Some(spawned) = self.entities.get_mut(idx) {
            spawned.runtime.policies.damage_share = DamageSharePolicy::ShareToOwner;
            spawned.template.id_key_name = "quilt-monkey".to_owned();
            spawned.runtime.quilt_minion = Some(Box::new(QuiltMinionState {
                owner: Some(owner),
                no_split: true,
                ..QuiltMinionState::default()
            }));
        }
    }
}

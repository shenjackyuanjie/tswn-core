//! 被子（quilt）Boss：不可被选中的后排本体，靠猴区与鱼承受并传导伤害。
//!
//! 本体不攻击，行动完全由脚本决定：钻出猴区、多重魂合并、猴区宣布占领此群、鱼我所欲也。
//! 衍生物受到的伤害有一半经 `DamageSharePolicy::ShareToOwner` 转给本体。
use super::*;

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
/// 场上猴区上限。
pub const MONKEY_CAP: usize = 5;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Child {
    Monkey,
    Fish,
}

impl CombatRuntime {
    /// 本体是否为被子。
    pub(super) fn is_quilt(&self, idx: EntityIdx) -> bool { self.entities.get(idx).is_some_and(|e| e.template.name == ID) }

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
        if monkeys.len() >= 3 && turn % 4 == 0 {
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
            Child::Monkey => ("猴区", "quilt-monkey", 120, 130, 55, 100, 70, 130, 55, 70, 60, 1_048_576.0_f64),
            Child::Fish => ("鱼", "quilt-fish", 150, 70, 110, 100, 60, 70, 110, 60, 40, 4096.0_f64),
        };
        let mut template = PlayerTemplate::with_kind(0, name, kind, team, hp, speed);
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
            self.drain_plain_slow_skill_into(actor, target, updates);
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

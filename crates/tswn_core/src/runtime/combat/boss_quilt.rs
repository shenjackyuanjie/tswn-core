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
pub const OCCUPY_MONKEYS: usize = 4;
/// 场上猴区上限。
pub const MONKEY_CAP: usize = 10;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Child {
    Monkey,
    Fish,
}

impl CombatRuntime {
    /// 本体是否为被子。
    pub(super) fn is_quilt(&self, idx: EntityIdx) -> bool {
        self.entities
            .get(idx)
            .is_some_and(|e| e.template.name == ID && e.runtime.flags.contains(PlayerKindFlags::BOSS))
    }

    /// 被子始终排除在敌方选敌之外，空场时仍可轮到本体召唤。
    pub(super) fn quilt_untargetable(&self, idx: EntityIdx) -> bool { self.is_quilt(idx) }

    /// 选敌时排除被子，保留其在调度器和判胜中的存活记录。
    pub(super) fn quilt_target_pool<'a>(entities: &EntityArena, world: &'a WorldArena) -> std::borrow::Cow<'a, [EntityIdx]> {
        let alive = world.flat_alive();
        let visible = |idx| {
            entities
                .get(idx)
                .is_none_or(|e| e.template.name != ID || !e.runtime.flags.contains(PlayerKindFlags::BOSS))
        };
        if alive.iter().copied().all(visible) {
            std::borrow::Cow::Borrowed(alive)
        } else {
            std::borrow::Cow::Owned(alive.iter().copied().filter(|idx| visible(*idx)).collect())
        }
    }

    /// 对衍生物统一计算钻被免伤与飞翔减伤。
    pub(super) fn quilt_damage(&self, target: EntityIdx, amount: i32) -> i32 {
        if self.is_quilt(target) || self.quilt_minion_sleeping(target) {
            return 0;
        }
        if let Some(owner) = self.quilt_owner_of(target) {
            if self.quilt_minion_kind(target) == Some(MinionKind::Monkey)
                && self.quilt_fish(owner).iter().any(|idx| self.minion_state(*idx).is_some_and(|s| s.flying))
            {
                return amount * 3 / 4;
            }
        }
        amount
    }

    /// 按衍生物受到的伤害（而不是实际掉血）传导一半给本体；1 血吃 200 伤也会传导 100。
    pub(super) fn relay_quilt_damage(&mut self, caster: EntityIdx, target: EntityIdx, amount: i32, updates: &mut RunUpdates) {
        let shared = amount.max(0) / 2;
        if shared == 0 {
            return;
        }
        let Some(owner) = self.quilt_share_owner(target) else {
            return;
        };
        let e = self.entities.get_mut(owner).unwrap();
        if !e.runtime.active() {
            return;
        }
        e.runtime.hp = (e.runtime.hp - shared).max(0);
        updates.add_with(|| RuntimeFrame::legacy_damage_update(caster.0 as usize, owner.0 as usize, shared));
        if e.runtime.hp == 0 {
            self.drain_plain_lethal_damage_into(caster, owner, updates);
        }
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

    /// 用本体行动计数安排扩张，先让占领达到门槛，再消耗猴区合并。
    pub(super) fn drain_quilt_action_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if !self.entities.get(actor).is_some_and(|e| e.runtime.active()) {
            return;
        }
        let state = self
            .entities
            .get_mut(actor)
            .unwrap()
            .runtime
            .quilt_minion
            .get_or_insert_with(Default::default);
        state.turn += 1;
        let turn = state.turn;
        let monkeys = self.quilt_monkeys(actor);
        if monkeys.len() >= OCCUPY_MONKEYS && !self.minion_state(actor).unwrap().occupied {
            self.minion_state_mut(actor).unwrap().occupied = true;
            self.quilt_occupy(actor, updates);
        } else if !monkeys.is_empty() && self.quilt_fish(actor).is_empty() && turn % 3 == 0 {
            self.quilt_spawn(actor, Child::Fish, updates);
        } else if monkeys.len() >= 2
            && (monkeys.len() >= MONKEY_CAP || (self.minion_state(actor).unwrap().occupied && turn % 4 == 0) || turn % 11 == 0)
        {
            self.quilt_merge(actor, updates);
        } else {
            self.quilt_spawn(actor, Child::Monkey, updates);
        }
    }

    /// 从被子里钻出一只猴区或一条鱼，衍生物受伤时一半传给被子。
    pub(super) fn quilt_spawn(&mut self, actor: EntityIdx, child: Child, updates: &mut RunUpdates) {
        let owner = self.entities.get(actor).expect("quilt owner missing");
        let team = owner.runtime.team;
        let kind = self
            .registry
            .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
            .expect("core summon kind missing");
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
        template.display_name = name.to_owned();
        let idx = self.entities.next_spawn_idx(&template);
        let message = "[1]从被子里钻出来了".to_owned();
        // 猴区只显示钻出事件；鱼保留其传统技能发动行。
        if child == Child::Fish {
            updates.add(RunUpdate::new("[0]使用[鱼我所欲也]", actor.0 as usize, actor.0 as usize, 0));
            updates.add_newline();
        }
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

    /// 合并至多三只猴区，保留最早入场者的睡眠计数并生成独立实体编号。
    fn quilt_merge(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let monkeys = self.quilt_monkeys(actor);
        let sources = &monkeys[..monkeys.len().min(3)];
        let [keep, rest @ ..] = sources else {
            return;
        };
        if rest.is_empty() {
            return;
        }
        let first = self.entities.get(*keep).unwrap();
        let mut template = first.template.clone();
        let mut hp = first.runtime.hp;
        let state = first.runtime.quilt_minion.clone();
        for &idx in rest {
            let e = self.entities.get(idx).unwrap();
            template.attack += e.template.attack;
            template.defense += e.template.defense;
            template.speed += e.template.speed;
            template.agility += e.template.agility;
            template.magic += e.template.magic;
            template.resistance += e.template.resistance;
            template.wisdom += e.template.wisdom;
            template.max_hp += e.template.max_hp;
            hp += e.runtime.hp;
        }
        let merged = self.spawn_quilt_copy(actor, template, state, hp);
        let mut line = RunUpdate::new("[0]使用[多重魂]合并[2]，产生新的[1]", actor.0 as usize, merged.0 as usize, 0);
        line.targets.extend(sources.iter().map(|idx| idx.0 as usize));
        updates.add(line);
        for &idx in sources {
            self.quilt_remove(idx, updates);
        }
    }

    /// 复制和合并都由被子直接持有新实体，旧猴区消失不会带走它。
    fn spawn_quilt_copy(
        &mut self,
        owner: EntityIdx,
        template: PlayerTemplate,
        state: Option<Box<QuiltMinionState>>,
        hp: i32,
    ) -> EntityIdx {
        let child = self
            .entities
            .spawn_from_template_with_owner(template, &self.registry, Some(owner), Some(owner));
        let e = self.entities.get_mut(child).unwrap();
        e.template.id_key_name = "quilt-monkey".into();
        e.runtime.policies.damage_share = DamageSharePolicy::ShareToOwner;
        e.runtime.quilt_minion = state;
        e.runtime.hp = hp;
        self.world.add_spawned_alive(child, e.runtime.team);
        child
    }

    fn quilt_remove(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if self.kill_entity_without_damage_into(actor, updates) {
            updates.add_newline();
            updates.add(RunUpdate::new("[1]消失了", actor.0 as usize, actor.0 as usize, 0));
        }
    }

    /// 占领时直接给整队施加迟缓，只有猴区作为施法者。
    fn quilt_occupy(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(monkey) = self.quilt_monkeys(actor).first().copied() else {
            return;
        };
        updates.add(RunUpdate::new("猴区宣布占领此群。", monkey.0 as usize, monkey.0 as usize, 0));
        for target in self.quilt_enemies(actor) {
            self.apply_plain_slow_into(monkey, target, false, updates);
        }
    }

    /// 被子的敌方本体。
    fn quilt_enemies(&self, actor: EntityIdx) -> Vec<EntityIdx> {
        let team = self.plain_effective_team(actor);
        self.world
            .flat_alive()
            .iter()
            .copied()
            .filter(|idx| *idx != actor && !self.is_quilt(*idx) && self.plain_effective_team(*idx) != team)
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

    /// 冰冻解除时立即钻被；若解冻当轮也获选，该轮仍保留免伤。
    pub(super) fn quilt_thaw(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if self.quilt_minion_kind(actor) != Some(MinionKind::Monkey) {
            return;
        }
        let round = self.round;
        let state = self.minion_state_mut(actor).unwrap();
        state.sleeping = true;
        state.thawed_at = Some(round);
        updates.add_newline();
        updates.add(RunUpdate::new("[0]钻进了被子里", actor.0 as usize, actor.0 as usize, 0));
    }

    /// 每只衍生物单独计数，睡觉占第 5、10、15…次行动，免伤到下次行动开始。
    pub(super) fn drain_quilt_minion_action_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let Some(kind) = self.quilt_minion_kind(actor) else {
            return;
        };
        let round = self.round;
        let state = self.minion_state_mut(actor).unwrap();
        state.turn += 1;
        let turn = state.turn;
        if state.thawed_at != Some(round) {
            state.sleeping = false;
        }
        if std::mem::take(&mut state.pending_spawn) {
            if let Some(owner) = self.quilt_owner_of(actor) {
                self.quilt_spawn(owner, Child::Monkey, updates);
            }
        }
        if turn % 5 == 0 {
            self.quilt_minion_sleep(actor, updates);
            return;
        }
        if self.minion_state(actor).unwrap().thawed_at == Some(round) {
            return;
        }
        match kind {
            MinionKind::Monkey => self.quilt_monkey_action(actor, target, updates),
            MinionKind::Fish => self.quilt_fish_action(actor, target, updates),
        }
    }

    fn quilt_minion_sleep(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        updates.add(RunUpdate::new("[0]开始睡5觉", actor.0 as usize, actor.0 as usize, 0));
        updates.add_newline();
        updates.add(RunUpdate::new("[0]钻进了被子里", actor.0 as usize, actor.0 as usize, 0));
        self.quilt_dive(actor, updates);
    }

    fn quilt_dive(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let fish = self.quilt_minion_kind(actor) == Some(MinionKind::Fish);
        let s = self.minion_state_mut(actor).unwrap();
        s.sleeping = true;
        if fish {
            s.dives += 1;
            if s.dives == 3 {
                self.quilt_fish_evolve(actor, updates);
            }
        }
    }

    /// 固定技能窗口互不覆盖，逮捕无需等到全局轮数取模命中。
    fn quilt_monkey_action(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let turn = self.minion_state(actor).unwrap().turn;
        match turn % 8 {
            2 | 6 => self.quilt_minion_haste(actor, updates),
            3 => self.quilt_clone_and_devour(actor, updates),
            // 占领前保留足够的猴区，同时给长局中的逮捕留出固定窗口。
            4 | 7
                if !self.is_quilt(target)
                    && self
                        .quilt_owner_of(actor)
                        .is_some_and(|owner| self.minion_state(owner).is_some_and(|s| s.occupied) || turn >= 12) =>
            {
                self.quilt_arrest(actor, target, updates)
            }
            _ => self.quilt_minion_attack(actor, target, updates),
        }
    }

    fn quilt_minion_attack(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        if self.is_quilt(target) || target == actor {
            return;
        }
        let r = &self.entities.get(actor).unwrap().runtime;
        let magic = r.magic_point >= 8 && self.rng.c50();
        if magic {
            self.entities.get_mut(actor).unwrap().runtime.magic_point -= 8;
        }
        self.drain_plain_default_attack_into(actor, target, magic, updates);
    }

    /// 得意独立计时，每次添加一层，速度与攻击倍率由对应计算入口读取。
    fn quilt_minion_haste(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let s = self.minion_state_mut(actor).unwrap();
        s.pride.push(s.turn + 2);
        updates.add(RunUpdate::new("[0]进入[得意]", actor.0 as usize, actor.0 as usize, 0));
    }

    fn quilt_arrest(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        if target == actor || !self.entities.get(target).is_some_and(|e| e.runtime.active()) {
            return;
        }
        let damage = (self.entities.get(actor).unwrap().runtime.magic / 3).max(1);
        updates.add(RunUpdate::new("[1]被[0]逮捕了", actor.0 as usize, target.0 as usize, 0));
        self.add_quilt_arrest(actor, target, damage);
        self.quilt_remove(actor, updates);
    }

    fn add_quilt_arrest(&mut self, actor: EntityIdx, target: EntityIdx, damage: i32) {
        self.entities.get_mut(target).unwrap().runtime.quilt_arrests.push((actor, damage, 3));
    }

    /// 逮捕按受害者行动结算，不注册中毒状态；飞翔跟随在场的鱼。
    pub(super) fn quilt_post_action(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if let Some(s) = self.minion_state_mut(actor) {
            let before = s.pride.len();
            s.pride.retain(|until| *until > s.turn);
            if before != s.pride.len() && s.pride.is_empty() {
                updates.add_newline();
                updates.add(RunUpdate::new("[0]从[得意]中解除", actor.0 as usize, actor.0 as usize, 0));
            }
        }
        let Some(e) = self.entities.get_mut(actor) else {
            return;
        };
        let mut arrests = std::mem::take(&mut e.runtime.quilt_arrests);
        let had = !arrests.is_empty();
        for (caster, damage, remaining) in &mut arrests {
            if !self.entities.get(actor).is_some_and(|e| e.runtime.active()) {
                break;
            }
            *remaining -= 1;
            updates.add_newline();
            updates.add(RunUpdate::new("[1]的[逮捕]发作", caster.0 as usize, actor.0 as usize, 0));
            if self.apply_plain_attack_damage_into(*caster, actor, *damage, updates) {
                self.drain_plain_lethal_damage_into(*caster, actor, updates);
            }
        }
        arrests.retain(|(_, _, remaining)| *remaining > 0);
        if let Some(e) = self.entities.get_mut(actor).filter(|e| e.runtime.active()) {
            if had && arrests.is_empty() {
                updates.add_newline();
                updates.add(RunUpdate::new("[0]从[逮捕]中解除", actor.0 as usize, actor.0 as usize, 0));
            }
            e.runtime.quilt_arrests = arrests;
        }
    }

    fn quilt_clone_and_devour(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let source = self.entities.get(actor).unwrap();
        let owner = source.runtime.owner;
        let clone = self.spawn_quilt_copy(
            owner,
            source.template.clone(),
            source.runtime.quilt_minion.clone(),
            source.runtime.hp,
        );
        updates.add(RunUpdate::new(
            "[0]使用[houqu -twin stroke of the end-]，[1]钻出来了",
            actor.0 as usize,
            clone.0 as usize,
            0,
        ));
        updates.add_newline();
        updates.add(RunUpdate::new("[1]吞噬了[0]", actor.0 as usize, clone.0 as usize, 0));
        self.quilt_remove(actor, updates);
    }

    /// 鱼按自己的行动轮换三种技能，第五次行动仍优先睡觉。
    fn quilt_fish_action(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let state = self.minion_state(actor).unwrap();
        let owner = state.owner.unwrap();
        let monkeys = self.quilt_monkeys(owner);
        match state.turn % 4 {
            1 => {
                self.minion_state_mut(actor).unwrap().pending_spawn = true;
                updates.add(RunUpdate::new(
                    "[0]上睡觉掀开被子全都是猴区",
                    actor.0 as usize,
                    actor.0 as usize,
                    0,
                ));
                self.quilt_dive(actor, updates);
            }
            2 if !monkeys.is_empty() => {
                updates.add(RunUpdate::new(
                    "[0]使用[さかなホチ恋の歌]",
                    actor.0 as usize,
                    actor.0 as usize,
                    0,
                ));
                for monkey in monkeys {
                    updates.add_newline();
                    if self.apply_plain_attack_damage_into(actor, monkey, 10, updates) {
                        self.drain_plain_lethal_damage_into(actor, monkey, updates);
                    }
                    if self.entities.get(monkey).is_some_and(|e| e.runtime.active()) {
                        self.quilt_minion_haste(monkey, updates);
                    }
                }
            }
            3 if !state.flying => {
                self.minion_state_mut(actor).unwrap().flying = true;
                self.add_quilt_arrest(actor, actor, 5);
                updates.add(RunUpdate::new("[0]被飞翔猴区逮捕了", actor.0 as usize, actor.0 as usize, 0));
            }
            _ => self.quilt_minion_attack(actor, target, updates),
        }
    }

    /// 转化沿用原实体编号和状态，旧身份在日志中显式保留。
    fn quilt_fish_evolve(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let entity = self.entities.get_mut(actor).unwrap();
        entity.template.name = MONKEY_NAME.into();
        entity.template.display_name = MONKEY_NAME.into();
        entity.template.id_key_name = "quilt-monkey".into();
        updates.add_newline();
        updates.add(RunUpdate::new(
            format!("#{} 鱼用进废退成为[1]", actor.0),
            actor.0 as usize,
            actor.0 as usize,
            0,
        ));
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
        if !self.entities.get(owner).is_some_and(|e| e.runtime.active()) {
            return;
        }
        let children = [
            self.quilt_spawn_split_child(owner, target),
            self.quilt_spawn_split_child(owner, target),
        ];
        let mut line = RunUpdate::new(
            "[0]使用[HOUQU_BLAST]，[2]钻出来了",
            target.0 as usize,
            children[0].0 as usize,
            0,
        );
        line.targets.extend(children.map(|idx| idx.0 as usize));
        updates.add(line);
    }

    /// 子体保留猴区类型，七围和最大体力减半，且不再递归分裂。
    fn quilt_spawn_split_child(&mut self, owner: EntityIdx, parent: EntityIdx) -> EntityIdx {
        let mut template = self.entities.get(parent).unwrap().template.clone();
        for stat in [
            &mut template.attack,
            &mut template.magic,
            &mut template.defense,
            &mut template.resistance,
            &mut template.speed,
            &mut template.wisdom,
            &mut template.agility,
            &mut template.max_hp,
        ] {
            *stat = (*stat / 2).max(1);
        }
        let hp = template.max_hp;
        self.spawn_quilt_copy(
            owner,
            template,
            Some(Box::new(QuiltMinionState {
                owner: Some(owner),
                no_split: true,
                ..Default::default()
            })),
            hp,
        )
    }
}

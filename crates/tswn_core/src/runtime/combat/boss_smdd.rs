//! smdd（曙光）Boss：以“做局”为核心的轮转型 Boss，技能全部取自群成员 3023226682 的发言。
use super::*;

#[cfg(test)]
#[path = "boss_smdd_tests.rs"]
mod tests;

/// 角色名（小写键名）。
pub const ID: &str = "smdd";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SmddSkill {
    /// 我 rp 神：全属性 ×1.25，最多两层。
    Rp,
    /// 虎星恒等式：召唤 Gordon 与一对笑面虎。
    Tiger,
    /// 卿言猴儿：十连击，每击 90%。
    Monkey,
    /// 孤常卖牢：召唤猎豹。
    Leopard,
    /// 矿机测号：随机敌方除智外属性减半，智变 36。
    Mining,
    /// 赢：按敌方人数连续普攻，回避率固定 50%。
    Win,
    /// 可能我是名竞之神：随机三选一。
    God,
}

impl CombatRuntime {
    fn smdd_enemies(&self, actor: EntityIdx) -> Vec<EntityIdx> {
        let team = self.plain_effective_team(actor);
        self.world
            .flat_alive()
            .iter()
            .copied()
            .filter(|idx| *idx != actor && self.plain_effective_team(*idx) != team)
            .collect()
    }

    fn next_smdd_skill(&self, actor: EntityIdx) -> SmddSkill {
        let Some(state) = self.entities.get(actor).and_then(|e| e.runtime.smdd.as_ref()) else {
            return SmddSkill::Win;
        };
        match state.turn % 7 {
            1 => SmddSkill::Rp,
            2 => SmddSkill::Tiger,
            3 => SmddSkill::Monkey,
            4 => SmddSkill::Leopard,
            5 => SmddSkill::Mining,
            6 => SmddSkill::Win,
            _ => SmddSkill::God,
        }
    }

    /// 被敌方本体命中时做局层数 +1；开始做局始终激活，每个自身行动也会 +1。
    pub(super) fn record_smdd_hit(&mut self, target: EntityIdx) {
        if let Some(state) = self.entities.get_mut(target).and_then(|e| e.runtime.smdd.as_mut()) {
            state.stacks = (state.stacks + 1).min(8);
        }
    }

    pub(super) fn drain_smdd_action_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if !self.entities.get(actor).is_some_and(|e| e.runtime.active()) {
            return;
        }
        // 开始做局：始终激活的被动，每次自身行动叠一层。
        let stacks = {
            let Some(state) = self.entities.get_mut(actor).and_then(|e| e.runtime.smdd.as_mut()) else {
                return;
            };
            state.turn = state.turn.saturating_add(1);
            state.stacks = (state.stacks + 1).min(8);
            state.stacks
        };
        if stacks >= 2 {
            updates.add(RunUpdate::new(
                format!("[0]的[局]累积到{stacks}层"),
                actor.0 as usize,
                actor.0 as usize,
                0,
            ));
            updates.add_newline();
        }
        match self.next_smdd_skill(actor) {
            SmddSkill::Rp => self.smdd_rp(actor, updates),
            SmddSkill::Tiger => self.smdd_tiger(actor, updates),
            SmddSkill::Monkey => self.smdd_monkey(actor, updates),
            SmddSkill::Leopard => self.smdd_leopard(actor, updates),
            SmddSkill::Mining => self.smdd_mining(actor, updates),
            SmddSkill::Win => self.smdd_win(actor, updates),
            SmddSkill::God => self.smdd_god(actor, updates),
        }
    }

    /// 只播报用户给定的文本，不再保留任何旧技能名。
    fn smdd_say(&self, actor: EntityIdx, message: &'static str, updates: &mut RunUpdates) {
        updates.add_newline();
        updates.add(RunUpdate::new(message, actor.0 as usize, actor.0 as usize, 1).without_replay_target());
        updates.add_newline();
    }

    /// 抛开事实不谈：全属性 ×1.25，最多两层。
    fn smdd_rp(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(entity) = self.entities.get_mut(actor) else {
            return;
        };
        let Some(state) = entity.runtime.smdd.as_mut() else {
            return;
        };
        if state.rp_stacks >= 2 {
            self.smdd_win(actor, updates);
            return;
        }
        state.rp_stacks += 1;
        let stacks = state.rp_stacks;
        for (runtime, template) in [
            (&mut entity.runtime.attack, &mut entity.template.attack),
            (&mut entity.runtime.defense, &mut entity.template.defense),
            (&mut entity.runtime.speed, &mut entity.template.speed),
            (&mut entity.runtime.agility, &mut entity.template.agility),
            (&mut entity.runtime.magic, &mut entity.template.magic),
            (&mut entity.runtime.resistance, &mut entity.template.resistance),
            (&mut entity.runtime.wisdom, &mut entity.template.wisdom),
        ] {
            *runtime = *runtime * 5 / 4;
            *template = *template * 5 / 4;
        }
        self.smdd_say(actor, "[0]使用[我 rp 神]", updates);
        updates.add(RunUpdate::new(
            format!("[0]全属性上升25%（{stacks}层）"),
            actor.0 as usize,
            actor.0 as usize,
            0,
        ));
    }

    /// 虎星恒等式：召唤 Gordon 与一对笑面虎。
    fn smdd_tiger(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(owner) = self.entities.get(actor) else {
            return;
        };
        let team = owner.runtime.team;
        self.smdd_say(actor, "[0]使用[本队单双人都一堆]", updates);
        for (display, icon_key, hp, attack, speed) in [
            ("Gordon RLHTKAZRMFFJ@nan", "Gordon RLHTKAZRMFFJ@nan", 140, 120, 150),
            ("一对笑面虎 my7CAxNHHz@XJ联队", "XJ联队@XJ联队", 120, 110, 140),
            ("2LRLYnOw5pmmhXG@XJ联队", "XJ联队@XJ联队", 120, 110, 140),
        ] {
            self.smdd_spawn_unit(actor, team, display, icon_key, hp, attack, speed, updates);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn smdd_spawn_unit(
        &mut self,
        actor: EntityIdx,
        team: usize,
        display: &str,
        icon_key: &str,
        hp: i32,
        attack: i32,
        speed: i32,
        updates: &mut RunUpdates,
    ) {
        let kind = self
            .registry
            .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
            .expect("core summon kind missing");
        // 名字必须完整保留前缀与空格（含 @队伍），否则名字派生属性会变。
        let mut template = PlayerTemplate::with_kind(0, display, kind, team, hp, attack);
        template.speed = speed;
        template.magic = attack;
        template.defense = 70;
        template.resistance = 70;
        template.agility = 90;
        template.wisdom = 70;
        template.magic_point = 40;
        template.clone_build = self.entities.get(actor).and_then(|e| e.template.clone_build.clone());
        template.id_key_name = icon_key.to_owned();
        template.display_name = display.to_owned();
        let idx = self.entities.next_spawn_idx(&template);
        self.effects.push(QueuedEffect::SpawnWithMessage {
            caster: actor,
            template,
            message: "[1]加入了己方".to_owned(),
        });
        self.drain_effects_into(updates);
        // 召唤会继承 Boss 的 @! 徽章名，这里改回完整的名字与图标键。
        if let Some(spawned) = self.entities.get_mut(idx) {
            spawned.template.id_key_name = icon_key.to_owned();
            spawned.template.display_name = display.to_owned();
            spawned.template.name = display.to_owned();
        }
    }

    /// 卿言猴儿：十连击，每击 90%。
    fn smdd_monkey(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        self.smdd_say(actor, "[0]使用[唉，乱杀]", updates);
        for _ in 0..10 {
            if !self.entities.get(actor).is_some_and(|e| e.runtime.active()) {
                break;
            }
            let Some(current) = self.select_plain_default_attack_target(actor, true) else {
                break;
            };
            let atp = self.entities.get(actor).map_or(0.0, |entity| entity.runtime.get_at(false, &mut self.rng)) * 0.9;
            self.drain_plain_attack_with_atp_into(actor, current, false, atp, updates);
        }
    }

    /// 孤常卖牢：召唤猎豹。
    fn smdd_leopard(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(owner) = self.entities.get(actor) else {
            return;
        };
        let team = owner.runtime.team;
        self.smdd_say(actor, "[0]使用[猎豹不彻底等于彻底不猎豹]", updates);
        self.smdd_spawn_unit(actor, team, "猎豹", "smdd-leopard", 100, 130, 320, updates);
    }

    /// 矿机测号：随机敌方除智外属性减半，智变 36。
    fn smdd_mining(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let enemies = self.smdd_enemies(actor);
        if enemies.is_empty() {
            return;
        }
        let index = usize::from(self.rng.next_u8()) % enemies.len();
        let target = enemies[index];
        self.smdd_say(actor, "[0]使用[神人，评价是弱智]", updates);
        if let Some(entity) = self.entities.get_mut(target) {
            for (runtime, template) in [
                (&mut entity.runtime.attack, &mut entity.template.attack),
                (&mut entity.runtime.defense, &mut entity.template.defense),
                (&mut entity.runtime.speed, &mut entity.template.speed),
                (&mut entity.runtime.agility, &mut entity.template.agility),
                (&mut entity.runtime.magic, &mut entity.template.magic),
                (&mut entity.runtime.resistance, &mut entity.template.resistance),
            ] {
                *runtime /= 2;
                *template /= 2;
            }
            entity.runtime.wisdom = 36;
            entity.template.wisdom = 36;
        }
        updates.add(RunUpdate::new(
            "[1]的属性被削半，智力变为36",
            actor.0 as usize,
            target.0 as usize,
            0,
        ));
    }

    /// 赢：按敌方人数连续普攻，回避率固定 50%。
    fn smdd_win(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let enemies = self.smdd_enemies(actor);
        if enemies.is_empty() {
            return;
        }
        let hits = enemies.len().clamp(1, 5);
        let label: &'static str = match hits {
            1 => "[0]使用[单人组：赢]",
            2 => "[0]使用[双人组：赢]",
            3 => "[0]使用[三人组：赢]",
            4 => "[0]使用[四人组：赢]",
            _ => "[0]使用[五人组：赢]",
        };
        self.smdd_say(actor, label, updates);
        for _ in 0..hits {
            let enemies = self.smdd_enemies(actor);
            if enemies.is_empty() {
                break;
            }
            let target = enemies[usize::from(self.rng.next_u8()) % enemies.len()];
            updates.add_newline();
            // 固定 50% 回避，不参考命中与敏捷。
            if self.rng.next_u8() < 128 {
                updates.add(RunUpdate::new("[1][回避]了攻击", actor.0 as usize, target.0 as usize, 0));
                continue;
            }
            let amount = self.smdd_at(actor, 1.0) as i32;
            if self.apply_plain_attack_damage_into(actor, target, amount, updates) {
                self.drain_plain_lethal_damage_into(actor, target, updates);
            }
        }
    }

    /// 可能我是名竞之神：随机三选一。
    fn smdd_god(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        self.smdd_say(actor, "[0]使用[可能我是名竞之神]", updates);
        match self.rng.next_u8() % 3 {
            0 => {
                let enemies = self.smdd_enemies(actor);
                if let Some(target) = enemies.first().copied() {
                    updates.add(RunUpdate::new("[1]受到大额伤害", actor.0 as usize, target.0 as usize, 0));
                    let amount = self.smdd_at(actor, 2.0) as i32;
                    if self.apply_plain_attack_damage_into(actor, target, amount, updates) {
                        self.drain_plain_lethal_damage_into(actor, target, updates);
                    }
                }
            }
            1 => {
                let (before, healed) = {
                    let Some(entity) = self.entities.get_mut(actor) else {
                        return;
                    };
                    let before = entity.runtime.hp;
                    entity.runtime.hp = (before + entity.template.max_hp / 4).min(entity.template.max_hp);
                    (before, entity.runtime.hp - before)
                };
                let mut line = RunUpdate::new("[0]回复体力[2]点", actor.0 as usize, actor.0 as usize, 0);
                line.param = Some(healed as u32);
                line.hp_changes = Some(
                    vec![crate::runtime::update::RunUpdateHp {
                        player: actor.0 as usize,
                        before,
                        after: before + healed,
                    }]
                    .into_boxed_slice(),
                );
                updates.add(line);
            }
            _ => {
                if let Some(entity) = self.entities.get_mut(actor) {
                    entity.runtime.speed += 40;
                    entity.template.speed += 40;
                }
                updates.add(RunUpdate::new("[0]速度上升", actor.0 as usize, actor.0 as usize, 0));
            }
        }
    }

    fn smdd_at(&self, actor: EntityIdx, scale: f64) -> f64 {
        self.entities.get(actor).map_or(0.0, |entity| f64::from(entity.runtime.attack) * scale)
    }
}

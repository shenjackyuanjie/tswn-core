use super::*;

/// `SklS11` 强化提示里的属性名，按语言包 `detail` 的槽位顺序拆分得到。
const WEAPON_S11_ATTR_NAMES: [&str; 7] = ["攻", "防", "速", "敏", "魔", "抗", "智"];

impl CombatRuntime {
    /// 0.5.5 `SklS11.v`：促销日购买武器，强化属性或掉落耐久，最后按耐久自伤。
    pub fn drain_plain_weapon_s11_skill_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        use crate::runtime::lang::keys;

        updates.add(crate::runtime::update::RunUpdate::new_lang(
            keys::WEAPON_S11_0,
            actor.0 as usize,
            actor.0 as usize,
            0,
        ));
        if self.rng.c25() {
            updates.add(crate::runtime::update::RunUpdate::new_lang(
                keys::WEAPON_S11_1,
                actor.0 as usize,
                actor.0 as usize,
                0,
            ));
            let s11 = self
                .entities
                .get_mut(actor)
                .and_then(|entity| entity.runtime.weapon_s11.as_mut())
                .unwrap_or_else(|| panic!("runtime weapon s11 state missing for entity {}", actor.0));
            s11.durability -= 1;
        } else {
            let attr_index = self.rng.next_i32(7) as usize;
            let increment = u32::from(self.rng.next_u8() & 31) + 6;
            self.raise_plain_weapon_s11_attr(actor, attr_index, increment);
            let mut update = crate::runtime::update::RunUpdate::new(
                format!(
                    "[{}]{}",
                    WEAPON_S11_ATTR_NAMES[attr_index],
                    crate::runtime::lang::get_lang(keys::WEAPON_S11_2)
                ),
                actor.0 as usize,
                actor.0 as usize,
                increment,
            );
            update.param = Some(increment);
            updates.add(update);
        }

        let durability = {
            let s11 = self
                .entities
                .get_mut(actor)
                .and_then(|entity| entity.runtime.weapon_s11.as_mut())
                .unwrap_or_else(|| panic!("runtime weapon s11 state missing for entity {}", actor.0));
            s11.durability -= self.rng.r3() as i32;
            s11.durability
        };
        self.entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("runtime weapon s11 owner disappeared: {}", actor.0))
            .runtime
            .move_state
            .speed_points += 1024;
        if durability > 0 {
            return;
        }

        updates.add(crate::runtime::update::RunUpdate::new_lang(
            keys::WEAPON_S11_DONE_1,
            actor.0 as usize,
            actor.0 as usize,
            0,
        ));
        let level = {
            let s11 = self
                .entities
                .get_mut(actor)
                .and_then(|entity| entity.runtime.weapon_s11.as_mut())
                .unwrap_or_else(|| panic!("runtime weapon s11 state missing for entity {}", actor.0));
            let level = s11.level;
            s11.level = if level < 20 { 0 } else { 1 };
            level
        };
        let done_key = if level < 20 {
            keys::WEAPON_S11_DONE_3
        } else {
            keys::WEAPON_S11_DONE_2
        };
        updates.add(crate::runtime::update::RunUpdate::new_lang(
            done_key,
            actor.0 as usize,
            actor.0 as usize,
            0,
        ));
        let amount = self.rng.r31() as i32 + 16;
        if self.apply_plain_legacy_damage_into(actor, actor, amount, updates) {
            self.drain_plain_lethal_damage_into(actor, actor, updates);
        }
    }

    /// 0.5.5 `SklDeathNote.v`：处决笔记目标，牺牲自身移动速度与法力后清空笔记。
    pub fn drain_plain_weapon_death_note_skill_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        use crate::runtime::lang::keys;

        let Some(mark) = self.entities.get(actor).and_then(|entity| entity.runtime.death_note_mark) else {
            return;
        };
        let mark_hp = self.entities.get(mark).map_or(0, |entity| entity.runtime.hp);
        updates.add(crate::runtime::update::RunUpdate::new_lang(
            keys::WEAPON_DEATH_NOTE_ATK,
            actor.0 as usize,
            mark.0 as usize,
            20,
        ));
        if self.apply_plain_legacy_damage_into(actor, mark, mark_hp, updates) {
            self.drain_plain_lethal_damage_into(actor, mark, updates);
        }
        let owner = self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("runtime death note owner disappeared: {}", actor.0));
        owner.template.speed -= 1024;
        if owner.runtime.magic_point > 0 {
            owner.runtime.magic_point = 0;
        }
        owner.runtime.death_note_mark = None;
        owner.refresh_runtime_stats_from_template();
    }

    /// 0.5.5 `RinickModifierPreAction.aN`：行动前清除自身负向状态。
    ///
    /// 解除消息只在体力大于 0 时输出，火伤状态没有解除文本；只要产生了解除消息，
    /// 「使用[属性修改器]」就会被插回本段更新开头，并在末尾补一个换行。
    /// 身份分支的隐身技能以更高优先级覆盖了强制分身返回值，因此这里不复刻该分支。
    pub fn run_plain_rinick_modifier_pre_action(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        use crate::runtime::lang::keys;

        let Some(entity) = self.entities.get_mut(actor) else {
            return;
        };
        if entity.template.rinick_revive_attrs.is_none() {
            return;
        }
        let mut had = [false; 6];
        let mut should_refresh_stats = false;
        let mut negative_keys = Vec::new();
        for entry in entity.states.entries() {
            let slot = match entry.payload {
                StatePayload::FireMagHalfSteps(_) => {
                    negative_keys.push(entry.legacy_order_key);
                    continue;
                }
                StatePayload::Berserk { .. } => 0,
                StatePayload::Charm { .. } => {
                    should_refresh_stats = true;
                    1
                }
                StatePayload::Curse { .. } => {
                    should_refresh_stats = true;
                    2
                }
                StatePayload::Ice { .. } => {
                    should_refresh_stats = true;
                    3
                }
                StatePayload::Poison { .. } => 4,
                StatePayload::Slow { .. } => {
                    should_refresh_stats = true;
                    5
                }
                _ => continue,
            };
            had[slot] = true;
            negative_keys.push(entry.legacy_order_key);
        }
        if negative_keys.is_empty() {
            return;
        }
        let insert_at = updates.updates.len();
        for legacy_order_key in negative_keys {
            assert!(
                entity.states.clear_legacy_key(legacy_order_key),
                "runtime negative state disappeared during rinick pre-action"
            );
        }
        if should_refresh_stats {
            entity.refresh_runtime_stats_from_template();
        }
        if !had.iter().any(|flag| *flag) {
            return;
        }
        if updates.capture_updates {
            updates.updates.insert(
                insert_at,
                crate::runtime::update::RunUpdate::new_lang(keys::WEAPON_R_MODIFIER_USE, actor.0 as usize, actor.0 as usize, 60),
            );
        }
        for (had_state, message) in [
            (had[0], "[1]从[狂暴]中解除"),
            (had[1], "[1]从[魅惑]中解除"),
            (had[2], "[1]从[诅咒]中解除"),
            (had[3], "[1]从[冰冻]中解除"),
            (had[4], "[1]从[中毒]中解除"),
            (had[5], "[1]从[迟缓]中解除"),
        ] {
            if had_state {
                updates.add_newline();
                updates.add(RuntimeFrame::replay_update(actor.0 as usize, actor.0 as usize, message, 0));
            }
        }
        updates.add_newline();
    }

    /// 0.5.5 属性修改器身份使用的隐身型 `SklMarioReraise`：死亡时消耗一次复活并重算八围。
    ///
    /// 顺序为 `bL`（体力为 0 时静默清除负向状态）→ `hy.dA`（从名字表重建 `q` 并回满）
    /// → 复活与生命数提示 → `hy.dd`（人数劣势时召唤分身）。这里不重跑武器 `cs()`，
    /// 体力上限取名字推导值、不再抬到 324；前七围由 `ar` 抬到 63 并把面板写成固定值。
    pub fn drain_plain_rinick_reraise_into(&mut self, target: EntityIdx, updates: &mut RunUpdates) -> bool {
        use crate::runtime::lang::keys;

        let Some(entity) = self.entities.get_mut(target) else {
            return false;
        };
        if entity.runtime.hp > 0 {
            return false;
        }
        let Some(raw) = entity.template.rinick_revive_attrs else {
            return false;
        };
        let remaining = entity.runtime.rinick_reraise_lives.unwrap_or(0).saturating_sub(1);
        entity.runtime.rinick_reraise_lives = Some(remaining);
        if remaining == 0 {
            return false;
        }
        let negative_keys: smallvec::SmallVec<[u32; 8]> = entity
            .states
            .entries()
            .iter()
            .filter(|entry| entry.is_negative_state())
            .map(|entry| entry.legacy_order_key)
            .collect();
        for legacy_order_key in negative_keys {
            entity.states.clear_legacy_key(legacy_order_key);
        }

        let factor = entity.template.clone_build.as_ref().map_or(0.0, |build| build.name_factor());
        let scale = |value: u32, divisor: f64| (f64::from(value) * (1.0 - factor / divisor)).round() as i32;
        let mut q = raw;
        let mut floored = [false; 7];
        for index in 0..7 {
            if q[index] < 63 {
                q[index] = 63;
                floored[index] = true;
            }
        }
        let panel = |index: usize, divisor: f64| {
            if floored[index] { 63 } else { scale(q[index], divisor) }
        };
        let wisdom = panel(6, 80.0);
        let stats = CloneDerivedStats {
            max_hp: q[7] as i32,
            attack: panel(0, 128.0),
            magic: panel(4, 128.0),
            wisdom,
            speed: if floored[2] { 223 } else { scale(q[2], 128.0) + 160 },
            defense: panel(1, 128.0),
            resistance: panel(5, 128.0),
            agility: panel(3, 128.0),
            at_boost_bits: entity.template.at_boost_bits,
            at_boost_millionths: entity.template.at_boost_millionths,
            // legacy 的 `ci()` 在 `ar` 抬升前计算总和，而 `ar` 会把 63 永久写回 `q`；
            // 这里直接按收敛后的值记录。
            attr_sum: q[..7].iter().sum(),
            atk_sum: (q[0] as i32 - q[1] as i32 + q[2] as i32 + q[4] as i32 - q[5] as i32) * 2 + q[3] as i32 + q[6] as i32,
            attract_bits: 32768.0_f64.to_bits(),
        };
        entity.apply_derived_stats(stats);
        entity.template.magic_point = wisdom >> 1;
        let max_hp = entity.template.max_hp;
        entity.refresh_runtime_stats_from_template();
        entity.runtime.hp = max_hp;
        entity.runtime.magic_point = wisdom >> 1;

        let mut revive_update =
            crate::runtime::update::RunUpdate::new_lang(keys::BOSS_MARIO_REVIVE, target.0 as usize, target.0 as usize, 0);
        revive_update.delay0 = 3000;
        updates.add(revive_update);
        let mut life_update =
            crate::runtime::update::RunUpdate::new_lang(keys::BOSS_MARIO_LIFE, target.0 as usize, target.0 as usize, 0);
        life_update.param = Some(remaining);
        updates.add(life_update);
        self.spawn_plain_rinick_clones_into(target, updates);
        true
    }

    /// 0.5.5 `SklRinickModifierClone.v`：人数劣势时召唤分身。
    ///
    /// 召唤数按 `((总存活 - 队伍存活) >> 1) - 队伍存活` 计算；本体与每个分身都会重新抽取
    /// 移动点（`rng * 4 + 1024` 直接赋值），随后依次输出换行与克隆提示。
    fn spawn_plain_rinick_clones_into(&mut self, owner: EntityIdx, updates: &mut RunUpdates) {
        use crate::runtime::lang::keys;

        let owner_team = self.plain_effective_team(owner);
        let mut total_alive = 0i64;
        let mut team_alive = 0i64;
        for (_, entity) in self.entities.iter() {
            if entity.runtime.hp <= 0 {
                continue;
            }
            total_alive += 1;
            if entity.runtime.team == owner_team {
                team_alive += 1;
            }
        }
        let count = ((total_alive - team_alive) >> 1) - team_alive;
        if count <= 0 {
            return;
        }

        let clone_kind = self
            .registry
            .player_kind_id_by_export_name(DEFAULT_CORE_CLONE_KIND_EXPORT)
            .expect("default runtime profile must register core clone kind");
        let counter_slot = self
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_MINION_COUNTER_ENTITY_EXPORT)
            .expect("default runtime profile must register core minion counter slot");

        let owner_move_points = self.rng.r255() as i32 * 4 + 1024;
        let (owner_name, owner_display_name, clone_build, clone_skills) = {
            let entity = self
                .entities
                .get_mut(owner)
                .unwrap_or_else(|| panic!("runtime rinick owner disappeared: {}", owner.0));
            entity.runtime.move_state.speed_points = owner_move_points;
            let clone_build = entity
                .template
                .clone_build
                .clone()
                .unwrap_or_else(|| panic!("runtime rinick owner missing clone build data: {}", owner.0));
            let clone_skills = entity
                .template
                .rinick_clone_skills
                .clone()
                .unwrap_or_else(|| panic!("runtime rinick owner missing clone skill snapshot: {}", owner.0));
            (
                entity.template.name.clone(),
                entity.template.display_name.clone(),
                clone_build,
                clone_skills,
            )
        };

        updates.add_newline();
        updates.add(crate::runtime::update::RunUpdate::new_lang(
            keys::WEAPON_R_MODIFIER_USE,
            owner.0 as usize,
            owner.0 as usize,
            60,
        ));

        for _ in 0..count {
            let clone_move_points = self.rng.r255() as i32 * 4 + 1024;
            let clone_build = clone_build.child();
            let stats = clone_build.derive_stats();
            let next_minion_index = match self
                .entities
                .get(owner)
                .unwrap_or_else(|| panic!("runtime rinick owner disappeared: {}", owner.0))
                .slots
                .get(counter_slot)
            {
                Some(SlotValue::U64(next)) => *next,
                Some(_) => panic!("runtime core minion counter slot has invalid value"),
                None => 0,
            };
            self.entities
                .get_mut(owner)
                .unwrap()
                .slots
                .set(counter_slot, SlotValue::U64(next_minion_index + 1))
                .expect("runtime core minion counter slot must exist");
            let next_entity = self.entities.len();
            let mut template = PlayerTemplate::with_kind(
                next_entity + 1,
                format!("{owner_name}?{next_minion_index}"),
                clone_kind,
                owner_team,
                stats.max_hp.max(1),
                stats.attack.max(0),
            )
            .with_display_name(owner_display_name.clone())
            .with_magic(stats.magic.max(0))
            .with_magic_point((stats.wisdom >> 1).max(0))
            .with_wisdom(stats.wisdom.max(0))
            .with_speed(stats.speed.max(0))
            .with_def_res(stats.defense.max(0), stats.resistance.max(0))
            .with_agility(stats.agility.max(0))
            .with_at_boost(f64::from_bits(stats.at_boost_bits).max(0.0))
            .with_target_score_stats(stats.attr_sum, stats.atk_sum, f64::from_bits(stats.attract_bits))
            .with_speed_points(clone_move_points)
            .with_skill_loadout(clone_skills.clone());
            template.clone_build = Some(clone_build);
            updates.add_newline();
            self.effects.push(QueuedEffect::SpawnWithMessage {
                caster: owner,
                template,
                message: crate::runtime::lang::get_lang(keys::SKL_CLONED).to_owned(),
            });
            self.drain_effects_into(updates);
        }
    }

    /// 0.5.5 `SklS11.v`：把强化点写回构造八围，再按名字系数重建面板。
    fn raise_plain_weapon_s11_attr(&mut self, actor: EntityIdx, attr_index: usize, increment: u32) {
        let owner = self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("runtime weapon s11 owner disappeared: {}", actor.0));
        if let Some(build) = owner.template.clone_build.as_mut() {
            build.attrs[attr_index] += increment;
            let stats = build.derive_stats();
            owner.apply_derived_stats(stats);
            return;
        }
        match attr_index {
            0 => owner.template.attack += increment as i32,
            1 => owner.template.defense += increment as i32,
            2 => owner.template.speed += increment as i32,
            3 => owner.template.agility += increment as i32,
            4 => owner.template.magic += increment as i32,
            5 => owner.template.resistance += increment as i32,
            6 => owner.template.wisdom += increment as i32,
            _ => unreachable!("weapon s11 attr index out of range: {attr_index}"),
        }
        owner.refresh_runtime_stats_from_template();
    }
}

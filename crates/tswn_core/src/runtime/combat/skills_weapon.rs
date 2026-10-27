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

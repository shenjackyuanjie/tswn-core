use super::*;

use crate::runtime::lang::{get_lang, keys};

/// 0.5.5 `BossSlime2.eV` 对名字底数的改写。
///
/// 第一代子体把名字头固定为 16 并给前 50 项或上 16；第二代固定为 -5 并或上 32。
/// legacy 的 `t` 是普通 JS 数字数组，负值会保留下来参与体力求和，因此这里用 `i32`。
fn slime_child_attrs(name_base: &[u8; 128], generation: u32) -> [u32; 8] {
    let mut base = [0i32; 128];
    for (slot, &value) in base.iter_mut().zip(name_base) {
        *slot = i32::from(value);
    }
    if generation == 1 {
        base[..10].fill(16);
        for value in &mut base[10..50] {
            *value |= 16;
        }
    } else {
        base[..10].fill(-5);
        for value in &mut base[10..50] {
            *value |= 32;
        }
    }

    let mut head = [0i32; 10];
    head.copy_from_slice(&base[..10]);
    head.sort_unstable();
    let mut attrs = [0u32; 8];
    for (attr, offset) in attrs[..7].iter_mut().zip((10..31).step_by(3)) {
        *attr = crate::namerena::median(base[offset], base[offset + 1], base[offset + 2])
            .try_into()
            .expect("史莱姆分裂属性必须非负");
    }
    attrs[7] = (154 + head[3] + head[4] + head[5] + head[6]).try_into().expect("史莱姆分裂体力必须为正");
    attrs
}

impl CombatRuntime {
    /// 0.5.5 `SklSlimeSpawn.b1`：史莱姆死亡时分裂成两只子体并保留尸体。
    ///
    /// 第一代子体还会再分裂一次；第二代只带等级 32 的半血与治疗技能。
    /// 子体是普通玩家而不是战斗召唤物：不随本体死亡清理，也不继承 Boss 状态免疫。
    pub fn drain_plain_slime_split_into(&mut self, target: EntityIdx, updates: &mut RunUpdates) -> bool {
        let Some(entity) = self.entities.get_mut(target) else {
            return false;
        };
        let Some(generation) = entity.runtime.slime_split_generation else {
            return false;
        };
        if generation >= 2 || entity.runtime.hp > 0 {
            return false;
        }
        // 分裂只能发生一次；继续留在死亡链里的是普通尸体。
        entity.runtime.slime_split_generation = None;
        entity.states.register_compressed_legacy_state(CompressedLegacyState::Corpse);
        entity.runtime.corpse = RuntimeCorpseKind::Slime;

        let (name, clan_name, team, root_owner) = {
            let entity = self.entities.get(target).unwrap();
            (
                entity.template.name.clone(),
                entity.template.clan_name.clone(),
                entity.runtime.team,
                entity.runtime.root_owner,
            )
        };
        let child_generation = generation + 1;

        updates.add_newline();
        updates.add(crate::runtime::update::RunUpdate::new_lang(
            keys::SKL_SLIME_SPAWN,
            target.0 as usize,
            target.0 as usize,
            0,
        ));

        let first = self.spawn_plain_slime_child(&name, &clan_name, team, root_owner, child_generation);
        let second = self.spawn_plain_slime_child(&name, &clan_name, team, root_owner, child_generation);

        updates.add(crate::runtime::update::RunUpdate::new_lang(
            keys::SKL_SLIME_SPAWNED,
            first.0 as usize,
            second.0 as usize,
            0,
        ));
        true
    }

    fn spawn_plain_slime_child(
        &mut self,
        name: &str,
        clan_name: &str,
        team: usize,
        root_owner: EntityIdx,
        generation: u32,
    ) -> EntityIdx {
        let (name_base, _) = PreparedBattleInit::plain_minion_name_base(clan_name, name);
        let attrs = slime_child_attrs(&name_base, generation);
        // legacy 会按名字系数缩放子体八围，但该系数属于构造期数据，运行期模板无法还原；
        // 与普通召唤物保持一致，按系数 0 直接换算。
        let status = PreparedBattleInit::plain_minion_status(attrs);
        let allocated = self.allocate_plain_minion_name(root_owner);
        let id_key_name = if clan_name.is_empty() {
            allocated
        } else {
            format!("{allocated}@{clan_name}")
        };
        let skills = if generation >= 2 {
            let half = self
                .registry
                .skill_id_by_export_name(BuiltinActiveSkill::Half.export_name())
                .expect("default runtime profile must register core half skill");
            let heal = self
                .registry
                .skill_id_by_export_name(BuiltinActiveSkill::Heal.export_name())
                .expect("default runtime profile must register core heal skill");
            SkillLoadout::from_skill_levels([(half, 32), (heal, 32)])
        } else {
            SkillLoadout::default()
        };
        let template = PlayerTemplate::new(0, name.to_owned(), team, status.max_hp, status.attack)
            .with_identity_names(id_key_name, clan_name.to_owned())
            .with_display_name(get_lang(keys::BOSS_NAME_SLIME))
            .with_magic(status.magic)
            .with_magic_point(status.magic_point)
            .with_wisdom(status.wisdom)
            .with_speed(status.speed)
            .with_def_res(status.defense, status.resistance)
            .with_agility(status.agility)
            .with_at_boost(status.at_boost)
            .with_target_score_stats(status.attr_sum, status.atk_sum, status.attract)
            .with_speed_points(self.rng.r255() as i32 * 4)
            .with_skill_loadout(skills);
        // 子体不属于任何战斗召唤物，owner 停在自身；root owner 仍指向原始史莱姆，
        // 供模型输入映射与以后可能的命名计数使用。
        let child = self
            .entities
            .spawn_from_template_with_owner(template, &self.registry, None, Some(root_owner));
        self.entities.get_mut(child).unwrap().runtime.slime_split_generation = Some(generation);
        let child_team = self.entities.get(child).unwrap().runtime.team;
        self.world.add_spawned_alive(child, child_team);
        child
    }
}

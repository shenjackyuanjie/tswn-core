//! 技能 hook 上下文：当前等级、受击值、owner 数据、RNG 和效果提交入口。
//!
//! handler 的 entry 与上下文使用同一 owner。PRE_DEFEND 中的值是浮点攻击威力，
//! POST_DEFEND 中的值是整数伤害；错误阶段使用 setter 会 panic。
//! 自身数据与部分内置修改有直接入口，跨实体读取和扩展槽按 capabilities 检查。

use super::*;
use crate::runtime::combat::PlainAttackOnDamage;

/// 一次技能 handler 调用内的借用视图，使用本场 Runtime 的随机流。
pub struct SkillContext<'a> {
    entities: &'a mut EntityArena,
    world: &'a mut WorldArena,
    registry: &'a ExtensionRegistry,
    template_slots: &'a TemplateSlotStorage,
    slots: &'a mut BattleSlotStorage,
    queue: &'a mut EffectQueue,
    updates: &'a mut RunUpdates,
    rng: &'a mut RC4,
    defend_value: Option<&'a mut RuntimeDefendValue>,
    defend_on_damage: PlainAttackOnDamage,
    selected_target: Option<EntityIdx>,
    owner: EntityIdx,
    capabilities: &'a [ExtensionCapability],
    force_passive: bool,
    skill_multiplier: f64,
}

impl<'a> SkillContext<'a> {
    /// 从 hook 计划条目构造上下文；`entry.owner` 即本上下文的 owner。
    pub fn new(
        entities: &'a mut EntityArena,
        world: &'a mut WorldArena,
        registry: &'a ExtensionRegistry,
        template_slots: &'a TemplateSlotStorage,
        slots: &'a mut BattleSlotStorage,
        queue: &'a mut EffectQueue,
        updates: &'a mut RunUpdates,
        rng: &'a mut RC4,
        entry: SkillHookPlanEntry,
        capabilities: &'a [ExtensionCapability],
    ) -> Self {
        Self {
            entities,
            world,
            registry,
            template_slots,
            slots,
            queue,
            updates,
            rng,
            defend_value: None,
            defend_on_damage: PlainAttackOnDamage::None,
            selected_target: None,
            owner: entry.owner,
            capabilities,
            force_passive: false,
            skill_multiplier: 1.0,
        }
    }

    /// 附加受击值视图，供 PRE_DEFEND / POST_DEFEND hook 读写。
    pub fn with_defend_value(mut self, defend_value: &'a mut RuntimeDefendValue) -> Self {
        self.defend_value = Some(defend_value);
        self
    }

    /// 附加本次普攻附带的伤害效果。
    pub fn with_defend_on_damage(mut self, on_damage: PlainAttackOnDamage) -> Self {
        self.defend_on_damage = on_damage;
        self
    }

    /// 记录本回合选中的目标。
    pub fn with_selected_target(mut self, selected_target: EntityIdx) -> Self {
        self.selected_target = Some(selected_target);
        self
    }

    /// 标记本次调用是否为强制被动（外部决策已决定发动）。
    pub fn with_force_passive(mut self, force_passive: bool) -> Self {
        self.force_passive = force_passive;
        self
    }

    /// 是否处于强制被动模式。
    pub fn force_passive(&self) -> bool { self.force_passive }

    /// 设置技能概率缩放倍率，影响 roll_automatic_skill。
    pub fn with_skill_multiplier(mut self, multiplier: f64) -> Self {
        self.skill_multiplier = multiplier;
        self
    }

    /// 按等级与范围掷自动技能概率，应用当前缩放倍率。
    pub fn roll_automatic_skill(&mut self, level: u32, range: u32) -> bool {
        crate::runtime::control::roll_scaled_skill(self.rng, level, range, self.skill_multiplier)
    }

    /// owner 的实体下标。
    pub fn owner_idx(&self) -> EntityIdx { self.owner }

    /// 本回合选中的目标（如有）。
    pub fn selected_target(&self) -> Option<EntityIdx> { self.selected_target }

    /// owner 实体记录；实体已不存在时返回 None。
    pub fn owner(&self) -> Option<&EntityRecord> { self.entities.get(self.owner) }

    /// 读取条目对应固定 lane 的技能等级；条目 owner 必须与上下文 owner 一致。
    pub fn skill_level(&self, entry: &SkillHookPlanEntry) -> u32 {
        assert_eq!(
            entry.owner, self.owner,
            "runtime skill hook entry owner must match skill context owner"
        );
        self.owner()
            .and_then(|entity| entity.template.skills.level_at(entry.fixed_lane))
            .unwrap_or_else(|| panic!("runtime skill level missing for fixed lane {}", entry.fixed_lane))
    }

    /// owner 的蓄力运行时状态。
    pub fn owner_charge_runtime(&self) -> Option<ChargeRuntime> { self.owner().map(|entity| entity.runtime.charge) }

    /// owner 的累积运行时状态。
    pub fn owner_accumulate_runtime(&self) -> Option<AccumulateRuntime> { self.owner().map(|entity| entity.runtime.accumulate) }

    /// owner 当前护盾值。
    pub fn owner_shield(&self) -> Option<i32> { self.owner().map(|entity| entity.runtime.shield) }

    /// 设置 owner 护盾；正值会登记护盾状态，负值按 0 处理。
    pub fn set_owner_shield(&mut self, shield: i32) -> Result<(), EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        if shield > 0 {
            owner.states.register_compressed_legacy_state(CompressedLegacyState::Shield);
        }
        owner.runtime.shield = shield.max(0);
        Ok(())
    }

    /// 激活 owner 的蓄力状态。
    pub fn activate_owner_charge_runtime(&mut self) -> Result<(), EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        owner.activate_charge_runtime();
        Ok(())
    }

    /// 行动后推进蓄力计数，返回蓄力是否仍在生效。
    pub fn tick_owner_charge_post_action(&mut self) -> Result<bool, EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        Ok(owner.tick_charge_post_action())
    }

    /// 清除蓄力，返回清除前蓄力是否生效。
    pub fn clear_owner_charge_runtime(&mut self) -> Result<bool, EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        Ok(owner.clear_charge_runtime())
    }

    /// 激活 owner 的累积状态，返回激活前是否已激活。
    pub fn activate_owner_accumulate_runtime(&mut self) -> Result<bool, EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        Ok(owner.activate_accumulate_runtime())
    }

    /// 清除累积，返回清除前累积是否生效。
    pub fn clear_owner_accumulate_runtime(&mut self) -> Result<bool, EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        Ok(owner.clear_accumulate_runtime())
    }

    /// 清除 owner 的正面运行时消息，返回 (顺序键, 文案键) 列表。
    pub fn clear_owner_positive_runtime_messages(&mut self) -> Result<Vec<(i32, &'static str)>, EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        Ok(owner.clear_positive_runtime_messages())
    }

    /// 清除 owner 的正面状态消息，返回 (顺序键, 文案键) 列表。
    pub fn clear_owner_positive_state_messages(&mut self) -> Result<Vec<(i32, &'static str)>, EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        Ok(owner.clear_positive_state_messages())
    }

    /// 清除 owner 的全部正面消息，返回 (顺序键, 文案键) 列表。
    pub fn clear_owner_positive_messages(&mut self) -> Result<Vec<(i32, &'static str)>, EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        Ok(owner.clear_positive_messages())
    }

    /// 在场实体总数。
    pub fn entity_count(&self) -> usize { self.entities.len() }

    /// 读取任意实体；读取 owner 以外的实体时按队友/敌方检查能力。
    pub fn entity(&self, entity: EntityIdx) -> Result<&EntityRecord, EffectContextError> {
        let observed = self.entities.get(entity).ok_or(EffectContextError::UnknownEntity(entity))?;
        if entity == self.owner {
            return Ok(observed);
        }

        let owner = self.entities.get(self.owner).ok_or(EffectContextError::UnknownEntity(self.owner))?;
        let capability = if observed.runtime.team == owner.runtime.team {
            ExtensionCapability::ReadAllies
        } else {
            ExtensionCapability::ReadEnemies
        };
        self.require(capability)?;
        Ok(observed)
    }

    /// 读取对局级槽；需要 ReadBattleSlots 能力。
    pub fn battle_slot(&self, id: crate::runtime::BattleSlotId) -> Result<Option<&SlotValue>, EffectContextError> {
        self.require(ExtensionCapability::ReadBattleSlots)?;
        Ok(self.slots.get(id))
    }

    /// 读取模板级槽；需要 ReadTemplateSlots 能力。
    pub fn template_slot(&self, id: TemplateSlotId) -> Result<Option<&SlotValue>, EffectContextError> {
        self.require(ExtensionCapability::ReadTemplateSlots)?;
        Ok(self.template_slots.get(id))
    }

    /// 写入实体槽；需要 MutateEntitySlots 能力。
    pub fn set_entity_slot(&mut self, entity: EntityIdx, slot: EntitySlotId, value: SlotValue) -> Result<(), EffectContextError> {
        self.require(ExtensionCapability::MutateEntitySlots)?;
        let Some(entity) = self.entities.get_mut(entity) else {
            return Err(EffectContextError::UnknownEntity(entity));
        };
        entity.slots.set(slot, value)?;
        Ok(())
    }

    /// 入队一个优先执行的效果。
    pub fn push_nested(&mut self, effect: QueuedEffect) { self.queue.push_nested(effect); }

    /// 追加一条展示更新。
    pub fn add_update(&mut self, update: RunUpdate) { self.updates.add(update); }

    /// 追加一条普通防御消息。
    pub fn add_plain_defense_with(&mut self, caster: usize, target: usize) {
        self.updates.add_plain_defense_with(caster, target);
    }

    /// 追加换行分隔。
    pub fn add_newline(&mut self) { self.updates.add_newline(); }

    /// 最近一条非换行更新，用于在既有消息上续写。
    pub fn last_non_newline_update(&self) -> Option<&RunUpdate> { self.updates.last_non_newline_update() }

    /// 取一个随机字节。
    pub fn rng_next_u8(&mut self) -> u8 { self.rng.next_u8() }

    /// 取 [0, max) 内的随机整数。
    pub fn rng_next_i32(&mut self, max: i32) -> i32 { self.rng.next_i32(max) }

    /// 各 50% 概率的真值。
    pub fn rng_c50(&mut self) -> bool { self.rng.c50() }

    /// 取 [0, 255] 内的随机值。
    pub fn rng_r255(&mut self) -> u32 { self.rng.r255() }

    /// 取 [0, 127] 内的随机值。
    pub fn rng_r127(&mut self) -> u32 { self.rng.r127() }

    /// 取 [0, 63] 内的随机值。
    pub fn rng_r63(&mut self) -> u32 { self.rng.r63() }

    /// 取 [0, 16] 内的随机值。
    pub fn rng_r16(&mut self) -> u32 { self.rng.r16() }

    /// 复活 owner：技能等级减半，体力置为给定值（至少 1）并恢复存活。
    pub fn reraise_owner(&mut self, entry: &SkillHookPlanEntry, hp: i32) -> Result<(), EffectContextError> {
        assert_eq!(
            entry.owner, self.owner,
            "runtime reraise hook entry owner must match skill context owner"
        );
        let owner = self.entities.get_mut(self.owner).ok_or(EffectContextError::UnknownEntity(self.owner))?;
        let current_level = owner
            .template
            .skills
            .level_at(entry.fixed_lane)
            .unwrap_or_else(|| panic!("runtime reraise level missing for fixed lane {}", entry.fixed_lane));
        assert!(
            owner.template.skills.set_level_at(entry.fixed_lane, current_level.div_ceil(2)),
            "runtime reraise fixed lane disappeared: {}",
            entry.fixed_lane
        );
        owner.runtime.hp = hp.max(1);
        owner.runtime.alive = true;
        Ok(())
    }

    /// 尝试支付 owner 的 MP：未激活或不足返回 false，成功则扣除并返回 true。
    pub fn owner_mp_ready(&mut self) -> Result<bool, EffectContextError> {
        let active = self
            .entities
            .get(self.owner)
            .ok_or(EffectContextError::UnknownEntity(self.owner))
            .map(EntityRecord::is_active)?;
        if !active {
            return Ok(false);
        }
        let required_mp = self.rng.r3x3() as i32;
        // 能力卡减耗只作用于当前技能的作用域，不改变掷骰消费。
        #[cfg(feature = "cards")]
        let required_mp = {
            let root = self
                .entities
                .get(self.owner)
                .ok_or(EffectContextError::UnknownEntity(self.owner))?
                .runtime
                .root_owner;
            (required_mp - crate::runtime::forced_action::mp_reduction(root.0)).max(0)
        };
        let owner = self.entities.get_mut(self.owner).ok_or(EffectContextError::UnknownEntity(self.owner))?;
        if owner.runtime.magic_point < required_mp {
            return Ok(false);
        }
        owner.runtime.magic_point -= required_mp;
        Ok(true)
    }

    /// 只读取独立能力乘区，不触碰技能的等级与锻造记录。
    #[cfg(feature = "cards")]
    pub fn ability_multiplier(&self) -> f64 {
        let root = self.entities.get(self.owner).map_or(self.owner, |e| e.runtime.root_owner);
        crate::runtime::forced_action::attack_multiplier(root.0)
    }

    /// 取 owner 的攻击威力（物理或魔法），消费一次随机取样。
    pub fn owner_attack_power(&mut self, use_magic: bool) -> Result<f64, EffectContextError> {
        let owner = self.entities.get(self.owner).ok_or(EffectContextError::UnknownEntity(self.owner))?;
        Ok(owner.runtime.get_at(use_magic, self.rng))
    }

    /// 受击值中的攻击者当前是否活跃。
    pub fn defend_caster_active(&self) -> Result<bool, EffectContextError> {
        let caster = self.defend_caster().ok_or(EffectContextError::UnknownEntity(self.owner))?;
        let caster = self.entities.get(caster).ok_or(EffectContextError::UnknownEntity(caster))?;
        Ok(caster.runtime.active())
    }

    /// 按等级重选 owner 的保护目标并维护双向链接；需要 ReadAllies 能力。
    pub fn refresh_owner_protect_target(
        &mut self,
        level: u32,
        forced_target: Option<EntityIdx>,
    ) -> Result<Option<EntityIdx>, EffectContextError> {
        self.require(ExtensionCapability::ReadAllies)?;
        let owner = self.entities.get(self.owner).ok_or(EffectContextError::UnknownEntity(self.owner))?;
        let wisdom = owner.runtime.wisdom.max(0) as u32;
        let effective_team = owner
            .states
            .entries()
            .iter()
            .find_map(|entry| match &entry.payload {
                StatePayload::Charm {
                    group_id,
                    effective_team_idx,
                    ..
                } => (*effective_team_idx).or_else(|| {
                    u32::try_from(*group_id)
                        .ok()
                        .and_then(|group_entity| self.entities.get(EntityIdx(group_entity)))
                        .map(|entity| entity.runtime.team)
                }),
                _ => None,
            })
            .unwrap_or(owner.runtime.team);
        let candidates = self
            .world
            .team_alive(effective_team)
            .unwrap_or_default()
            .iter()
            .copied()
            // legacy 的 team.alive 只看 alive 标志。自爆会先把使魔 HP 设为 0，
            // 等目标伤害链完成后才进入 on_die；这段窗口里使魔仍须作为无效候选
            // 参与 Protect 的抽样和重试，以保持 RC4 消耗一致。
            .filter(|candidate| self.entities.get(*candidate).is_some_and(|entity| entity.runtime.alive))
            .collect::<Vec<_>>();

        // legacy Protect 会先消耗 smart 判定，再处理有效友军列表为空的情况。
        let next_target = if let Some(forced_target) = forced_target {
            Some(forced_target)
        } else {
            let smart = self.rng.r127() < wisdom;
            if candidates.is_empty() {
                None
            } else {
                let owner_pos = candidates.iter().position(|candidate| *candidate == self.owner);
                let select_count = if smart { 3 } else { 2 };
                let mut selected = Vec::with_capacity(select_count);
                let mut dup = 0usize;
                let mut invalid = -(select_count as i32);
                while dup <= select_count && invalid <= select_count as i32 {
                    let picked = if let Some(owner_pos) = owner_pos {
                        self.rng.pick_skip(&candidates, owner_pos)
                    } else {
                        self.rng.pick(&candidates)
                    };
                    let Some(picked) = picked else {
                        break;
                    };
                    let target = candidates[picked];
                    let valid = self.entities.get(target).is_some_and(|entity| !entity.runtime.is_combat_minion());
                    if !valid {
                        invalid += 1;
                        continue;
                    }
                    if selected.contains(&target) {
                        dup += 1;
                        continue;
                    }
                    selected.push(target);
                    if selected.len() >= select_count {
                        break;
                    }
                }
                let mut scored = selected
                    .into_iter()
                    .map(|target| {
                        let entity = self
                            .entities
                            .get(target)
                            .unwrap_or_else(|| panic!("runtime protect candidate disappeared: {}", target.0));
                        let score = if smart {
                            let hp = entity.runtime.hp;
                            let rate_hi_hp = if hp < 20 {
                                30.0
                            } else if hp > 300 {
                                300.0
                            } else {
                                hp as f64
                            };
                            (1.0 / rate_hi_hp) * entity.runtime.atk_sum as f64 / (entity.runtime.protect_from.len() + 1) as f64
                        } else {
                            self.rng.rFFFF() as f64
                        };
                        (target, score)
                    })
                    .collect::<Vec<_>>();
                scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
                scored.first().map(|(target, _)| *target)
            }
        };

        let old_target = self
            .entities
            .get(self.owner)
            .ok_or(EffectContextError::UnknownEntity(self.owner))?
            .runtime
            .protect_to;
        if old_target != next_target {
            if let Some(old_target) = old_target
                && let Some(target) = self.entities.get_mut(old_target)
            {
                target.runtime.protect_from.retain(|link| link.owner != self.owner);
                if target.runtime.protect_from.is_empty() {
                    target.runtime.protect_pre_defend_skill_count = None;
                    target.states.clear_compressed_legacy_state(CompressedLegacyState::Protect);
                }
            }
            self.entities
                .get_mut(self.owner)
                .ok_or(EffectContextError::UnknownEntity(self.owner))?
                .runtime
                .protect_to = next_target;
        }
        if let Some(next_target) = next_target {
            let pre_defend_skill_count = {
                let target = self.entities.get(next_target).ok_or(EffectContextError::UnknownEntity(next_target))?;
                target
                    .template
                    .skills
                    .active_order()
                    .iter()
                    .filter(|fixed_lane| {
                        target.template.skills.level_at(**fixed_lane).is_some_and(|level| level > 0)
                            && target
                                .template
                                .skills
                                .skills()
                                .get(**fixed_lane)
                                .and_then(|skill_id| self.registry.skill(*skill_id))
                                .is_some_and(|skill| skill.hook_mask.intersects(ProcMask::PRE_DEFEND))
                    })
                    .count()
            };
            let target = self.entities.get_mut(next_target).ok_or(EffectContextError::UnknownEntity(next_target))?;
            if target.runtime.protect_from.is_empty() {
                target.runtime.protect_pre_defend_skill_count = Some(pre_defend_skill_count);
                target.states.register_compressed_legacy_state(CompressedLegacyState::Protect);
            }
            if let Some(link) = target.runtime.protect_from.iter_mut().find(|link| link.owner == self.owner) {
                link.level = level;
            } else {
                target.runtime.protect_from.push(ProtectLinkRuntime {
                    owner: self.owner,
                    level,
                });
            }
        }
        Ok(next_target)
    }

    /// 同步胜负视图，返回已产生的胜者队伍（如有）。
    pub fn sync_winner(&mut self) -> Option<usize> { self.world.sync_winner(self.entities) }

    /// 受击值中的攻击威力（PRE_DEFEND 阶段为浮点 ATP）。
    pub fn defend_atp(&self) -> Option<f64> { self.defend_value.as_ref().and_then(|value| value.atp()) }

    /// 受击值是否标记为魔法攻击。
    pub fn defend_is_magic(&self) -> Option<bool> { self.defend_value.as_ref().and_then(|value| value.is_magic()) }

    /// 受击值携带的伤害类型。
    pub fn defend_kind(&self) -> Option<DamageKind> { self.defend_value.as_ref().map(|value| value.kind()) }

    /// 受击值附带的普攻伤害效果。
    pub fn defend_on_damage(&self) -> PlainAttackOnDamage { self.defend_on_damage }

    /// 改写攻击威力；仅在 PRE_DEFEND 阶段可用，其他阶段调用会 panic。
    pub fn set_defend_atp(&mut self, atp: f64) {
        let Some(value) = self.defend_value.as_deref_mut() else {
            panic!("runtime defend atp is only available during PRE_DEFEND hooks");
        };
        value.set_atp(atp);
    }

    /// 受击值中的整数伤害（POST_DEFEND 阶段）。
    pub fn defend_damage(&self) -> Option<i32> { self.defend_value.as_ref().and_then(|value| value.damage()) }

    /// 改写整数伤害；仅在 POST_DEFEND 阶段可用，其他阶段调用会 panic。
    pub fn set_defend_damage(&mut self, damage: i32) {
        let Some(value) = self.defend_value.as_deref_mut() else {
            panic!("runtime defend damage is only available during POST_DEFEND hooks");
        };
        value.set_damage(damage);
    }

    /// 受击值中的攻击者下标。
    pub fn defend_caster(&self) -> Option<EntityIdx> { self.defend_value.as_ref().map(|value| value.caster()) }

    /// 受击值中的目标下标。
    pub fn defend_target(&self) -> Option<EntityIdx> { self.defend_value.as_ref().map(|value| value.target()) }

    /// 克隆读取 owner 指定顺序键的状态载荷；状态不存在返回 None。
    pub fn owner_state_payload(&self, legacy_order_key: u32) -> Option<StatePayload> {
        self.owner()?.states.entry(legacy_order_key).map(|entry| entry.payload.clone())
    }

    /// 写入 owner 的状态载荷并刷新派生数值；顺序键不存在返回错误。
    pub fn set_owner_state_payload(&mut self, legacy_order_key: u32, payload: StatePayload) -> Result<(), EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        if !owner.states.set_payload(legacy_order_key, payload) {
            return Err(EffectContextError::UnknownEntity(self.owner));
        }
        owner.refresh_runtime_stats_from_template();
        Ok(())
    }

    fn require(&self, capability: ExtensionCapability) -> Result<(), EffectContextError> {
        if self.capabilities.contains(&capability) {
            Ok(())
        } else {
            Err(EffectContextError::MissingCapability(capability))
        }
    }
}

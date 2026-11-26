use super::super::*;

impl CombatRuntime {
    /// 瘟疫（half）选目标：聪明时只考虑体力在 160 到 400 之间的敌人。
    pub fn select_plain_half_targets(&mut self, actor: EntityIdx, smart: bool) -> PreparedTargetList {
        self.select_plain_filtered_enemy_targets(
            actor,
            smart,
            |target, smart| !smart || target.is_some_and(|entity| entity.runtime.hp > 160 && entity.runtime.hp < 400),
            Self::score_plain_half_target,
        )
    }

    /// 瘟疫选目标评分：基础分乘以目标体力，体力越高越优先。
    pub fn score_plain_half_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime half target: {}", target.0));
        let rate_hi_hp = |hp: i32| -> f64 {
            if hp < 20 {
                30.0
            } else if hp > 300 {
                300.0
            } else {
                hp as f64
            }
        };
        let base = if smart {
            if self.world.alive_group_count() > 2 {
                rate_hi_hp(entity.runtime.hp) * self.world.alive_group_len_containing(target) as f64 * entity.runtime.attract()
            } else {
                rate_hi_hp(entity.runtime.hp) * entity.runtime.attr_sum as f64 * entity.runtime.attract()
            }
        } else {
            self.rng.rFFFF() as f64 + entity.runtime.attract()
        };
        base * entity.runtime.hp as f64
    }

    /// 诅咒选目标：聪明时要求体力不低于 80，且未携带高概率诅咒。
    pub fn select_plain_curse_targets(&mut self, actor: EntityIdx, smart: bool) -> PreparedTargetList {
        self.select_plain_filtered_enemy_targets(
            actor,
            smart,
            |target, smart| {
                !smart
                    || target.is_some_and(|entity| {
                        entity.runtime.hp >= 80
                            && entity
                                .states
                                .entries()
                                .iter()
                                .find_map(|entry| match entry.payload {
                                    StatePayload::Curse { prob, .. } => Some(prob),
                                    _ => None,
                                })
                                .is_none_or(|prob| prob <= 32)
                    })
            },
            Self::score_plain_curse_target,
        )
    }

    /// 诅咒选目标评分：已中诅咒的目标评分减半。
    pub fn score_plain_curse_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime curse target: {}", target.0));
        let rate_hi_hp = |hp: i32| -> f64 {
            if hp < 20 {
                30.0
            } else if hp > 300 {
                300.0
            } else {
                hp as f64
            }
        };
        let base = if smart {
            if self.world.alive_group_count() > 2 {
                rate_hi_hp(entity.runtime.hp) * self.world.alive_group_len_containing(target) as f64 * entity.runtime.attract()
            } else {
                (1.0 / rate_hi_hp(entity.runtime.hp)) * entity.runtime.atk_sum as f64 * entity.runtime.attract()
            }
        } else {
            self.rng.rFFFF() as f64 + entity.runtime.attract()
        };
        if entity
            .states
            .entries()
            .iter()
            .any(|entry| matches!(entry.payload, StatePayload::Curse { .. }))
        {
            base / 2.0
        } else {
            base
        }
    }

    /// 诅咒技能：取魔法威力攻击并附加诅咒伤害效果。
    pub fn drain_plain_curse_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime curse actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng);
        updates.add(crate::runtime::update::RunUpdate::new(
            "[0]使用[诅咒]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        self.drain_plain_attack_with_atp_and_on_damage_into(
            actor,
            target,
            true,
            atp,
            DamageKind::Default,
            PlainAttackOnDamage::Curse,
            updates,
        );
    }
}

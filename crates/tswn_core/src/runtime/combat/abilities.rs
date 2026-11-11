//! 能力卡：按根角色的队伍查询卡牌减耗与倍率，并为技能与被动建立独立作用域。
//!
//! 整个模块只随 cards feature 编译；关闭时 CombatRuntime 上不存在这些方法，
//! 各调用点也一并被 cfg 去掉，热路径与普通引擎对局完全一致。
use super::*;
impl CombatRuntime {
    // 卡牌玩家及其派生单位即使经吞噬提高原始幻术，也只按 5 计算；普通引擎对局不变。
    pub(crate) fn card_skill_level(&self, actor: EntityIdx, skill: BuiltinActiveSkill, level: u32) -> u32 {
        let root = self.entities.get(actor).and_then(|entity| self.entities.get(entity.runtime.root_owner));
        if skill == BuiltinActiveSkill::Shadow
            && root.is_some_and(|entity| matches!(crate::runtime::forced_action::controlled_team(), Some(team) if team == entity.template.team || team == u32::MAX as usize))
        {
            level.min(5)
        } else {
            level
        }
    }
    // 根角色的原始队伍决定能力归属，敌方与被魅惑角色不会借用玩家能力。
    pub(crate) fn ability_value(&self, actor: EntityIdx, skill: &str) -> (u32, f64) {
        let Some(entity) = self.entities.get(actor) else {
            return (0, 1.0);
        };
        let root = self.entities.get(entity.runtime.root_owner).unwrap_or(entity);
        crate::runtime::forced_action::ability_value(root.template.team, skill)
    }
    pub(crate) fn ability_scope(&self, actor: EntityIdx, skill: &str) -> crate::runtime::forced_action::AbilityScope {
        let (cost, power) = self.ability_value(actor, skill);
        let root = self.entities.get(actor).map_or(actor, |e| e.runtime.root_owner);
        crate::runtime::forced_action::scope_ability(root.0, power, cost)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::forced_action::{self, ForcedState};

    // 模拟吞噬后的高原始值，概率与熟练度 5 使用完全相同的随机流，敌方不受限。
    #[test]
    fn controlled_shadow_probability_uses_five_without_trimming_raw_values() {
        let mut high = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
            PlayerTemplate::new(1, "player", 0, 1000, 100),
            PlayerTemplate::new(2, "enemy", 1, 1000, 100),
        ]));
        let mut low = high.clone();
        let mut state = ForcedState::default();
        state.controlled_team = Some(0);
        forced_action::with_state(&mut state, || {
            for _ in 0..200 {
                assert_eq!(
                    high.plain_action_skill_probability(EntityIdx(0), BuiltinActiveSkill::Shadow, 200, false),
                    low.plain_action_skill_probability(EntityIdx(0), BuiltinActiveSkill::Shadow, 5, false)
                );
            }
            assert_eq!(high.card_skill_level(EntityIdx(1), BuiltinActiveSkill::Shadow, 88), 88);
        });
        assert_eq!(high.card_skill_level(EntityIdx(0), BuiltinActiveSkill::Shadow, 200), 200);
    }

    // 比较同一随机流，确保倍率不额外掷骰、不泄漏到敌方或下一次普攻。
    #[test]
    fn independent_multiplier_preserves_rng_and_restores_nested_scope() {
        let runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
            PlayerTemplate::new(1, "player", 0, 1000, 100).with_magic(90),
            PlayerTemplate::new(2, "enemy", 1, 1000, 100).with_magic(90),
        ]));
        let mut state = ForcedState::default();
        state.controlled_team = Some(0);
        state.abilities = forced_action::decode_abilities(&[0, 2, 10800]).unwrap();
        forced_action::with_state(&mut state, || {
            let player = &runtime.entities.get(EntityIdx(0)).unwrap().runtime;
            let mut rng = RC4::default();
            let mut expected = rng.clone();
            let raw = player.get_at(true, &mut expected);
            {
                let _outer = runtime.ability_scope(EntityIdx(0), "core.skill.fire");
                assert_eq!(player.get_at(true, &mut rng), raw * 1.08);
                assert_eq!(rng.main_val, expected.main_val);
                assert_eq!((rng.i, rng.j), (expected.i, expected.j));
                assert_eq!(forced_action::mp_reduction(0), 2);
                {
                    let _enemy = runtime.ability_scope(EntityIdx(1), "core.skill.fire");
                    assert_eq!(forced_action::attack_multiplier(1), 1.0);
                    assert_eq!(forced_action::mp_reduction(1), 0);
                }
                assert_eq!(forced_action::attack_multiplier(0), 1.08);
            }
            assert_eq!(forced_action::attack_multiplier(0), 1.0);
            assert_eq!(forced_action::mp_reduction(0), 0);
        });
        assert_eq!(forced_action::ability_value(0, "core.skill.fire"), (0, 1.0));
    }

    // 被动护盾使用额外乘区，技能等级和未受控队伍的护盾增长保持不变。
    #[test]
    fn passive_shield_boost_does_not_write_proficiency() {
        let mut builder = ExtensionRegistryBuilder::default();
        let shield = builder
            .register_skill_with_hooks(
                "core",
                "shield",
                "core.skill.shield",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .unwrap();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "player", 0, 1000, 100)
                    .with_skill_loadout(SkillLoadout::from_skill_levels([(shield, 106)])),
            ],
            builder.build(),
        ));
        runtime.set_skill_handler(shield, run_shield_pre_action_skill);
        let mut plain = runtime.clone();
        plain.run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION);
        let raw = plain.entities.get(EntityIdx(0)).unwrap().runtime.shield;
        let mut state = ForcedState::default();
        state.controlled_team = Some(0);
        state.abilities = forced_action::decode_abilities(&[29, 0, 11000]).unwrap();
        forced_action::with_state(&mut state, || {
            runtime.run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION);
        });
        let player = runtime.entities.get(EntityIdx(0)).unwrap();
        assert_eq!(player.runtime.shield, (raw as f64 * 1.10).round() as i32);
        assert_eq!(player.template.skills.level_at(0), Some(106));
    }
}

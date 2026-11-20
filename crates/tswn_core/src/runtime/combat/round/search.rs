use super::*;
use crate::runtime::search::SearchCandidate;

impl CombatRuntime {
    pub(super) fn prepare_search_action(&mut self, actor: EntityIdx, smart: bool) -> Option<PreparedPlainAction> {
        let runtime = &self.entities.get(actor)?.runtime;
        // 第一版保留 Boss、cards、等效熟练度的原版路径。
        if cfg!(feature = "cards")
            || runtime.flags.contains(PlayerKindFlags::BOSS)
            || self.entities.get(actor)?.template.skills.uses_effective_skill_weights()
        {
            self.scratch.search_turn = None;
            return self.prepare_plain_action(actor, smart, PlainSkillPreActionOutcome::default());
        }
        let (mp, attack, magic, team) = (runtime.magic_point, runtime.attack, runtime.magic, runtime.team);
        let cost = i32::from(self.rng.next_u8() & 15) + 8;
        let mut candidates = Vec::new();
        for &target in self.world.flat_alive() {
            if self.entities.get(target)?.runtime.team == team {
                continue;
            }
            candidates.push(SearchCandidate {
                action: PreparedPlainAction::BasicAttack {
                    target,
                    use_magic: false,
                    amount: attack,
                },
                cost: 0,
            });
            let magic_cost = (magic - attack) >> 2;
            if magic > attack && mp >= magic_cost {
                candidates.push(SearchCandidate {
                    action: PreparedPlainAction::BasicAttack {
                        target,
                        use_magic: true,
                        amount: magic,
                    },
                    cost: magic_cost,
                });
            }
        }
        if mp >= cost {
            let skills = &self.entities.get(actor)?.template.skills;
            let entries: Vec<_> = skills
                .active_order()
                .iter()
                .filter_map(|&lane| Some((lane, self.builtin_active_skill(skills.skills()[lane])?, skills.level_at(lane)?)))
                .collect();
            for (fixed_lane, skill, level) in entries {
                if level == 0 || !self.plain_action_skill_probability(actor, skill, level, false) {
                    continue;
                }
                let selected = SelectedBuiltinSkill { skill, fixed_lane };
                // 常见单体攻击和治疗枚举全场目标，不再随机抽两个候选。
                if matches!(
                    skill,
                    BuiltinActiveSkill::Fire
                        | BuiltinActiveSkill::Thunder
                        | BuiltinActiveSkill::Absorb
                        | BuiltinActiveSkill::Poison
                        | BuiltinActiveSkill::Critical
                        | BuiltinActiveSkill::Heal
                ) {
                    for &target in self.world.flat_alive() {
                        let entity = self.entities.get(target)?;
                        let allowed = if skill == BuiltinActiveSkill::Heal {
                            entity.runtime.team == team && entity.runtime.hp < entity.template.max_hp
                        } else {
                            entity.runtime.team != team
                        };
                        if allowed {
                            candidates.push(SearchCandidate {
                                action: PreparedPlainAction::BuiltinSkill(PreparedBuiltinSkillAction {
                                    selected,
                                    targets: smallvec::smallvec![target],
                                }),
                                cost,
                            });
                        }
                    }
                    continue;
                }
                if let Some(action) = self.prepare_plain_builtin_skill_action(actor, smart, selected) {
                    candidates.push(SearchCandidate {
                        action: PreparedPlainAction::BuiltinSkill(action),
                        cost,
                    });
                }
            }
        }
        let turn = self.scratch.search_turn.as_mut()?;
        turn.actor = Some(actor);
        turn.candidates = candidates;
        let selected = turn.chosen.and_then(|index| turn.candidates.get(index)).cloned();
        if let Some(seed) = turn.simulation_seed {
            self.rng = RC4::new(&seed.to_le_bytes(), 1);
        }
        if let Some(selected) = selected {
            self.entities.get_mut(actor)?.runtime.magic_point -= selected.cost;
            return Some(selected.action);
        }
        None
    }
}

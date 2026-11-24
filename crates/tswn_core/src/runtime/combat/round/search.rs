use super::*;
use crate::runtime::search::SearchCandidate;

impl CombatRuntime {
    pub(super) fn prepare_search_action(&mut self, actor: EntityIdx, smart: bool) -> Option<PreparedPlainAction> {
        let runtime = &self.entities.get(actor)?.runtime;
        // 特殊技能权重和 Boss 继续使用原版路径，不改变其抽样语义。
        if cfg!(feature = "cards")
            || runtime.flags.contains(PlayerKindFlags::BOSS)
            || self.entities.get(actor)?.template.skills.uses_effective_skill_weights()
        {
            self.scratch.search_turn = None;
            return self.prepare_plain_action(actor, smart, PlainSkillPreActionOutcome::default());
        }
        let (mp, attack, magic) = (runtime.magic_point, runtime.attack, runtime.magic);
        let team = self.plain_effective_team(actor);
        let cost = i32::from(self.rng.next_u8() & 15) + 8;
        let mut candidates = Vec::new();
        for &target in self.world.flat_alive() {
            if self.is_quilt(target) || self.entities.get(target)?.runtime.team == team {
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
                // 每个技能只抽一次资格；枚举目标不额外重掷资格或 MP 消耗。
                if level == 0 || !self.plain_action_skill_probability(actor, skill, level, false) {
                    continue;
                }
                let selected = SelectedBuiltinSkill { skill, fixed_lane };
                for targets in self.search_skill_targets(actor, skill, smart) {
                    candidates.push(SearchCandidate {
                        action: PreparedPlainAction::BuiltinSkill(PreparedBuiltinSkillAction { selected, targets }),
                        cost,
                    });
                }
            }
        }
        let turn = self.scratch.search_turn.as_mut()?;
        turn.actor = Some(actor);
        turn.team = Some(team);
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

    /// 枚举全部合法单体目标；群攻覆盖全部首目标，后续顺序有界，不穷举排列。
    pub(crate) fn search_skill_targets(
        &mut self,
        actor: EntityIdx,
        skill: BuiltinActiveSkill,
        smart: bool,
    ) -> Vec<PreparedTargetList> {
        use BuiltinActiveSkill::*;
        let team = self.plain_effective_team(actor);
        let actor_runtime = &self.entities.get(actor).unwrap().runtime;
        let actor_hp = actor_runtime.hp;
        if matches!(skill, Assassinate) && actor_runtime.assassinate.is_some() {
            return vec![PreparedTargetList::new()];
        }
        if matches!(skill, Shadow | Iron | Clone | Charge | Accumulate | Summon) {
            return vec![smallvec::smallvec![actor]];
        }
        if matches!(skill, WeaponS11 | WeaponDeathNote | WeaponXuanYue) {
            return self
                .prepare_plain_builtin_skill_action(actor, smart, SelectedBuiltinSkill { skill, fixed_lane: 0 })
                .map(|action| vec![action.targets])
                .unwrap_or_default();
        }
        let mut targets: Vec<_> = if skill == Revive {
            self.world
                .team_roster(team)
                .unwrap_or_default()
                .iter()
                .copied()
                .filter(|&id| {
                    self.entities.get(id).is_some_and(|entity| {
                        !entity.runtime.alive
                            && !entity.runtime.is_combat_minion()
                            && entity.runtime.corpse == RuntimeCorpseKind::None
                    })
                })
                .collect()
        } else {
            self.world
                .flat_alive()
                .iter()
                .filter(|target| !self.is_quilt(**target))
                .copied()
                .filter(|&id| {
                    let entity = self.entities.get(id).unwrap();
                    match skill {
                        Heal => entity.runtime.team == team && entity.runtime.hp < entity.template.max_hp,
                        Haste => entity.runtime.team == team,
                        // 蓄力的生命之轮还会夺取行动条，即使不回血也可能有战术价值。
                        Exchange => entity.runtime.team != team && (entity.runtime.hp > actor_hp || actor_runtime.charge.active),
                        RinickYuriControl => entity.runtime.team != team && entity.states.entry(PLAIN_CHARM_STATE_KEY).is_none(),
                        _ => entity.runtime.team != team,
                    }
                })
                .collect()
        };
        if !matches!(skill, Quake | Rapid | RinickIceAge) {
            return targets.into_iter().map(|target| smallvec::smallvec![target]).collect();
        }
        let limit = match skill {
            // 连击的执行器最多使用三个目标，额外目标不会产生不同动作。
            Rapid => 3,
            _ => {
                if smart {
                    6
                } else {
                    5
                }
            }
        };
        // 低血量优先与高输出优先两种后续队列；每个敌人都可作为首目标。
        targets.sort_by_key(|&id| (self.entities.get(id).unwrap().runtime.hp, id));
        let mut power_order = targets.clone();
        power_order.sort_by_key(|&id| {
            (
                std::cmp::Reverse(
                    self.entities
                        .get(id)
                        .unwrap()
                        .runtime
                        .attack
                        .max(self.entities.get(id).unwrap().runtime.magic),
                ),
                id,
            )
        });
        let mut lists = Vec::new();
        for &first in &targets {
            for order in [&targets, &power_order] {
                let mut list = smallvec::smallvec![first];
                list.extend(order.iter().copied().filter(|&id| id != first).take(limit - 1));
                if !lists.contains(&list) {
                    lists.push(list);
                }
            }
        }
        lists
    }
}

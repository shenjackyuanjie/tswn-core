//! CPU 搜索的预算与一次决策结果；不改变默认自动战斗规则。
use super::*;

#[derive(Debug, Clone, Copy)]
pub struct SearchConfig {
    /// 只接管指定运行时队伍；None 表示双方均接管。
    pub team: Option<usize>,
    pub shortlist: usize,
    pub samples: usize,
    pub horizon: usize,
    /// 独立搜索种子，禁止从真实战斗随机状态推导未来结果。
    pub seed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 无随机资格失败的受控战场，测试实际搜索选择而非源码形状。
    fn tactical_field() -> CombatRuntime {
        let config = default_custom_runtime_import_config().unwrap();
        let loadout = [
            BuiltinActiveSkill::Heal,
            BuiltinActiveSkill::Revive,
            BuiltinActiveSkill::Ice,
            BuiltinActiveSkill::Haste,
            BuiltinActiveSkill::Disperse,
        ]
        .map(|skill| (config.registry.skill_id_by_export_name(skill.export_name()).unwrap(), 128));
        CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "actor", 0, 300, 3)
                    .with_speed(100)
                    .with_speed_points(MOVE_POINT_THRESHOLD)
                    .with_magic_point(100)
                    .with_target_score_stats(100, 100, 1.0)
                    .with_skill_loadout(SkillLoadout::from_skill_levels(loadout)),
                PlayerTemplate::new(2, "ally", 0, 300, 3).with_target_score_stats(50, 50, 1.0),
                PlayerTemplate::new(3, "enemy-a", 1, 300, 3).with_target_score_stats(80, 80, 1.0),
                PlayerTemplate::new(4, "enemy-b", 1, 300, 3).with_target_score_stats(20, 20, 1.0),
            ],
            config.registry,
        ))
    }

    fn charm_to(field: &mut CombatRuntime, id: EntityIdx, team: usize) {
        // 用真实状态容器登记，覆盖有效阵营解析而不修改原队伍。
        let mut entry = StateEntry::legacy(PLAIN_CHARM_STATE_KEY);
        entry.payload = StatePayload::Charm {
            group_id: team,
            effective_team_idx: Some(team),
            source_team_idx: None,
            target: None,
            step: 2,
        };
        field.entities.get_mut(id).unwrap().states.add_entry(entry);
    }

    #[test]
    fn all_single_targets_and_charmed_allegiance_are_enumerated() {
        let mut field = tactical_field();
        for skill in [
            BuiltinActiveSkill::Ice,
            BuiltinActiveSkill::Berserk,
            BuiltinActiveSkill::Half,
            BuiltinActiveSkill::Charm,
            BuiltinActiveSkill::Slow,
            BuiltinActiveSkill::Curse,
            BuiltinActiveSkill::Disperse,
            BuiltinActiveSkill::Assassinate,
            BuiltinActiveSkill::Possess,
        ] {
            let lists = field.search_skill_targets(EntityIdx(0), skill, true);
            assert_eq!(
                lists,
                vec![
                    PreparedTargetList::from_slice(&[EntityIdx(2)]),
                    PreparedTargetList::from_slice(&[EntityIdx(3)])
                ],
                "{skill:?}"
            );
        }
        assert!(field.search_skill_targets(EntityIdx(0), BuiltinActiveSkill::Heal, true).is_empty());
        field.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 299;
        assert_eq!(
            field.search_skill_targets(EntityIdx(0), BuiltinActiveSkill::Heal, true),
            vec![PreparedTargetList::from_slice(&[EntityIdx(1)])]
        );
        charm_to(&mut field, EntityIdx(0), 1);
        assert_eq!(
            field.search_skill_targets(EntityIdx(0), BuiltinActiveSkill::Fire, true),
            vec![
                PreparedTargetList::from_slice(&[EntityIdx(0)]),
                PreparedTargetList::from_slice(&[EntityIdx(1)])
            ]
        );
        assert_eq!(
            field.search_skill_targets(EntityIdx(0), BuiltinActiveSkill::Haste, true),
            vec![
                PreparedTargetList::from_slice(&[EntityIdx(2)]),
                PreparedTargetList::from_slice(&[EntityIdx(3)])
            ]
        );
    }

    #[test]
    fn revive_excludes_live_and_consumed_corpses_and_preserves_mp() {
        let mut field = tactical_field();
        let ally = field.entities.get_mut(EntityIdx(1)).unwrap();
        ally.runtime.alive = false;
        ally.runtime.hp = 0;
        field.world.mark_dead(EntityIdx(1), 0);
        let before = field.entities.get(EntityIdx(0)).unwrap().runtime.magic_point;
        assert_eq!(
            field.search_skill_targets(EntityIdx(0), BuiltinActiveSkill::Revive, true),
            vec![PreparedTargetList::from_slice(&[EntityIdx(1)])]
        );
        assert_eq!(field.entities.get(EntityIdx(0)).unwrap().runtime.magic_point, before);
        field.entities.get_mut(EntityIdx(1)).unwrap().runtime.corpse = RuntimeCorpseKind::Merge;
        assert!(field.search_skill_targets(EntityIdx(0), BuiltinActiveSkill::Revive, true).is_empty());
    }

    #[test]
    fn multi_target_actions_cover_every_first_target_without_duplicates() {
        let config = default_custom_runtime_import_config().unwrap();
        let templates = (0..10)
            .map(|i| PlayerTemplate::new(i + 1, format!("unit-{i}"), usize::from(i > 0), 300, 3))
            .collect();
        let mut field = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(templates, config.registry));
        for (skill, limit) in [
            (BuiltinActiveSkill::Rapid, 3),
            (BuiltinActiveSkill::Quake, 6),
            (BuiltinActiveSkill::RinickIceAge, 6),
        ] {
            let lists = field.search_skill_targets(EntityIdx(0), skill, true);
            for target in 1..10 {
                assert!(lists.iter().any(|list| list[0] == EntityIdx(target)));
            }
            assert!(lists.len() <= 18);
            for list in lists {
                assert_eq!(list.len(), limit);
                for (offset, id) in list.iter().enumerate() {
                    assert!(!list[..offset].contains(id));
                    assert_ne!(*id, EntityIdx(0));
                }
            }
        }
    }

    #[test]
    fn position_value_accounts_for_shield_control_speed_and_revive_reserve() {
        let field = tactical_field();
        let base = field.search_position_value(0);
        let mut shielded = field.clone();
        shielded.entities.get_mut(EntityIdx(0)).unwrap().runtime.shield = 200;
        assert!(shielded.search_position_value(0) > base);
        let mut frozen = field.clone();
        frozen
            .entities
            .get_mut(EntityIdx(0))
            .unwrap()
            .states
            .add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 1));
        assert!(frozen.search_position_value(0) < base);
        let mut faster = field.clone();
        faster.entities.get_mut(EntityIdx(0)).unwrap().template.speed += 30;
        assert!(faster.search_position_value(0) > base);
        let mut charmed = field.clone();
        charm_to(&mut charmed, EntityIdx(0), 1);
        assert!(charmed.search_position_value(0) < base);
        let mut corpse = field.clone();
        let ally = corpse.entities.get_mut(EntityIdx(1)).unwrap();
        ally.runtime.alive = false;
        ally.runtime.hp = 0;
        corpse.world.mark_dead(EntityIdx(1), 0);
        let recoverable = corpse.search_position_value(0);
        corpse.entities.get_mut(EntityIdx(1)).unwrap().runtime.corpse = RuntimeCorpseKind::Merge;
        assert!(corpse.search_position_value(0) < recoverable);
        assert!(recoverable < base);
    }

    #[test]
    fn candidate_trials_leave_real_world_and_rng_untouched() {
        let mut field = tactical_field();
        let turn = field.search_turn_preview();
        let before = format!("{field:?}");
        let mut rng = field.rng.clone();
        let team = turn.team.unwrap();
        for index in 0..turn.candidates.len() {
            let (value, _) = field.evaluate_search_candidate(index, team, 91, 24, true, None);
            assert!(value.is_finite() && (-1.0..=1.0).contains(&value));
        }
        assert_eq!(format!("{field:?}"), before);
        for _ in 0..64 {
            assert_eq!(field.rng.next_u8(), rng.next_u8());
        }
    }

    #[test]
    fn shortlisted_candidates_receive_equal_samples_before_selection() {
        let mut field = tactical_field();
        let result = field.run_search_round(SearchConfig {
            samples: 16,
            horizon: 24,
            shortlist: 4,
            ..SearchConfig::default()
        });
        assert!(result.estimates.len() >= 3);
        assert_eq!(result.estimates[0].samples, 16);
        assert!(result.estimates.iter().all(|estimate| estimate.samples == 16));
        for estimate in &result.estimates {
            assert!(estimate.wins <= estimate.completed && estimate.completed <= estimate.samples);
        }
        assert!(field.scratch.search_turn.is_none());
    }

    #[test]
    fn tactical_rollout_selects_available_lethal_attack_and_clears_scratch() {
        let config = default_custom_runtime_import_config().unwrap();
        let mut field = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "attacker", 0, 300, 1000)
                    .with_speed(100)
                    .with_speed_points(MOVE_POINT_THRESHOLD),
                PlayerTemplate::new(2, "victim", 1, 1, 3),
            ],
            config.registry,
        ));
        // 等待真正的出手机会；不把调度空轮当成策略已作决定。
        let mut winner = None;
        for _ in 0..200 {
            winner = field.run_tactical_rollout_round(7, None).0.winner_team;
            assert!(field.scratch.search_turn.is_none());
            if winner.is_some() {
                break;
            }
        }
        assert_eq!(winner, Some(0));
    }

    #[test]
    fn family_preservation_keeps_support_candidate_in_shortlist() {
        let candidates = vec![
            SearchCandidate {
                action: PreparedPlainAction::BasicAttack {
                    target: EntityIdx(2),
                    use_magic: false,
                    amount: 10,
                },
                cost: 0,
            },
            SearchCandidate {
                action: PreparedPlainAction::BasicAttack {
                    target: EntityIdx(3),
                    use_magic: false,
                    amount: 10,
                },
                cost: 0,
            },
            SearchCandidate {
                action: PreparedPlainAction::BuiltinSkill(PreparedBuiltinSkillAction {
                    selected: SelectedBuiltinSkill {
                        skill: BuiltinActiveSkill::Revive,
                        fixed_lane: 0,
                    },
                    targets: smallvec::smallvec![EntityIdx(1)],
                }),
                cost: 8,
            },
        ];
        assert_eq!(diverse_shortlist(&[0, 1, 2], &candidates, 3), vec![0, 2, 1]);
        assert_eq!(diverse_shortlist(&[0, 1, 2], &candidates, 1), vec![0]);
    }

    #[test]
    fn tactical_continuation_reaches_late_allied_decision_instead_of_wasting_empty_slots() {
        // 队友在第 24 轮之后才行动；原版会持续治疗，战术续局应看见可立即终结的攻击。
        let config = default_custom_runtime_import_config().unwrap();
        let heal = config.registry.skill_id_by_export_name(BuiltinActiveSkill::Heal.export_name()).unwrap();
        let mut field = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "starter", 0, 1000, 1)
                    .with_speed(1)
                    .with_speed_points(MOVE_POINT_THRESHOLD + 1),
                PlayerTemplate::new(2, "delayed-ally", 0, 1000, 1000)
                    .with_speed(1)
                    .with_speed_points(1800)
                    .with_wisdom(128)
                    .with_magic_point(128)
                    .with_skill_loadout(SkillLoadout::from_skill_levels([(heal, 128)])),
                PlayerTemplate::new(3, "enemy", 1, 100, 1).with_speed(1).with_def_res(100, 100),
            ],
            config.registry,
        ));
        field.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 500;
        assert_eq!(field.search_turn_preview().actor, Some(EntityIdx(0)));
        let (_, legacy_winner) = field.evaluate_search_candidate(0, 0, 91, 128, false, Some(0));
        let (_, tactical_winner) = field.evaluate_search_candidate(0, 0, 91, 128, true, Some(0));
        assert_eq!(legacy_winner, None);
        assert_eq!(tactical_winner, Some(0));
    }

    #[test]
    fn tactical_continuation_values_delayed_backstab_through_forced_resolution() {
        // 潜行起手没有即时伤害；必须比较强制背刺兑现后的结果，不能只奖励眼前的普攻。
        let config = default_custom_runtime_import_config().unwrap();
        let skill = config
            .registry
            .skill_id_by_export_name(BuiltinActiveSkill::Assassinate.export_name())
            .unwrap();
        let mut field = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "attacker", 0, 300, 100)
                    .with_magic(100)
                    .with_speed(200)
                    .with_speed_points(MOVE_POINT_THRESHOLD + 1)
                    .with_magic_point(100)
                    .with_skill_loadout(SkillLoadout::from_skill_levels([(skill, 128)])),
                PlayerTemplate::new(2, "victim", 1, 200, 0).with_speed(1).with_def_res(128, 128),
            ],
            config.registry,
        ));
        let (_, used) = field.run_tactical_rollout_round(7, Some(0));
        assert!(used);
        assert!(field.entities.get(EntityIdx(0)).unwrap().runtime.assassinate.is_some());
        for _ in 0..32 {
            if field.run_minimal_round_no_capture().winner_team.is_some() {
                break;
            }
        }
        assert!(!field.entities.get(EntityIdx(1)).unwrap().runtime.alive);
    }

    #[test]
    fn tactical_rollout_keeps_uncontrolled_opponents_on_exact_legacy_path() {
        // 相同 RNG 下对手的实体结果和随机流均应与原版完全一致。
        let mut field = tactical_field();
        let mut legacy = field.clone();
        field.run_tactical_rollout_round(123, Some(1));
        legacy.run_minimal_round_no_capture();
        assert_eq!(format!("{field:?}"), format!("{legacy:?}"));
        for _ in 0..64 {
            assert_eq!(field.rng.next_u8(), legacy.rng.next_u8());
        }
    }

    #[test]
    fn iron_and_backstab_simulation_matches_captured_resolution() {
        // 同一铁壁起手和 seed 必须在试算、正式帧中得到相同状态，并真实兑现后续背刺。
        let config = default_custom_runtime_import_config().unwrap();
        let skills = [BuiltinActiveSkill::Iron, BuiltinActiveSkill::Assassinate]
            .map(|skill| (config.registry.skill_id_by_export_name(skill.export_name()).unwrap(), 128));
        let mut captured = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "attacker", 0, 1000, 100)
                    .with_magic(100)
                    .with_magic_point(128)
                    .with_speed(220)
                    .with_speed_points(MOVE_POINT_THRESHOLD + 1)
                    .with_def_res(128, 128)
                    .with_skill_loadout(SkillLoadout::from_skill_levels(skills)),
                PlayerTemplate::new(2, "victim", 1, 1_000_000, 10).with_speed(10).with_def_res(128, 128),
            ],
            config.registry,
        ));
        let turn = captured.search_turn_preview();
        let iron = turn
            .candidates
            .iter()
            .position(|c| matches!(&c.action,PreparedPlainAction::BuiltinSkill(a) if a.selected.skill==BuiltinActiveSkill::Iron))
            .unwrap();
        let mut simulated = captured.clone();
        captured.scratch.search_turn = Some(SearchTurn::new(Some(iron), Some(42)));
        simulated.scratch.search_turn = Some(SearchTurn::new(Some(iron), Some(42)));
        let mut saw_iron = false;
        let mut saw_backstab = false;
        for _ in 0..128 {
            let a = captured.run_minimal_round();
            let b = simulated.run_minimal_round_no_capture();
            captured.scratch.search_turn = None;
            simulated.scratch.search_turn = None;
            assert_eq!(a.winner_team, b.winner_team);
            assert_eq!(format!("{:?}", captured.entities), format!("{:?}", simulated.entities));
            if let Some(frame) = a.frame {
                saw_iron |= frame.updates.updates.iter().any(|u| u.caster == 0 && u.message == "[0]发动[铁壁]");
                saw_backstab |= frame.updates.updates.iter().any(|u| u.caster == 0 && u.message == "[0]发动[背刺]");
            }
        }
        assert!(saw_iron && saw_backstab);
        for _ in 0..64 {
            assert_eq!(captured.rng.next_u8(), simulated.rng.next_u8());
        }
    }

    fn runner() -> RuntimeRunner {
        RuntimeRunner::new_from_groups_with_seed(&[vec!["alpha".into()], vec!["beta".into()]], &["search-test".into()]).unwrap()
    }
    #[test]
    fn same_budget_and_seed_reproduce_real_battle() {
        let mut a = runner();
        let mut b = a.clone();
        let config = SearchConfig {
            samples: 2,
            horizon: 4,
            ..SearchConfig::default()
        };
        for _ in 0..12 {
            a.runtime.run_search_round(config);
            b.runtime.run_search_round(config);
        }
        assert_eq!(a.model_state().unwrap(), b.model_state().unwrap());
    }
    #[test]
    fn probing_does_not_mutate_original_world_or_rng() {
        let a = runner();
        let before = a.model_state().unwrap();
        let mut probe = a.clone();
        probe.runtime.scratch.search_turn = Some(SearchTurn::new(None, None));
        for _ in 0..20 {
            probe.runtime.run_minimal_round_no_capture();
            if probe.runtime.scratch.search_turn.as_ref().is_some_and(|turn| turn.actor.is_some()) {
                break;
            }
        }
        assert_eq!(a.model_state().unwrap(), before);
        assert!(probe.runtime.scratch.search_turn.unwrap().actor.is_some());
    }
    #[test]
    fn simulations_do_not_advance_real_round_counter() {
        let mut a = runner();
        let mut b = a.clone();
        a.runtime.run_search_round(SearchConfig {
            samples: 2,
            horizon: 10,
            ..SearchConfig::default()
        });
        b.runtime.run_minimal_round();
        assert_eq!(a.runtime.round, b.runtime.round);
        assert!(a.runtime.scratch.search_turn.is_none());
    }
    #[test]
    fn unselected_team_follows_exact_legacy_state() {
        let mut a = runner();
        let mut b = a.clone();
        for _ in 0..20 {
            a.runtime.run_search_round(SearchConfig {
                team: Some(usize::MAX),
                ..SearchConfig::default()
            });
            b.runtime.run_minimal_round();
        }
        assert_eq!(a.model_state().unwrap(), b.model_state().unwrap());
    }
}
impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            team: None,
            shortlist: 4,
            samples: 16,
            horizon: 128,
            seed: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchCandidate {
    pub action: PreparedPlainAction,
    pub cost: i32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchTurn {
    pub chosen: Option<usize>,
    pub simulation_seed: Option<u64>,
    pub candidates: Vec<SearchCandidate>,
    pub actor: Option<EntityIdx>,
    /// 行动前魅惑等状态解析后的有效阵营，目标和价值使用同一口径。
    pub team: Option<usize>,
}
impl SearchTurn {
    pub(crate) fn new(chosen: Option<usize>, simulation_seed: Option<u64>) -> Self {
        Self {
            chosen,
            simulation_seed,
            candidates: Vec::new(),
            actor: None,
            team: None,
        }
    }
}
#[derive(Debug, Clone)]
pub struct SearchEstimate {
    pub action: PreparedPlainAction,
    pub value: f64,
    pub wins: usize,
    pub completed: usize,
    pub samples: usize,
}
#[derive(Debug, Clone)]
pub struct SearchRound {
    pub outcome: RoundOutcome,
    pub estimates: Vec<SearchEstimate>,
}

/// 一个候选的累计试算；筛选阶段和最终阶段分别记账，不把短续局当作终局样本。
#[derive(Default)]
struct CandidateStats {
    total: f64,
    wins: usize,
    completed: usize,
    samples: usize,
}
impl CandidateStats {
    fn mean(&self) -> f64 { self.total / self.samples.max(1) as f64 }

    fn add(&mut self, score: f64, winner: Option<usize>, team: usize) {
        self.total += score;
        self.wins += usize::from(winner == Some(team));
        self.completed += usize::from(winner.is_some());
        self.samples += 1;
    }
}

/// 普攻、输出、控制、支援、生成分别保留探索机会，避免同技能的不同目标挤满候选。
fn action_family(action: &PreparedPlainAction) -> u8 {
    use BuiltinActiveSkill::*;
    match action {
        PreparedPlainAction::BuiltinSkill(action) => match action.selected.skill {
            Heal | Revive | Haste | Iron | Charge | Accumulate => 3,
            Ice | Half | Berserk | Charm | Slow | Curse | Exchange | Disperse | RinickYuriControl => 2,
            Clone | Summon | Shadow | Possess => 4,
            _ => 1,
        },
        _ => 0,
    }
}

/// 先取总分第一，再保留最多半数不同类别的最佳候选，其余名额仍按分数补齐。
fn diverse_shortlist(ranked: &[usize], candidates: &[SearchCandidate], limit: usize) -> Vec<usize> {
    let limit = limit.max(1).min(ranked.len());
    let mut chosen = Vec::new();
    let mut families = Vec::new();
    for &index in ranked {
        let family = action_family(&candidates[index].action);
        if chosen.is_empty() || (!families.contains(&family) && chosen.len() < limit.div_ceil(2)) {
            chosen.push(index);
            families.push(family);
        }
    }
    for &index in ranked {
        if chosen.len() == limit {
            break;
        }
        if !chosen.contains(&index) {
            chosen.push(index);
        }
    }
    chosen
}

impl CombatRuntime {
    /// 有界机制评价，不读取名字、随机状态或未来结果，也不解释为校准胜率。
    pub fn search_position_value(&self, team: usize) -> f64 {
        let can_revive: Vec<_> = self
            .entities
            .iter()
            .filter_map(|(id, entity)| {
                (entity.runtime.alive
                    && entity.runtime.magic_point >= 8
                    && entity.template.skills.active_order().iter().any(|&lane| {
                        entity.template.skills.level_at(lane).unwrap_or(0) > 0
                            && self.builtin_active_skill(entity.template.skills.skills()[lane])
                                == Some(BuiltinActiveSkill::Revive)
                    }))
                .then(|| self.plain_effective_team(id))
            })
            .collect();
        let mut ally = 0.0;
        let mut enemy = 0.0;
        for (id, entity) in self.entities.iter() {
            let r = &entity.runtime;
            if !r.alive {
                // 只有尚可被己方苏生术恢复的常规尸体才保留少量潜力，不能把全灭当作存活。
                if !r.is_combat_minion() && r.corpse == RuntimeCorpseKind::None && can_revive.contains(&r.team) {
                    let reserve = 12.0 + f64::from(r.attack.max(r.magic).clamp(0, 512)) * 0.04;
                    if r.team == team {
                        ally += reserve;
                    } else {
                        enemy += reserve;
                    }
                }
                continue;
            }
            let max_hp = f64::from(entity.template.max_hp.max(1));
            let hp = f64::from(r.hp.max(0));
            let health = (hp / max_hp).clamp(0.0, 1.0);
            let power = f64::from(r.attack.max(r.magic).clamp(0, 4096));
            let durability = (f64::from(r.defense.max(0)) + f64::from(r.resistance.max(0))).min(2048.0) * 0.06;
            let mut readiness = 1.0;
            let mut damage = 0.0;
            let mut support = 0.0;
            if entity.states.is_frozen() {
                readiness *= 0.45;
            }
            for entry in entity.states.entries() {
                match entry.payload {
                    StatePayload::Poison { atp_bits, count, .. } if count > 0 => {
                        // 只折算近期的一跳伤害，避免无限外推持续状态。
                        let atp = f64::from_bits(atp_bits);
                        if atp.is_finite() {
                            damage += atp.max(0.0).min(hp) * 0.08;
                        }
                    }
                    StatePayload::Curse { prob, .. } => readiness *= 1.0 - f64::from(prob.clamp(0, 128)) / 256.0,
                    StatePayload::Iron { protect, step } if step > 0 => support += f64::from(protect.clamp(0, 256)) * 0.12,
                    _ => {}
                }
            }
            if r.charge.active && !r.charge.permanent {
                support += power * 0.12;
            }
            if r.accumulate.active {
                support += power * 0.16;
            }
            if r.hide.is_some() {
                support += 12.0;
            }
            if r.assassinate.is_some() {
                support += power * 0.10;
            }
            support += r
                .protect_from
                .iter()
                .filter(|link| {
                    self.entities
                        .get(link.owner)
                        .is_some_and(|owner| owner.runtime.alive && owner.runtime.team == r.team)
                })
                .count()
                .min(3) as f64
                * 6.0;
            let shield = f64::from(r.shield.max(0)).min(max_hp) * 0.18;
            let mobility = f64::from(entity.effective_speed().clamp(0, 1024)) * 0.08
                + f64::from(r.move_state.speed_points.clamp(0, MOVE_POINT_THRESHOLD)) * 0.01;
            let value = (35.0
                + 90.0 * health
                + hp.min(4096.0) * 0.16
                + power * health.sqrt() * readiness * 0.55
                + durability * health
                + f64::from(r.magic_point.clamp(0, 128)) * 0.20
                + mobility * readiness
                + shield
                + support
                - damage)
                .max(1.0);
            // 魅惑只暂借行动权，给有效阵营 75% 价值，原阵营保留恢复后的剩余价值。
            let effective = self.plain_effective_team(id);
            let credit = if effective == r.team {
                f64::from(r.team == team)
            } else {
                0.75 * f64::from(effective == team) + 0.25 * f64::from(r.team == team)
            };
            ally += value * credit;
            enemy += value * (1.0 - credit);
        }
        ((ally - enemy) / (ally + enemy).max(1.0)).clamp(-1.0, 1.0)
    }

    /// 探测只作用于副本；强制行动、无候选和不支持的实体仍由正式入口结算。
    fn search_turn_preview(&self) -> SearchTurn {
        let mut probe = self.clone();
        probe.scratch.search_turn = Some(SearchTurn::new(None, None));
        probe.run_minimal_round_no_capture();
        probe.scratch.search_turn.take().unwrap_or_else(|| SearchTurn::new(None, None))
    }

    fn search_reward(&self, winner: Option<usize>, team: usize) -> f64 {
        match winner {
            Some(winner) => {
                if winner == team {
                    1.0
                } else {
                    -1.0
                }
            }
            None => self.search_position_value(team) * 0.5,
        }
    }

    /// 轻量续局只做一层动作比较；潜行需等强制背刺兑现或被打断，禁止递归搜索。
    fn run_tactical_rollout_round(&mut self, seed: u64, controlled_team: Option<usize>) -> (RoundOutcome, bool) {
        let turn = self.search_turn_preview();
        // 单方评分的对手在模拟中也必须保持原版；普通双方搜索则允许双方使用战术续局。
        if controlled_team
            .is_some_and(|selected| turn.actor.is_none_or(|actor| self.entities.get(actor).unwrap().runtime.team != selected))
        {
            return (self.run_minimal_round_no_capture(), false);
        }
        let Some(team) = turn.team else {
            return (self.run_minimal_round_no_capture(), false);
        };
        if turn.candidates.is_empty() {
            return (self.run_minimal_round_no_capture(), false);
        }
        let actor = turn.actor.unwrap();
        let mut best = None;
        for index in 0..turn.candidates.len() {
            let mut trial = self.clone();
            trial.scratch.search_turn = Some(SearchTurn::new(Some(index), Some(seed)));
            let mut winner = trial.run_minimal_round_no_capture().winner_team;
            trial.scratch.search_turn = None;
            // 潜行与背刺是一次延迟动作；不提前结算虚构伤害，直接让正式引擎处理打断、强制出手。
            // 只跟进当前起手的 pending，最多 32 个调度轮，其他单位仍走原版，不再枚举新动作。
            for _ in 0..32 {
                if winner.is_some() || trial.entities.get(actor).is_none_or(|entity| entity.runtime.assassinate.is_none()) {
                    break;
                }
                winner = trial.run_minimal_round_no_capture().winner_team;
            }
            let score = trial.search_reward(winner, team);
            if best.is_none_or(|(_, value)| score > value) {
                best = Some((index, score));
            }
        }
        self.scratch.search_turn = Some(SearchTurn::new(best.map(|(index, _)| index), None));
        let outcome = self.run_minimal_round_no_capture();
        self.scratch.search_turn = None;
        (outcome, true)
    }

    fn evaluate_search_candidate(
        &self,
        index: usize,
        team: usize,
        seed: u64,
        horizon: usize,
        tactical: bool,
        controlled_team: Option<usize>,
    ) -> (f64, Option<usize>) {
        let mut trial = self.clone();
        trial.scratch.search_turn = Some(SearchTurn::new(Some(index), Some(seed)));
        let mut winner = trial.run_minimal_round_no_capture().winner_team;
        trial.scratch.search_turn = None;
        let mut tactical_decisions = 0;
        for round in 0..horizon {
            if winner.is_some() {
                break;
            }
            // 预算按真实可决策机会计数，不让空调度轮或原版对手耗尽三个续局名额。
            winner = if tactical && tactical_decisions < 3 {
                let (outcome, used) = trial.run_tactical_rollout_round(
                    seed.wrapping_add(round as u64).wrapping_add(0xd1b54a32d192ed03),
                    controlled_team,
                );
                tactical_decisions += usize::from(used);
                outcome.winner_team
            } else {
                trial.run_minimal_round_no_capture().winner_team
            };
        }
        (trial.search_reward(winner, team), winner)
    }

    /// 多样本短续局筛选后给所有入围动作相同预算，避免用四次偶然结果淘汰组合起手。
    pub fn run_search_round(&mut self, config: SearchConfig) -> SearchRound {
        let turn = self.search_turn_preview();
        let Some(actor) = turn.actor else {
            return SearchRound {
                outcome: self.run_minimal_round(),
                estimates: Vec::new(),
            };
        };
        // 接管设置按角色原队伍生效，动作目标及价值按本回合有效阵营判断。
        let owner_team = self.entities.get(actor).unwrap().runtime.team;
        if config.team.is_some_and(|selected| selected != owner_team) || turn.candidates.is_empty() {
            return SearchRound {
                outcome: self.run_minimal_round(),
                estimates: Vec::new(),
            };
        }
        let team = turn.team.unwrap();
        let samples = config.samples.max(1);
        let seed_at = |sample: usize| config.seed.wrapping_add((sample as u64).wrapping_mul(0x9e3779b97f4a7c15));
        let mut ranked: Vec<_> = (0..turn.candidates.len()).collect();
        let mut screening: Vec<CandidateStats> = (0..ranked.len()).map(|_| CandidateStats::default()).collect();
        // 至少两个独立样本，最多三个；先观察八个调度轮，减少即时评价的单次偶然性。
        for &index in &ranked {
            for sample in 0..samples.clamp(2, 3) {
                let (score, winner) =
                    self.evaluate_search_candidate(index, team, seed_at(sample), config.horizon.min(8), false, config.team);
                screening[index].add(score, winner, team);
            }
        }
        ranked.sort_by(|&a, &b| screening[b].mean().total_cmp(&screening[a].mean()).then(a.cmp(&b)));
        let mut active = diverse_shortlist(&ranked, &turn.candidates, config.shortlist);
        let mut stats: Vec<CandidateStats> = (0..turn.candidates.len()).map(|_| CandidateStats::default()).collect();
        let mut evaluated = active.clone();
        // 每个入围动作都完成同一组独立样本，不能凭前四次的偶然结果提前淘汰。
        for &index in &active {
            for sample in 0..samples {
                let (score, winner) =
                    self.evaluate_search_candidate(index, team, seed_at(sample), config.horizon, sample % 2 == 1, config.team);
                stats[index].add(score, winner, team);
            }
        }
        active.sort_by(|&a, &b| stats[b].mean().total_cmp(&stats[a].mean()).then(a.cmp(&b)));
        let chosen = active[0];
        // 输出按最终选择优先排列，保留各候选实际样本数供审计。
        evaluated.sort_by(|&a, &b| (a != chosen).cmp(&(b != chosen)).then(a.cmp(&b)));
        let estimates = evaluated
            .into_iter()
            .map(|index| SearchEstimate {
                action: turn.candidates[index].action.clone(),
                value: stats[index].mean(),
                wins: stats[index].wins,
                completed: stats[index].completed,
                samples: stats[index].samples,
            })
            .collect();
        self.scratch.search_turn = Some(SearchTurn::new(Some(chosen), None));
        let outcome = self.run_minimal_round();
        self.scratch.search_turn = None;
        SearchRound { outcome, estimates }
    }
}

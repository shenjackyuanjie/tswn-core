//! CPU 搜索的预算与一次决策结果；不改变默认自动战斗规则。
use super::nn_eval::ValueStage;
use super::*;

use std::sync::Once;
use std::sync::atomic::{AtomicU64, Ordering};

/// 可训练启发式权重表（训练用）：env TSWN_EVAL_W=逗号分隔覆盖，未设置/0 用代码默认。
/// 索引顺序：0尸体底 1尸体率 2耐久率 3蓄能对数 4火威胁 5火上限 6冰冻底 7冰冻斜率
/// 8诅咒率 9诅咒上限 10毒衰减 11毒率 12疾走支援 13疾走行动 14狂暴倍率 15狂暴斜率
/// 16减速斜率 17减速平伤 18铁壁率 19蓄力残值 20背刺率 21背刺置信 22反击底 23反击率
/// 24隐匿支援 25守护链 26护盾率 27速度率 28行动点率 29价值底 30血量权重 31HP率
/// 32输出权重 33MP率 34魅惑底 35魅惑步率
static EVAL_W: [AtomicU64; 36] = [const { AtomicU64::new(0) }; 36];
static EVAL_W_INIT: Once = Once::new();

fn env_eval_w(idx: usize, default: f64) -> f64 {
    EVAL_W_INIT.call_once(|| {
        if let Ok(s) = std::env::var("TSWN_EVAL_W") {
            for (i, tok) in s.split(',').enumerate() {
                if i >= EVAL_W.len() {
                    break;
                }
                if let Ok(v) = tok.trim().parse::<f64>() {
                    if v.is_finite() {
                        EVAL_W[i].store(v.to_bits(), Ordering::Relaxed);
                    }
                }
            }
        }
    });
    let bits = EVAL_W[idx].load(Ordering::Relaxed);
    if bits == 0 { default } else { f64::from_bits(bits) }
}

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
            let (value, _) = field.evaluate_search_candidate(index, team, 91, 24, true, true, None, true);
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
            winner = field.run_tactical_rollout_round(7, None, true).0.winner_team;
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
        let (_, legacy_winner) = field.evaluate_search_candidate(0, 0, 91, 128, false, true, Some(0), true);
        let (_, tactical_winner) = field.evaluate_search_candidate(0, 0, 91, 128, true, true, Some(0), true);
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
        let (_, used) = field.run_tactical_rollout_round(7, Some(0), true);
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
        field.run_tactical_rollout_round(123, Some(1), true);
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

    #[test]
    fn every_ordinary_active_skill_is_settled_and_valued() {
        fn assert_valued(field: &CombatRuntime, skill: BuiltinActiveSkill) {
            let mut expected_rng = field.rng.clone();
            let value = field.search_position_value(0);
            assert!(value.is_finite() && (-1.0..=1.0).contains(&value), "{skill:?}: {value}");
            assert_eq!(value, field.search_position_value(0), "{skill:?} must be deterministic");
            let mut actual_rng = field.rng.clone();
            for _ in 0..8 {
                assert_eq!(actual_rng.next_u8(), expected_rng.next_u8(), "{skill:?} valuation advanced RNG");
            }
        }

        fn settle(
            field: &mut CombatRuntime,
            actor: EntityIdx,
            skill: BuiltinActiveSkill,
            targets: PreparedTargetList,
            updates: &mut RunUpdates,
        ) {
            field.drain_plain_builtin_skill_into(
                actor,
                PreparedBuiltinSkillAction {
                    selected: SelectedBuiltinSkill { skill, fixed_lane: 0 },
                    targets,
                },
                updates,
            );
        }

        // 仅覆盖普通主动技能及召唤物动作；所有 +武器技能均不在本次评价范围。
        for skill in BuiltinActiveSkill::CORE
            .into_iter()
            .chain([BuiltinActiveSkill::Possess, BuiltinActiveSkill::SummonExplode])
        {
            let mut field = tactical_field();
            // 固定独立随机流覆盖成功施法；闪避/抵抗分支由对应技能测试单独验证。
            field.rng = RC4::new(b"ordinary-skill-effects", 1);
            {
                let actor = field.entities.get_mut(EntityIdx(0)).unwrap();
                actor.runtime.attack = 240;
                actor.runtime.magic = 80;
                actor.runtime.agility = 1000;
                actor.runtime.hp = 80;
            }
            field.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 250;
            if skill == BuiltinActiveSkill::Revive {
                let ally = field.entities.get_mut(EntityIdx(1)).unwrap();
                ally.runtime.alive = false;
                ally.runtime.hp = 0;
                field.world.mark_dead(EntityIdx(1), 0);
            }
            if skill == BuiltinActiveSkill::Disperse {
                field
                    .entities
                    .get_mut(EntityIdx(2))
                    .unwrap()
                    .states
                    .add_entry(StateEntry::berserk(PLAIN_BERSERK_STATE_KEY, 2));
            }
            let mut updates = RunUpdates::new();
            if matches!(
                skill,
                BuiltinActiveSkill::Summon
                    | BuiltinActiveSkill::SummonExplode
                    | BuiltinActiveSkill::Clone
                    | BuiltinActiveSkill::Shadow
            ) {
                let config = default_custom_runtime_import_config().unwrap();
                let raw = concat!(
                    r#"owner@red+ol:{"attrs":[86,86,86,86,86,86,86,300],"skills":{"sklshadow":10,"sklsummon":10,"sklzombie":10}}"#,
                    "\n",
                    "target@blue",
                );
                let mut field = RuntimeRunner::from_custom_mixed_namerena_raw(raw.to_owned(), config)
                    .expect("minion fixture should build")
                    .runtime;
                if matches!(skill, BuiltinActiveSkill::Clone | BuiltinActiveSkill::Shadow) {
                    let targets = field.search_skill_targets(EntityIdx(0), skill, true).remove(0);
                    settle(&mut field, EntityIdx(0), skill, targets, &mut updates);
                    assert!(updates.had_updates(), "{skill:?} executor emitted no settlement");
                    assert_valued(&field, skill);
                    continue;
                }
                settle(
                    &mut field,
                    EntityIdx(0),
                    BuiltinActiveSkill::Summon,
                    smallvec::smallvec![EntityIdx(0)],
                    &mut updates,
                );
                if skill == BuiltinActiveSkill::Summon {
                    assert!(updates.had_updates(), "{skill:?} executor emitted no settlement");
                    assert_valued(&field, skill);
                    continue;
                }
                let summoned = field
                    .entities
                    .iter()
                    .find(|(id, entity)| id.0 >= 2 && entity.runtime.is_combat_minion())
                    .map(|(id, _)| id)
                    .expect("summon must create a real minion before explosion");
                let targets = field.search_skill_targets(summoned, skill, true).remove(0);
                updates.reset();
                settle(&mut field, summoned, skill, targets, &mut updates);
                assert!(updates.had_updates(), "{skill:?} executor emitted no settlement");
                assert_valued(&field, skill);
                continue;
            }
            let targets = field
                .search_skill_targets(EntityIdx(0), skill, true)
                .into_iter()
                .next()
                .unwrap_or_else(|| panic!("{skill:?} has no legal test target"));
            settle(&mut field, EntityIdx(0), skill, targets, &mut updates);
            assert!(updates.had_updates(), "{skill:?} executor emitted no settlement");
            // 逐技能验证正式结算的可见结果，不能仅以“估值是有限数”冒充机制覆盖。
            use BuiltinActiveSkill::*;
            match skill {
                Fire | Ice | Thunder | Quake | Absorb | Poison | Rapid | Critical | Half | Berserk | Curse | Disperse => {
                    assert!(
                        field.entities.get(EntityIdx(2)).unwrap().runtime.hp < 300,
                        "{skill:?} must settle actual damage"
                    );
                }
                Exchange => {
                    assert_eq!(field.entities.get(EntityIdx(0)).unwrap().runtime.hp, 300);
                    assert_eq!(field.entities.get(EntityIdx(2)).unwrap().runtime.hp, 80);
                }
                Heal => assert!(field.entities.get(EntityIdx(0)).unwrap().runtime.hp > 80),
                Revive => assert!(field.entities.get(EntityIdx(1)).unwrap().runtime.active()),
                Charm => assert!(field.entities.get(EntityIdx(2)).unwrap().states.entry(PLAIN_CHARM_STATE_KEY).is_some()),
                Slow => assert!(field.entities.get(EntityIdx(2)).unwrap().states.entry(PLAIN_SLOW_STATE_KEY).is_some()),
                Haste => assert!(field.entities.get(EntityIdx(0)).unwrap().states.entry(PLAIN_HASTE_STATE_KEY).is_some()),
                Iron => assert!(field.entities.get(EntityIdx(0)).unwrap().states.entry(PLAIN_IRON_STATE_KEY).is_some()),
                Charge => assert!(field.entities.get(EntityIdx(0)).unwrap().runtime.charge.active),
                Accumulate => assert!(field.entities.get(EntityIdx(0)).unwrap().runtime.accumulate.active),
                Assassinate => assert!(field.entities.get(EntityIdx(0)).unwrap().runtime.assassinate.is_some()),
                Possess => {
                    assert!(!field.entities.get(EntityIdx(0)).unwrap().runtime.alive);
                    assert!(field.entities.get(EntityIdx(2)).unwrap().states.entry(PLAIN_BERSERK_STATE_KEY).is_some());
                }
                _ => unreachable!("普通召唤动作已在独立蓝图场景中验证"),
            }
            assert_valued(&field, skill);
        }
    }

    #[test]
    fn burn_and_curse_residuals_require_relevant_enemy_pressure() {
        // 同一状态放在不同对手面前，必须按能否兑现评价，而非一律按最大 HP 扣分。
        let penalty = |field: &CombatRuntime, payload: StatePayload| {
            let mut changed = field.clone();
            let mut entry = StateEntry::legacy(9001);
            entry.payload = payload;
            changed.entities.get_mut(EntityIdx(2)).unwrap().states.add_entry(entry);
            field.search_position_value(0) - changed.search_position_value(0)
        };
        let mut field = tactical_field();
        assert_eq!(penalty(&field, StatePayload::FireMagHalfSteps(4)), 0.0);
        let fire = field.registry.skill_id_by_export_name(BuiltinActiveSkill::Fire.export_name()).unwrap();
        field.entities.get_mut(EntityIdx(0)).unwrap().template.skills = SkillLoadout::from_skill_levels([(fire, 128)]);
        field.entities.get_mut(EntityIdx(0)).unwrap().runtime.magic = 100;
        assert!(penalty(&field, StatePayload::FireMagHalfSteps(4)) < 0.0);
        field.entities.get_mut(EntityIdx(0)).unwrap().runtime.magic_point = 0;
        assert_eq!(penalty(&field, StatePayload::FireMagHalfSteps(4)), 0.0);
        for id in [EntityIdx(0), EntityIdx(1)] {
            let r = &mut field.entities.get_mut(id).unwrap().runtime;
            r.attack = 0;
            r.magic = 0;
        }
        assert_eq!(penalty(&field, StatePayload::Curse { prob: 42, multiply: 3 }), 0.0);
        field.entities.get_mut(EntityIdx(0)).unwrap().runtime.attack = 100;
        assert!(penalty(&field, StatePayload::Curse { prob: 42, multiply: 3 }) < 0.0);
    }

    #[test]
    fn poison_residual_accounts_for_magic_mitigation_and_total_reservoir() {
        let penalty = |field: &CombatRuntime, atp: f64, count| {
            let mut changed = field.clone();
            let mut entry = StateEntry::legacy(PLAIN_POISON_STATE_KEY);
            entry.payload = StatePayload::Poison {
                caster: None,
                target: None,
                atp_bits: atp.to_bits(),
                count,
            };
            changed.entities.get_mut(EntityIdx(2)).unwrap().states.add_entry(entry);
            changed.search_position_value(0) - field.search_position_value(0)
        };
        let mut field = tactical_field();
        let weak = penalty(&field, 6400.0, 1);
        assert!(weak > 0.0);
        assert!(penalty(&field, 12800.0, 1) > weak);
        assert!(penalty(&field, 6400.0, 3) < weak);
        field.entities.get_mut(EntityIdx(2)).unwrap().runtime.magic = 300;
        assert!(penalty(&field, 6400.0, 1) < weak);
        assert_eq!(penalty(&field, f64::NAN, 1), 0.0);
        assert_eq!(penalty(&field, 6400.0, 0), 0.0);
    }

    #[test]
    fn pending_backstab_values_resistance_interruptibility_and_live_target() {
        let bonus = |field: &CombatRuntime, break_on_damage| {
            let mut changed = field.clone();
            changed.entities.get_mut(EntityIdx(0)).unwrap().runtime.assassinate = Some(AssassinateRuntime {
                fixed_lane: 0,
                target: EntityIdx(2),
                break_on_damage,
            });
            changed.search_position_value(0) - field.search_position_value(0)
        };
        let mut field = tactical_field();
        let safe = bonus(&field, false);
        assert!(safe > bonus(&field, true));
        field.entities.get_mut(EntityIdx(2)).unwrap().runtime.resistance = 1000;
        assert!(bonus(&field, false) < safe);
        let target = field.entities.get_mut(EntityIdx(2)).unwrap();
        target.runtime.alive = false;
        target.runtime.hp = 0;
        field.world.mark_dead(EntityIdx(2), 1);
        assert_eq!(bonus(&field, false), 0.0);
    }

    #[test]
    fn charged_exchange_enumerates_and_settles_turn_bar_trade_without_healing() {
        // 回血阈值不能屏蔽蓄力生命之轮夺取行动条的真实效果。
        let mut field = tactical_field();
        let skill = field
            .registry
            .skill_id_by_export_name(BuiltinActiveSkill::Exchange.export_name())
            .unwrap();
        field.entities.get_mut(EntityIdx(0)).unwrap().template.skills = SkillLoadout::from_skill_levels([(skill, 128)]);
        field.entities.get_mut(EntityIdx(2)).unwrap().runtime.hp = 100;
        assert!(field.search_skill_targets(EntityIdx(0), BuiltinActiveSkill::Exchange, true).is_empty());
        field.entities.get_mut(EntityIdx(0)).unwrap().activate_charge_runtime();
        field.entities.get_mut(EntityIdx(2)).unwrap().runtime.move_state.speed_points = 700;
        let before = field.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points;
        let targets = field.search_skill_targets(EntityIdx(0), BuiltinActiveSkill::Exchange, true);
        assert!(targets.contains(&smallvec::smallvec![EntityIdx(2)]));
        field.drain_plain_builtin_skill_into(
            EntityIdx(0),
            PreparedBuiltinSkillAction {
                selected: SelectedBuiltinSkill {
                    skill: BuiltinActiveSkill::Exchange,
                    fixed_lane: 0,
                },
                targets: smallvec::smallvec![EntityIdx(2)],
            },
            &mut RunUpdates::new(),
        );
        assert_eq!(field.entities.get(EntityIdx(0)).unwrap().runtime.hp, 100);
        assert_eq!(field.entities.get(EntityIdx(2)).unwrap().runtime.hp, 300);
        assert_eq!(
            field.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points,
            before + 700
        );
        assert_eq!(field.entities.get(EntityIdx(2)).unwrap().runtime.move_state.speed_points, 0);
    }

    #[test]
    fn charge_and_accumulate_stack_without_saturating_the_same_residual() {
        // 用真实激活与属性刷新验证组合收益，不手工伪造倍率。
        let mut charged = tactical_field();
        let base = charged.search_position_value(0);
        charged.entities.get_mut(EntityIdx(0)).unwrap().activate_charge_runtime();
        let charge_value = charged.search_position_value(0);
        assert!(charge_value > base);
        charged.entities.get_mut(EntityIdx(0)).unwrap().activate_accumulate_runtime();
        assert!(charged.search_position_value(0) > charge_value);
    }

    #[test]
    fn ordinary_state_residuals_follow_mechanism_duration_and_pending_target() {
        fn entry(key: u32, payload: StatePayload) -> StateEntry {
            let mut entry = StateEntry::legacy(key);
            entry.payload = payload;
            entry
        }

        let field = tactical_field();
        let base = field.search_position_value(0);
        let with_state = |id, state| {
            let mut changed = field.clone();
            assert!(changed.entities.get_mut(id).unwrap().states.add_entry(state));
            changed.search_position_value(0)
        };
        let poison = |count| {
            entry(
                9001,
                StatePayload::Poison {
                    caster: None,
                    target: None,
                    atp_bits: 6400.0f64.to_bits(),
                    count,
                },
            )
        };
        let poison_one = with_state(EntityIdx(2), poison(1));
        let poison_three = with_state(EntityIdx(2), poison(3));
        // 相同总毒量分成更多跳，不应凭空产生更大的近期收益。
        assert!(poison_three <= poison_one, "poison one={poison_one}, three={poison_three}");
        assert!(
            with_state(EntityIdx(2), entry(9002, StatePayload::FireMagHalfSteps(4)))
                == with_state(EntityIdx(2), entry(9002, StatePayload::FireMagHalfSteps(1)))
        );
        let curse = |multiply| entry(9003, StatePayload::Curse { prob: 64, multiply });
        assert!(with_state(EntityIdx(2), curse(4)) > with_state(EntityIdx(2), curse(1)));
        assert!(
            with_state(EntityIdx(2), entry(9004, StatePayload::Ice { frozen_step: 4 }))
                > with_state(EntityIdx(2), entry(9004, StatePayload::Ice { frozen_step: 1 }))
        );
        assert!(
            with_state(EntityIdx(2), entry(9005, StatePayload::Slow { step: 4 }))
                > with_state(EntityIdx(2), entry(9005, StatePayload::Slow { step: 1 }))
        );
        let haste = |step| StatePayload::Haste {
            faster: 64,
            effective_faster: 64,
            step,
        };
        assert!(with_state(EntityIdx(0), entry(9006, haste(4))) > with_state(EntityIdx(0), entry(9006, haste(1))));
        assert!(
            with_state(EntityIdx(0), entry(9007, StatePayload::Iron { protect: 100, step: 4 }))
                > with_state(EntityIdx(0), entry(9007, StatePayload::Iron { protect: 100, step: 1 }))
        );
        assert!(with_state(EntityIdx(0), entry(9008, StatePayload::ShieldValue(200))) > base);

        let charm = |step| StatePayload::Charm {
            group_id: 0,
            effective_team_idx: Some(0),
            source_team_idx: None,
            target: None,
            step,
        };
        assert!(with_state(EntityIdx(2), entry(9009, charm(4))) > with_state(EntityIdx(2), entry(9009, charm(1))));

        let mut magic_target = field.clone();
        magic_target.entities.get_mut(EntityIdx(2)).unwrap().runtime.attack = 0;
        magic_target.entities.get_mut(EntityIdx(2)).unwrap().runtime.magic = 300;
        let magic_base = magic_target.search_position_value(0);
        magic_target
            .entities
            .get_mut(EntityIdx(2))
            .unwrap()
            .states
            .add_entry(StateEntry::berserk(PLAIN_BERSERK_STATE_KEY, 4));
        assert!(magic_target.search_position_value(0) > magic_base);

        let mut assassinate = field.clone();
        assassinate.entities.get_mut(EntityIdx(0)).unwrap().runtime.assassinate = Some(AssassinateRuntime {
            fixed_lane: 0,
            target: EntityIdx(2),
            break_on_damage: true,
        });
        assert!(assassinate.search_position_value(0) > base);
        let mut counter = field.clone();
        counter.entities.get_mut(EntityIdx(0)).unwrap().runtime.counter.pending = true;
        counter.entities.get_mut(EntityIdx(0)).unwrap().runtime.counter.last_target = Some(EntityIdx(2));
        assert!(counter.search_position_value(0) > base);
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
    /// 仅统计可用的普通主动技能；不把武器或零级技能计入后续威胁。
    fn search_has_skill(&self, entity: &EntityRecord, skill: BuiltinActiveSkill) -> bool {
        entity.template.skills.active_order().iter().any(|&lane| {
            entity.template.skills.level_at(lane).unwrap_or(0) > 0
                && self.builtin_active_skill(entity.template.skills.skills()[lane]) == Some(skill)
        })
    }

    /// 有界机制评价，不读取名字、随机状态或未来结果，也不解释为校准胜率。
    /// `TSWN_EVAL_NN=1` 且已装载网络权重时走神经网络估值，否则保持启发式。
    pub fn search_position_value(&self, team: usize) -> f64 {
        if crate::runtime::nn_eval::nn_enabled()
            && let Some(value) = crate::runtime::nn_eval::nn_value(&self.nn_features(team))
        {
            return value;
        }
        self.search_position_value_raw(team, None)
    }

    /// 启发式估值主体；`table` 为 Some 时使用固定权重表（神经网络特征提取的确定性输入），
    /// None 时使用环境覆盖权重。局部闭包 `eval_w` 遮蔽模块级 `env_eval_w` 以统一两条路径。
    pub(crate) fn search_position_value_raw(&self, team: usize, table: Option<&[f64; 36]>) -> f64 {
        let eval_w = |idx: usize, default: f64| match table {
            Some(table) => table[idx],
            None => env_eval_w(idx, default),
        };
        // 每次局面只收集一次施压能力；灼烧必须有后续火球/地裂，诅咒必须承受伤害才有残值。
        let threats: Vec<_> = self
            .entities
            .iter()
            .filter(|(_, entity)| entity.runtime.active())
            .map(|(id, entity)| {
                let power = f64::from(entity.runtime.attack.max(entity.runtime.magic).clamp(0, 4096));
                let fire = if entity.runtime.magic_point >= 8
                    && (self.search_has_skill(entity, BuiltinActiveSkill::Fire)
                        || self.search_has_skill(entity, BuiltinActiveSkill::Quake))
                {
                    f64::from(entity.runtime.magic.clamp(0, 4096))
                } else {
                    0.0
                };
                (self.plain_effective_team(id), power, fire)
            })
            .collect();
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
                    let reserve = eval_w(0, 12.0) + f64::from(r.attack.max(r.magic).clamp(0, 512)) * eval_w(1, 0.04);
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
            let durability = (f64::from(r.defense.max(0)) + f64::from(r.resistance.max(0))).min(2048.0) * eval_w(2, 0.153116288);
            let mut readiness = 1.0;
            let mut damage = 0.0;
            let mut support = 0.0;
            let attack_boost = r.at_boost();
            let attack_boost = if attack_boost.is_finite() {
                attack_boost.clamp(0.0, 16.0)
            } else {
                1.0
            };
            // 攻击倍率已经参与真实动作结算；horizon 残值只保留有界影响，避免蓄积覆盖全部局面信息。
            // 对数压缩仍能区分蓄力、聚气及二者叠加，避免两倍后全部饱和成同一个价值。
            let boost_value = 1.0 + attack_boost.max(0.25).log2() * eval_w(3, 0.0993943736);
            let mut action_power = power * boost_value;
            let mut state_shield = 0.0;
            let mut charm_steps = 0;
            let (incoming, fire_threat) = threats
                .iter()
                .filter(|(source_team, _, _)| *source_team != r.team)
                .fold((0.0_f64, 0.0_f64), |(damage, fire), (_, power, fire_power)| {
                    (damage + power, fire + fire_power)
                });
            let duration = |step: i32| f64::from(step.clamp(0, 4)) / 4.0;
            for entry in entity.states.entries() {
                match entry.payload {
                    StatePayload::FireMagHalfSteps(half_steps) if half_steps > 0 => {
                        // 实际机制是后续火系/震系增伤，不是降低自身魔法输出。
                        damage +=
                            (fire_threat * f64::from(half_steps.clamp(0, 8)) * eval_w(4, 0.04)).min(hp * eval_w(5, 0.587449632));
                    }
                    StatePayload::Ice { frozen_step } => {
                        // frozen_step 是冻结行动进度，不是剩余回合数；归零后仍需到达解除动作点。
                        let lost_actions = (f64::from(frozen_step.max(0)) / f64::from(MOVE_POINT_THRESHOLD)).min(2.0);
                        readiness *= eval_w(6, 0.75) - eval_w(7, 0.25) * lost_actions;
                    }
                    StatePayload::ShieldValue(value) if value > 0 => {
                        state_shield += f64::from(value).min(max_hp);
                    }
                    StatePayload::Curse { prob, multiply } if prob > 0 => {
                        // 正式判定使用六位随机数；诅咒不妨碍自身行动，只放大受击伤害。
                        let chance = f64::from(prob.clamp(0, 64)) / 64.0;
                        damage +=
                            (incoming * chance * f64::from(multiply.saturating_sub(1).clamp(0, 8)) * eval_w(8, 0.124457993))
                                .min(hp * eval_w(9, 0.5));
                    }
                    StatePayload::Poison { atp_bits, count, .. } if count > 0 => {
                        let atp = f64::from_bits(atp_bits);
                        if atp.is_finite() {
                            // atp 是待分摊总量，按正式递推折算最多三跳，再折扣较晚的伤害。
                            let mut remaining = atp.max(0.0);
                            let mut weight = 1.0;
                            let mut expected = 0.0;
                            for offset in 0..count.min(3) {
                                let left = f64::from(count - offset);
                                let tick = remaining * (1.0 + (left - 1.0) * 0.10000000149011612) / left;
                                expected += (tick / (f64::from(r.magic) + 64.0).max(1.0)).ceil() * weight;
                                remaining = (remaining - tick).max(0.0);
                                weight *= eval_w(10, 0.334896921);
                            }
                            damage += expected.min(hp) * eval_w(11, 0.4132613);
                        }
                    }
                    StatePayload::Haste { step, .. } if step > 0 => {
                        let remaining = duration(step);
                        support += power * eval_w(12, 0.02) * remaining;
                        readiness *= 1.0 + eval_w(13, 0.08) * remaining;
                    }
                    StatePayload::Berserk { step } if step > 0 => {
                        let remaining = duration(step);
                        let berserk_power = f64::from(r.attack.clamp(0, 4096)) * boost_value * eval_w(14, 1.2);
                        // 狂暴是 1.2 倍物理、随机目标且失去技能选择，不能按通用增益处理。
                        action_power = action_power * (1.0 - remaining) + berserk_power * remaining;
                        readiness *= 1.0 - eval_w(15, 0.25) * remaining;
                    }
                    StatePayload::Charm { step, .. } => charm_steps = charm_steps.max(step),
                    StatePayload::Slow { step } if step > 0 => {
                        let remaining = duration(step);
                        readiness *= 1.0 - eval_w(16, 0.12) * remaining;
                        damage += eval_w(17, 4.0) * remaining;
                    }
                    StatePayload::Iron { protect, step } if step > 0 => {
                        support += f64::from(protect.clamp(0, 256)) * eval_w(18, 0.105811553) * (0.5 + 0.5 * duration(step));
                    }
                    // Boss 不进入 Clever 搜索，也不为其专属状态建立启发式估值。
                    StatePayload::CovidBoss { .. }
                    | StatePayload::CovidInfection { .. }
                    | StatePayload::SaitamaBoss { .. }
                    | StatePayload::LazyBoss { .. }
                    | StatePayload::LazyInfection { .. }
                    | StatePayload::MarioBoss { .. } => {}
                    // 已处理分支的零值、负值或非有限值不贡献未来收益；保持穷举以便新增状态必须显式审查。
                    StatePayload::None
                    | StatePayload::FireMagHalfSteps(_)
                    | StatePayload::ShieldValue(_)
                    | StatePayload::Curse { .. }
                    | StatePayload::Poison { .. }
                    | StatePayload::Haste { .. }
                    | StatePayload::Berserk { .. }
                    | StatePayload::Slow { .. }
                    | StatePayload::Iron { .. } => {}
                }
            }
            if r.charge.active && !r.charge.permanent {
                // 速度点及蓄积联动已分别进入 mobility/at_boost，这里只保留其他技能的蓄力联动残值。
                support += power * eval_w(19, 0.06);
            }
            if let Some(pending) = r.assassinate.as_ref() {
                if let Some(target) = self.entities.get(pending.target).filter(|target| target.runtime.alive) {
                    let target_hp = f64::from(target.runtime.hp.max(0));
                    // 背刺是四倍魔法攻击，不是按目标 HP 必杀；脆弱潜行保留被打断的折扣。
                    let nominal = (f64::from(r.magic.clamp(0, 127)) + 32.0) * 96.0 * attack_boost * 4.0;
                    let expected_strike = nominal / (f64::from(target.runtime.resistance) + 64.0).max(1.0);
                    let confidence = if pending.break_on_damage { eval_w(21, 0.5) } else { 1.0 };
                    support += expected_strike.min(target_hp).min(4096.0) * eval_w(20, 0.08) * confidence;
                }
            }
            if r.counter.pending
                && r.counter
                    .last_target
                    .is_some_and(|target| self.entities.get(target).is_some_and(|target| target.runtime.active()))
            {
                // 普通反击走物攻；无有效待反击目标时不保留虚假的攻击收益。
                support += eval_w(22, 8.0) + f64::from(r.attack.clamp(0, 4096)) * boost_value * eval_w(23, 0.04);
            }
            // +武器只由正式续局结算，不添加专属残值或决策偏好。
            if r.hide.is_some() {
                support += eval_w(24, 12.0);
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
                * eval_w(25, 6.0);
            let shield = (f64::from(r.shield.max(0)) + state_shield).min(max_hp) * eval_w(26, 0.0884436616);
            let mobility = f64::from(entity.effective_speed().clamp(0, 1024)) * eval_w(27, 0.08)
                + f64::from(r.move_state.speed_points.clamp(0, MOVE_POINT_THRESHOLD)) * eval_w(28, 0.01);
            let value = (eval_w(29, 60.3681443)
                + eval_w(30, 128.278657) * health
                + hp.min(4096.0) * eval_w(31, 0.4466431)
                + action_power * health.sqrt() * readiness * eval_w(32, 1.3862735)
                + durability * health
                + f64::from(r.magic_point.clamp(0, 128)) * eval_w(33, 0.532987596)
                + mobility * readiness
                + shield
                + support
                - damage)
                .max(1.0);
            // 魅惑只暂借行动权；剩余步数越多，有效阵营获得的价值越高，原阵营保留恢复残值。
            let effective = self.plain_effective_team(id);
            let credit = if effective == r.team {
                f64::from(r.team == team)
            } else {
                let borrowed = eval_w(34, 0.70) + eval_w(35, 0.05) * f64::from(charm_steps.clamp(1, 4));
                borrowed * f64::from(effective == team) + (1.0 - borrowed) * f64::from(r.team == team)
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

    /// 终局直接给 ±1；未终局按 `use_nn` 选择估值路径（混合模式下筛选走启发式、终局走网络），
    /// 并在采集开启（TSWN_DUMP_VALUE=1）时登记估值状态供训练分布对齐使用。
    fn search_reward(&self, winner: Option<usize>, team: usize, use_nn: bool, stage: ValueStage) -> f64 {
        match winner {
            Some(winner) => {
                if winner == team {
                    1.0
                } else {
                    -1.0
                }
            }
            None => {
                crate::runtime::nn_eval::nn_dump_push(self, team, stage);
                // 训练目标为 ±1 终局期望，与终局奖励使用相同尺度。
                if stage != ValueStage::Screening
                    && use_nn
                    && crate::runtime::nn_eval::nn_enabled()
                    && let Some(value) = crate::runtime::nn_eval::nn_value(&self.nn_features(team))
                {
                    return value;
                }
                let value = if use_nn && !crate::runtime::nn_eval::nn_hybrid_enabled() {
                    self.search_position_value(team)
                } else {
                    self.search_position_value_raw(team, None)
                };
                value * 0.5
            }
        }
    }

    /// 轻量续局只做一层动作比较；潜行需等强制背刺兑现或被打断，禁止递归搜索。
    fn run_tactical_rollout_round(&mut self, seed: u64, controlled_team: Option<usize>, use_nn: bool) -> (RoundOutcome, bool) {
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
        // 单候选不存在比较，直接执行可避免一次完整克隆和重复动作。
        if turn.candidates.len() == 1 {
            self.scratch.search_turn = Some(SearchTurn::new(Some(0), None));
            let outcome = self.run_minimal_round_no_capture();
            self.scratch.search_turn = None;
            return (outcome, true);
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
            let score = trial.search_reward(winner, team, use_nn, ValueStage::Tactical);
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
        final_leaf: bool,
        controlled_team: Option<usize>,
        use_nn: bool,
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
                    use_nn,
                );
                tactical_decisions += usize::from(used);
                outcome.winner_team
            } else {
                trial.run_minimal_round_no_capture().winner_team
            };
        }
        (
            trial.search_reward(
                winner,
                team,
                use_nn,
                if final_leaf {
                    ValueStage::Final
                } else {
                    ValueStage::Screening
                },
            ),
            winner,
        )
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
        // 只有一个合法动作时没有任何比较分支。直接执行可保留真实 RNG 和战斗结果，
        // 避免为“选择”唯一候选重复运行整轮战场。
        if turn.candidates.len() == 1 {
            self.scratch.search_turn = Some(SearchTurn::new(Some(0), None));
            let outcome = self.run_minimal_round();
            self.scratch.search_turn = None;
            return SearchRound {
                outcome,
                estimates: Vec::new(),
            };
        }
        let team = turn.team.unwrap();
        let samples = config.samples.max(1);
        let seed_at = |sample: usize| config.seed.wrapping_add((sample as u64).wrapping_mul(0x9e3779b97f4a7c15));
        // 混合模式（TSWN_EVAL_NN=1 + TSWN_EVAL_NN_HYBRID=1）：筛选阶段用启发式短视估值，
        // 其手调机制知识在 8 轮视界最可靠；最终阶段用网络估值。未启用网络时两路径同落启发式。
        let screening_nn = crate::runtime::nn_eval::nn_enabled() && !crate::runtime::nn_eval::nn_hybrid_enabled();
        let mut ranked: Vec<_> = (0..turn.candidates.len()).collect();
        let mut screening: Vec<CandidateStats> = (0..ranked.len()).map(|_| CandidateStats::default()).collect();
        // 至少两个独立样本，最多三个；先观察八个调度轮，减少即时评价的单次偶然性。
        for &index in &ranked {
            for sample in 0..samples.clamp(2, 3) {
                let (score, winner) = self.evaluate_search_candidate(
                    index,
                    team,
                    seed_at(sample),
                    config.horizon.min(8),
                    false,
                    false,
                    config.team,
                    screening_nn,
                );
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
                let (score, winner) = self.evaluate_search_candidate(
                    index,
                    team,
                    seed_at(sample),
                    config.horizon,
                    sample % 2 == 1,
                    true,
                    config.team,
                    true,
                );
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

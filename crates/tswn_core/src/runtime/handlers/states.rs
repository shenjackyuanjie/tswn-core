use super::*;

/// 护盾状态的 POST_DEFEND：按剩余盾值抵减伤害，破盾时保留完整伤害。
pub fn run_shield_post_defend_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::ShieldValue(shield)) = context.owner_state_payload(entry.legacy_order_key) else {
        return;
    };
    if shield <= 0 {
        return;
    }
    let damage = context.defend_damage().expect("shield state should run during POST_DEFEND");
    if damage > shield {
        context
            .set_owner_state_payload(entry.legacy_order_key, StatePayload::ShieldValue(0))
            .expect("shield state payload should still exist");
    } else {
        context.set_defend_damage(0);
        context
            .set_owner_state_payload(entry.legacy_order_key, StatePayload::ShieldValue(shield - damage))
            .expect("shield state payload should still exist");
    }
}

/// 诅咒状态的 POST_DEFEND：按概率把本次伤害乘以诅咒倍率。
pub fn run_curse_post_defend_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::Curse { prob, multiply }) = context.owner_state_payload(entry.legacy_order_key) else {
        return;
    };
    let damage = context.defend_damage().expect("curse state should run during POST_DEFEND");
    if damage <= 0 {
        return;
    }

    if (context.rng_next_u8() as u32) & 63 < prob as u32 {
        let caster = context.defend_caster().expect("curse state should receive incoming defend caster");
        let target = context.defend_target().expect("curse state should receive incoming defend target");
        context.add_update(crate::runtime::update::RunUpdate::new(
            "[诅咒]使伤害加倍",
            caster.0 as usize,
            target.0 as usize,
            0,
        ));
        context.set_defend_damage(damage * multiply);
    }
}

/// 中毒状态的 POST_ACTION：按剩余层数分摊毒性伤害并递减层数，归零解除。
pub fn run_poison_post_action_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::Poison {
        caster,
        target,
        atp_bits,
        count,
    }) = context.owner_state_payload(entry.legacy_order_key)
    else {
        return;
    };
    let Some(owner) = context.owner() else {
        return;
    };
    if !owner.runtime.alive {
        return;
    }

    let atp = f64::from_bits(atp_bits);
    let tick_atp = atp * (1.0 + (count - 1) as f64 * 0.10000000149011612) / count as f64;
    let next_atp = atp - tick_atp;
    let damage = (tick_atp / (owner.runtime.magic + 64) as f64).ceil() as i32;
    let next_count = count - 1;
    let poison_caster = caster.map_or(context.owner_idx(), EntityIdx);

    context.add_update(crate::runtime::update::RunUpdate::new(
        "[1][毒性发作]",
        poison_caster.0 as usize,
        context.owner_idx().0 as usize,
        0,
    ));
    context.push_nested(QueuedEffect::PoisonTick {
        caster: poison_caster,
        target: context.owner_idx(),
        amount: damage,
    });

    if next_count > 0 {
        context
            .set_owner_state_payload_without_refresh(
                entry.legacy_order_key,
                StatePayload::Poison {
                    caster,
                    target,
                    atp_bits: next_atp.to_bits(),
                    count: next_count,
                },
            )
            .expect("poison state payload should still exist");
        return;
    }

    context
        .clear_owner_state_without_refresh(entry.legacy_order_key)
        .expect("poison state payload should still exist");
}

/// 疾走状态的 POST_ACTION：递减步数，归零时播报解除。
pub fn run_haste_post_action_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::Haste {
        faster,
        effective_faster,
        step,
    }) = context.owner_state_payload(entry.legacy_order_key)
    else {
        return;
    };
    run_timed_release_post_action_state(
        context,
        entry,
        StatePayload::Haste {
            faster,
            effective_faster,
            step: step - 1,
        },
        step,
        "[1]从[疾走]中解除",
    );
}

/// 魅惑状态的 POST_ACTION：递减步数，归零时播报解除。
pub fn run_charm_post_action_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::Charm {
        group_id,
        effective_team_idx,
        source_team_idx,
        target,
        step,
    }) = context.owner_state_payload(entry.legacy_order_key)
    else {
        return;
    };
    run_timed_release_post_action_state(
        context,
        entry,
        StatePayload::Charm {
            group_id,
            effective_team_idx,
            source_team_idx,
            target,
            step: step - 1,
        },
        step,
        "[1]从[魅惑]中解除",
    );
}

/// 迟缓状态的 POST_ACTION：递减步数，归零时播报解除。
pub fn run_slow_post_action_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::Slow { step }) = context.owner_state_payload(entry.legacy_order_key) else {
        return;
    };
    run_timed_release_post_action_state(context, entry, StatePayload::Slow { step: step - 1 }, step, "[1]从[迟缓]中解除");
}

/// 原版的变异抽样属于前置钩子，即使这次最终由懒癌接管也照常执行。
fn roll_covid_mutations(
    context: &mut StateContext<'_>,
    entries: &mut [CovidInfectionEntry],
    mutation_set: &mut smallvec::SmallVec<[i32; 4]>,
) -> bool {
    let mut changed = false;
    for infection in entries {
        if context.rng_next_u8() < 64 {
            let mutation = context.rng_r127() as i32;
            changed |= infection.mutation != mutation;
            infection.mutation = mutation;
            if !mutation_set.contains(&mutation) {
                mutation_set.push(mutation);
                changed = true;
            }
        }
    }
    changed
}

/// 按注册序在新冠感染条目中定位本次钩子对应的毒株下标。
fn planned_covid_strain(
    context: &StateContext<'_>,
    entry: &StateHookPlanEntry,
    entries: &[CovidInfectionEntry],
) -> Option<usize> {
    let owner = context.owner()?;
    let original_order = owner.states.runtime_registration_order(entry.legacy_order_key)?;
    entries
        .iter()
        .position(|strain| strain.registration_order.unwrap_or(original_order) == entry.runtime_registration_order)
}

pub fn propose_boss_infection_pre_action(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) -> Option<StateHandlerFn> {
    match context.owner_state_payload(entry.legacy_order_key)? {
        StatePayload::CovidInfection {
            mut entries,
            mut mutation_set,
            recovered,
        } if !entries.is_empty() => {
            let index = planned_covid_strain(context, entry, &entries)?;
            if roll_covid_mutations(context, &mut entries[index..=index], &mut mutation_set) {
                context
                    .set_owner_state_payload_without_refresh(
                        entry.legacy_order_key,
                        StatePayload::CovidInfection {
                            entries,
                            mutation_set,
                            recovered,
                        },
                    )
                    .expect("runtime covid state payload must still exist");
            }
            Some(execute_proposed_covid_pre_action)
        }
        StatePayload::LazyInfection { .. } if roll_lazy_pre_action(context) => Some(execute_proposed_lazy_pre_action),
        _ => None,
    }
}

/// 提议路径的新冠 PRE_ACTION 执行入口：变异已在前置钩子掷过，直接进入实现体。
fn execute_proposed_covid_pre_action(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    run_covid_infection_state_impl(context, entry, true);
}

/// 提议路径的懒癌 PRE_ACTION 执行入口：发作概率已判定过，直接进入实现体。
fn execute_proposed_lazy_pre_action(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    run_lazy_infection_state_impl(context, entry, true);
}

/// 新冠传染状态：推进病程，按变异抽样决定发作、重症或解除。
pub fn run_covid_infection_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    run_covid_infection_state_impl(context, entry, false);
}

/// 新冠传染状态实现：PRE_ACTION 按变异抽样决定发作或接管动作，POST_ACTION 推入新冠肺炎效果；`proposed` 表示变异已在前置钩子处理。
fn run_covid_infection_state_impl(context: &mut StateContext<'_>, entry: &StateHookPlanEntry, proposed: bool) {
    let Some(StatePayload::CovidInfection {
        mut entries,
        mut mutation_set,
        recovered,
    }) = context.owner_state_payload(entry.legacy_order_key)
    else {
        return;
    };
    if entries.is_empty() {
        return;
    }

    if context.hook().intersects(ProcMask::PRE_ACTION) {
        let smart = context
            .action_smart()
            .expect("runtime covid PRE_ACTION state must receive the action smart roll");
        if !proposed {
            roll_covid_mutations(context, &mut entries, &mut mutation_set);
        }

        // 冻结检查在变异前钩子之后、选敌和替代动作之前。
        if context.owner().is_some_and(|owner| owner.states.is_frozen()) {
            context
                .set_owner_state_payload_without_refresh(
                    entry.legacy_order_key,
                    StatePayload::CovidInfection {
                        entries,
                        mutation_set,
                        recovered,
                    },
                )
                .expect("runtime covid state payload must still exist");
            return;
        }

        let last_idx = if proposed {
            planned_covid_strain(context, entry, &entries).expect("runtime covid proposal must have a strain")
        } else {
            entries.len() - 1
        };
        let boss = entries[last_idx].boss;
        let mutation = entries[last_idx].mutation;
        let days = entries[last_idx].days;
        let all_alive = context.flat_alive().expect("runtime covid state must read the complete alive list");

        let boss_team = context.effective_team(boss).expect("runtime covid boss must exist");
        let skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                (context.entity(*candidate).expect("runtime covid alive candidate must exist").runtime.team == boss_team)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::with_capacity(select_count);
        let mut duplicate_count = 0usize;
        while duplicate_count <= select_count {
            let picked = if skip_indices.is_empty() {
                context.rng_pick_entity(&all_alive)
            } else {
                context.rng_pick_skip_range_entity(&all_alive, &skip_indices)
            };
            let Some(picked) = picked else {
                break;
            };
            let candidate = all_alive[picked];
            if selected.contains(&candidate) {
                duplicate_count += 1;
                continue;
            }
            selected.push(candidate);
            if selected.len() >= select_count {
                break;
            }
        }
        if !smart {
            for _ in &selected {
                let _ = context.rng_r_ffff();
            }
        }

        let owner = context.owner_idx();
        let owner_wisdom = context.owner().map(|entity| entity.runtime.wisdom).unwrap_or(0);
        if days == 0 || i32::from(context.rng_next_u8()) > owner_wisdom {
            entries[last_idx].days += i32::from(context.rng_next_u8() & 3);
            for _ in 0..5 {
                let Some(picked) = context.rng_pick_entity(&all_alive) else {
                    break;
                };
                let candidate = all_alive[picked];
                if candidate == owner || candidate == boss {
                    continue;
                }
                let candidate_entity = context.entity(candidate).expect("runtime covid spread candidate must exist");
                if !candidate_entity.runtime.alive {
                    continue;
                }
                let already_has_mutation = candidate_entity.states.entries().iter().any(|state| {
                    matches!(
                        &state.payload,
                        StatePayload::CovidInfection {
                            mutation_set,
                            ..
                        } if mutation_set.contains(&mutation)
                    )
                });
                if already_has_mutation {
                    continue;
                }
                let owner_team = context.owner().expect("runtime covid owner must exist").runtime.team;
                let effect = if candidate_entity.runtime.team == owner_team {
                    QueuedEffect::CovidContact {
                        owner,
                        candidate,
                        boss,
                        mutation,
                    }
                } else {
                    QueuedEffect::CovidAttack {
                        owner,
                        candidate,
                        boss,
                        mutation,
                    }
                };
                context
                    .set_owner_state_payload_without_refresh(
                        entry.legacy_order_key,
                        StatePayload::CovidInfection {
                            entries,
                            mutation_set,
                            recovered,
                        },
                    )
                    .expect("runtime covid state payload must still exist");
                context.push(effect);
                context.intercept_action();
                return;
            }
        }

        entries[last_idx].days += i32::from(context.rng_next_u8() & 3);
        let message = if entries[last_idx].days > 2 {
            crate::runtime::lang::keys::SKL_COVID_ICU
        } else {
            crate::runtime::lang::keys::SKL_COVID_STAY_HOME
        };
        context.add_update(crate::runtime::update::RunUpdate::new_lang(
            message,
            boss.0 as usize,
            owner.0 as usize,
            0,
        ));
        context
            .set_owner_state_payload_without_refresh(
                entry.legacy_order_key,
                StatePayload::CovidInfection {
                    entries,
                    mutation_set,
                    recovered,
                },
            )
            .expect("runtime covid state payload must still exist");
        context.intercept_action();
        return;
    }

    if context.hook().intersects(ProcMask::POST_ACTION) {
        let order = context
            .owner_has_boss_infection(entry.legacy_order_key)
            .then_some(entry.runtime_registration_order);
        context.push(QueuedEffect::CovidPneumonia {
            owner: context.owner_idx(),
            order,
        });
    }
}

/// 懒癌发作判定：128 分之一的概率，且本体当前可行动。
fn roll_lazy_pre_action(context: &mut StateContext<'_>) -> bool {
    context.rng_next_u8() < 128 && context.owner().is_some_and(|owner| owner.is_active())
}

/// 懒癌传染状态：按概率跳回合或发作，退场时解除。
pub fn run_lazy_infection_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    run_lazy_infection_state_impl(context, entry, false);
}

/// 懒癌传染状态实现：PRE_ACTION 掷概率后选目标跳回合或发作；`proposed` 表示概率已在前置钩子判定过。
fn run_lazy_infection_state_impl(context: &mut StateContext<'_>, entry: &StateHookPlanEntry, proposed: bool) {
    let Some(StatePayload::LazyInfection { boss }) = context.owner_state_payload(entry.legacy_order_key) else {
        return;
    };
    if context.hook().intersects(ProcMask::PRE_ACTION) {
        if !proposed && !roll_lazy_pre_action(context) {
            return;
        }
        let smart = context
            .action_smart()
            .expect("runtime lazy PRE_ACTION state must receive the action smart roll");
        let all_alive = context.flat_alive().expect("runtime lazy state must read the complete alive list");
        let boss_team = context.effective_team(boss).expect("runtime lazy boss must exist");
        let skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                (context.entity(*candidate).expect("runtime lazy candidate must exist").runtime.team == boss_team)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::with_capacity(select_count);
        let mut duplicate_count = 0usize;
        let invalid_count = -(select_count as i32);
        while duplicate_count <= select_count && invalid_count <= select_count as i32 {
            let picked = if skip_indices.is_empty() {
                context.rng_pick_entity(&all_alive)
            } else if all_alive.len() > skip_indices.len() {
                context.rng_pick_skip_range_entity(&all_alive, &skip_indices)
            } else {
                None
            };
            let Some(picked) = picked else {
                break;
            };
            if selected.contains(&picked) {
                duplicate_count += 1;
                continue;
            }
            selected.push(picked);
            if selected.len() >= select_count {
                break;
            }
        }
        if !smart {
            for _ in &selected {
                let _ = context.rng_r_ffff();
            }
        }
        use crate::runtime::lang::{keys, lazy_activity_key};
        let activity = lazy_activity_key(context.rng_next_u8());
        let owner = context.owner_idx().0 as usize;
        for key in [activity, keys::SKL_LAZY_SKIP_TURN_0] {
            context.add_update(crate::runtime::update::RunUpdate::new_lang(key, owner, owner, 0).without_target());
        }
        context.intercept_action();
        return;
    }

    // 感染状态跟随目标，来源退场不会解除已有感染。
    if context.hook().intersects(ProcMask::POST_ACTION) {
        context.push(QueuedEffect::LazyFlare {
            owner: context.owner_idx(),
            boss,
        });
    }
}

/// 一拳超人 Boss 状态的 POST_DEFEND：累计受伤与攻击者，伤害按百分之一结算。
pub fn run_saitama_boss_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    if context.hook() != ProcMask::POST_DEFEND {
        return;
    }
    let Some(StatePayload::SaitamaBoss {
        turns,
        mut damages,
        mut hitters,
        mut minions,
    }) = context.owner_state_payload(entry.legacy_order_key)
    else {
        return;
    };
    let damage = context.defend_damage().expect("runtime saitama state requires POST_DEFEND damage");
    let caster = context.defend_caster().expect("runtime saitama state requires a damage caster");
    damages += damage;
    let caster_entity = context
        .entity(caster)
        .unwrap_or_else(|error| panic!("runtime saitama caster lookup failed: {error:?}"));
    let hitter = if caster_entity.runtime.is_minion() && caster_entity.runtime.owner != caster {
        if !minions.contains(&caster) {
            minions.push(caster);
        }
        if caster_entity.runtime.is_combat_minion() {
            caster_entity.runtime.owner
        } else {
            caster_entity.runtime.root_owner
        }
    } else {
        caster
    };
    if !hitters.contains(&hitter) {
        hitters.push(hitter);
    }
    context
        .set_owner_state_payload(
            entry.legacy_order_key,
            StatePayload::SaitamaBoss {
                turns,
                damages,
                hitters,
                minions,
            },
        )
        .expect("runtime saitama state owner must exist");
    context.set_defend_damage(damage / 100);
}

/// 计时解除类状态的 POST_ACTION 收尾：步数归零时写回载荷并播报解除消息（含一拳超人的特殊发帧条件）。
fn run_timed_release_post_action_state(
    context: &mut StateContext<'_>,
    entry: &StateHookPlanEntry,
    next_payload: StatePayload,
    step: i32,
    release_message: &'static str,
) {
    let next_step = step - 1;
    if next_step > 0 {
        context
            .set_owner_state_payload_without_refresh(entry.legacy_order_key, next_payload)
            .expect("timed state payload should still exist");
        return;
    }

    context
        .set_owner_state_payload_without_refresh(entry.legacy_order_key, next_payload)
        .expect("timed state payload should still exist");
    let should_emit_release = context.owner().is_some_and(|owner| {
        // 一拳超人饥饿时只移出场上名单，原版仍按正体力判断自然解除能否发帧。
        // 终局退场不会进入行动后链，这里只补非终局退场的发帧。
        owner.runtime.alive
            || (owner.runtime.hp > 0 && owner.runtime.flags.contains(PlayerKindFlags::BOSS) && owner.template.name == "saitama")
    });
    context.clear_owner_state(entry.legacy_order_key).expect("timed state should still exist");
    if should_emit_release {
        context.add_newline();
        context.add_update(crate::runtime::update::RunUpdate::new(
            release_message,
            context.owner_idx().0 as usize,
            context.owner_idx().0 as usize,
            0,
        ));
    }
}

/// 铁壁状态的 POST_DEFEND：按保护值削减伤害并递减步数。
pub fn run_iron_post_defend_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::Iron { protect, step }) = context.owner_state_payload(entry.legacy_order_key) else {
        return;
    };
    if context.defend_damage().is_none() {
        run_iron_post_action_state(context, entry, protect, step);
        return;
    }
    if step <= 0 || protect <= 0 {
        return;
    }

    let damage = context.defend_damage().expect("iron state should run during POST_DEFEND");
    if damage <= 0 {
        context.set_defend_damage(0);
        return;
    }

    let caster = context.defend_caster().expect("iron state should receive incoming defend caster");
    let target = context.defend_target().expect("iron state should receive incoming defend target");
    if damage <= protect {
        let defended = context.last_update_was_plain_defense(target, caster);
        context.set_defend_damage(if defended { 0 } else { 1 });
        return;
    }

    let remaining = damage - protect;
    // legacy Iron.K 会在护盾被打破时立即注销全部 hook 并调用 update_states；
    // 不能留下 step=0 的 post-action 条目，否则它会在下次行动后再次刷新状态，
    // 把期间叠加的 Haste 倍率提前提交。
    context
        .clear_owner_state(entry.legacy_order_key)
        .expect("iron state payload should still exist");
    context.set_defend_damage(remaining);
    context.add_newline();
    context.add_update(crate::runtime::update::RunUpdate::new(
        "[1]的[铁壁]被打消了",
        caster.0 as usize,
        target.0 as usize,
        0,
    ));
}

/// 铁壁状态的 POST_ACTION 推进：原地递减步数，到期时延后清理状态并结算剩余免伤。
fn run_iron_post_action_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry, protect: i32, step: i32) {
    if step <= 0 {
        context
            .defer_owner_state_clear(entry.legacy_order_key)
            .expect("iron state payload should still exist");
        return;
    }

    let next_step = step - 1;
    if next_step > 0 {
        // legacy 这里只原地递减铁壁步数，不会调用 update_states；否则会提前同步疾走的待生效倍率。
        context
            .set_owner_state_payload_without_refresh(
                entry.legacy_order_key,
                StatePayload::Iron {
                    protect,
                    step: next_step,
                },
            )
            .expect("iron state payload should still exist");
        return;
    }

    context
        .set_owner_state_payload_without_refresh(
            entry.legacy_order_key,
            StatePayload::Iron {
                protect,
                step: next_step,
            },
        )
        .expect("iron state payload should still exist");
    context
        .defer_owner_state_clear(entry.legacy_order_key)
        .expect("iron state payload should still exist");
    context.adjust_owner_speed_points(-128).expect("iron state owner should still exist");
    context.add_newline();
    context.add_update(crate::runtime::update::RunUpdate::new(
        "[1]从[铁壁]中解除",
        context.owner_idx().0 as usize,
        context.owner_idx().0 as usize,
        0,
    ));
}

/// 实验体易伤状态的 POST_DEFEND：把本次受伤按 1.5 倍向上取整。
///
/// 层数只表示持续回合，不提高倍率；与旧实现一致，倍率恒为 1.5 且正数才生效。
pub fn run_testsubject_skull_fracture_post_defend_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::SkullFracture { stacks }) = context.owner_state_payload(entry.legacy_order_key) else {
        return;
    };
    if stacks <= 0 {
        return;
    }
    let Some(damage) = context.defend_damage() else {
        return;
    };
    if damage <= 0 {
        return;
    }
    let caster = context
        .defend_caster()
        .expect("runtime skull-fracture state should receive incoming defend caster");
    let target = context
        .defend_target()
        .expect("runtime skull-fracture state should receive incoming defend target");
    context.add_update(crate::runtime::update::RunUpdate::new(
        "[易伤]使伤害提高",
        caster.0 as usize,
        target.0 as usize,
        0,
    ));
    context.set_defend_damage(((f64::from(damage)) * 1.5).ceil() as i32);
}

/// 实验体易伤状态的 POST_ACTION：递减一层持续回合，归零时解除。
pub fn run_testsubject_skull_fracture_post_action_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::SkullFracture { stacks }) = context.owner_state_payload(entry.legacy_order_key) else {
        return;
    };
    let next_stacks = stacks - 1;
    context
        .set_owner_state_payload_without_refresh(entry.legacy_order_key, StatePayload::SkullFracture { stacks: next_stacks })
        .expect("skull-fracture state payload should still exist");
    if next_stacks > 0 {
        return;
    }
    context
        .defer_owner_state_clear(entry.legacy_order_key)
        .expect("skull-fracture state payload should still exist");
    if context.owner().is_some_and(|owner| owner.runtime.alive) {
        context.add_newline();
        context.add_update(crate::runtime::update::RunUpdate::new(
            "[1]从[易伤]中解除",
            context.owner_idx().0 as usize,
            context.owner_idx().0 as usize,
            0,
        ));
    }
}

/// 实验体易伤状态的统一入口：按当前 hook 分流到受伤放大或持续回合递减。
pub fn run_testsubject_skull_fracture_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    if context.hook().intersects(ProcMask::POST_DEFEND) {
        run_testsubject_skull_fracture_post_defend_state(context, entry);
    } else {
        run_testsubject_skull_fracture_post_action_state(context, entry);
    }
}

/// 实验体无实体状态的 POST_DEFEND：把受伤压到 1。
///
/// 与铁壁同为减伤，但没有持续步数：进入与解除完全由实验体自己的奇偶行动控制，
/// 因此既不能被打消，也不会在行动后自然到期。
pub fn run_testsubject_no_entity_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    if !context.hook().intersects(ProcMask::POST_DEFEND) {
        return;
    }
    if !matches!(
        context.owner_state_payload(entry.legacy_order_key),
        Some(StatePayload::TestSubjectNoEntity)
    ) {
        return;
    }
    let Some(damage) = context.defend_damage() else {
        return;
    };
    if damage > 1 {
        context.set_defend_damage(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lazy_boss_flare_and_timed_releases_follow_application_order() {
        for (raw, batch, expected) in [
            (
                "yuri@!\n\nlazy@!",
                6,
                &[
                    "[0]刷起了[抖音]",
                    "这回合什么也没做",
                    "[1]从[魅惑]中解除",
                    "[1][懒癌]发作",
                    "[1]受到[2]点伤害",
                ][..],
            ),
            (
                "yuri@!\n\nlazy@!",
                12,
                &[
                    "[0]发起攻击",
                    "[1]受到[2]点伤害",
                    "[1][懒癌]发作",
                    "[1]受到[2]点伤害",
                    "[1]被击倒了",
                ],
            ),
            (
                "lazy@!\n\nparity17a\nparity17b\n\nparity17c",
                12,
                &[
                    "[0]去[无畏契约]打排位了",
                    "这回合什么也没做",
                    "[1][懒癌]发作",
                    "[1]受到[2]点伤害",
                    "[1]从[疾走]中解除",
                ],
            ),
            (
                "yuri@!\n\nparity4a\n\nparity4b\n\nlazy@!\nparity4c",
                25,
                &[
                    "[0]使用[减速术]",
                    "[1]进入[迟缓]状态",
                    "[1][懒癌]发作",
                    "[1]受到[2]点伤害",
                    "[1]被击倒了",
                ],
            ),
            (
                "yuri@!\n\nparity4a\n\nparity4b\n\nlazy@!\nparity4c",
                32,
                &[
                    "[0]发起攻击",
                    "[0][回避]了攻击",
                    "[1][懒癌]发作",
                    "[1]受到[2]点伤害",
                    "[1]从[迟缓]中解除",
                ],
            ),
        ] {
            let mut runner = RuntimeRunner::new_from_namerena_raw(raw.to_owned()).unwrap();
            let frame = (0..2048)
                .filter_map(|_| runner.run_round().frame)
                .filter(|frame| frame.updates.updates.iter().any(|update| !update.message.trim().is_empty()))
                .nth(batch - 1)
                .expect("应有原版可见批次");
            let messages = frame
                .updates
                .updates
                .iter()
                .map(|update| update.message.as_ref())
                .filter(|message| !message.trim().is_empty())
                .collect::<Vec<_>>();
            assert_eq!(messages, expected, "{raw:?} 第 {batch} 批");
        }
    }

    #[test]
    fn saitama_releases_charm_after_nonterminal_hunger_retirement() {
        let mut runner = RuntimeRunner::new_from_namerena_raw("yuri@!\n\nsaitama@!\nparity18a\nparity18b".to_owned()).unwrap();
        let saitama = runner.input_groups[1][0].0 as usize;
        let (outcome, frame) = (0..2048)
            .map(|_| runner.run_round())
            .filter_map(|mut outcome| outcome.frame.take().map(|frame| (outcome, frame)))
            .nth(37)
            .expect("原版第 38 批应出现一拳超人退场");
        let messages = frame
            .updates
            .updates
            .iter()
            .filter(|update| !update.message.trim().is_empty())
            .map(|update| update.message.as_ref())
            .collect::<Vec<_>>();
        assert_eq!(messages, ["[0]觉得有点饿", "[0]离开了战场", "[1]从[魅惑]中解除"]);
        assert!(outcome.winner_team.is_none());
        let boss = runner.player_snapshot(saitama).unwrap();
        assert_eq!(boss.hp, 389);
        assert!(!boss.alive);
        let rng = &runner.runtime.rng;
        let digest = rng.main_val.iter().fold(2_166_136_261u32, |hash, &byte| {
            (hash ^ u32::from(byte)).wrapping_mul(16_777_619)
        });
        assert_eq!((rng.i, rng.j, digest), (30, 152, 817_248_267));
    }

    #[test]
    fn lazy_state_from_an_ordinary_player_preserves_legacy_release_order() {
        let mut runner = RuntimeRunner::new_from_namerena_raw("lazy\n\ncarrier".to_owned()).unwrap();
        let source = runner.input_groups[0][0];
        let carrier = runner.input_groups[1][0];
        let runtime = &mut runner.runtime;
        let mut updates = RunUpdates::new();
        // 扩展可由普通同名角色直接施加懒癌载荷，仍应使用旧顺序。
        assert!(runtime.infect_with_lazy_into(source, carrier, &mut updates));
        let haste = runtime.registry.state_id_by_export_name(DEFAULT_CORE_HASTE_STATE_EXPORT).unwrap();
        assert!(runtime.entities.get_mut(carrier).unwrap().states.add_entry(StateEntry::haste(
            PLAIN_HASTE_STATE_KEY,
            haste,
            2,
            1,
            SkillPriority(100),
        )));
        let plan = runtime.scheduler.state_hook_plan(&runtime.entities, carrier, ProcMask::POST_ACTION);
        let mut updates = RunUpdates::new();
        runtime.drain_state_hook_plan_into(&plan, &mut updates);
        let messages = updates
            .updates
            .iter()
            .map(|update| update.message.as_ref())
            .filter(|message| !message.trim().is_empty())
            .collect::<Vec<_>>();
        assert_eq!(&messages[..2], &["[1]从[疾走]中解除", "[1][懒癌]发作"]);
    }

    #[test]
    fn mixed_boss_infection_pre_action_executes_only_the_last_proposal() {
        use crate::cli_api::battle::{BattleOptions, battle_replay};

        for (raw, expected) in [
            (
                "lazy@!\nrecheck0a\n\ncovid@!\nrecheck0b",
                &[
                    ("[1]在家中自我隔离", 0),
                    ("[1][懒癌]发作", 0),
                    ("[1]受到[2]点伤害[s_dmg120]", 135),
                    ("[1][肺炎]发作", 0),
                    ("[1]受到[2]点伤害", 117),
                    ("[1]被击倒了", 50),
                    ("[1]回复体力[2]点", 0),
                ][..],
            ),
            (
                "lazy@!\nrecheck4a\n\ncovid@!\nrecheck4b",
                &[
                    ("[0]去[无畏契约]打排位了", 0),
                    ("这回合什么也没做", 0),
                    ("[1][懒癌]发作", 0),
                    ("[1]受到[2]点伤害", 51),
                ][..],
            ),
        ] {
            let replay = battle_replay(raw, BattleOptions::default()).unwrap();
            let visible: Vec<_> = replay.frames[9]
                .updates
                .iter()
                .filter(|update| !update.is_next_line)
                .map(|update| (update.message_template.as_str(), update.score))
                .collect();
            assert_eq!(visible, expected, "{raw:?}");
        }
    }
}

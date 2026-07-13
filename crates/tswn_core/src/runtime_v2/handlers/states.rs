use super::*;

pub fn run_charge_post_action_skill(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    context.tick_owner_charge_post_action().expect("charge post_action owner should exist");
}

pub fn run_accumulate_skill(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    if !context.activate_owner_accumulate_runtime().expect("accumulate owner should exist") {
        return;
    }

    let owner = context.owner_idx();
    context.add_update(crate::engine::update::RunUpdate::new(
        "[0]开始[聚气]",
        owner.0 as usize,
        owner.0 as usize,
        1,
    ));
    context.add_update(crate::engine::update::RunUpdate::new(
        "[0]攻击力上升",
        owner.0 as usize,
        owner.0 as usize,
        0,
    ));
}

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
        context.add_update(crate::engine::update::RunUpdate::new(
            "[诅咒]使伤害加倍",
            caster.0 as usize,
            target.0 as usize,
            0,
        ));
        context.set_defend_damage(damage * multiply);
    }
}

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

    context.add_update(crate::engine::update::RunUpdate::new(
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
            .set_owner_state_payload(
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
        .clear_owner_state(entry.legacy_order_key)
        .expect("poison state payload should still exist");
}

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

pub fn run_slow_post_action_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::Slow { step }) = context.owner_state_payload(entry.legacy_order_key) else {
        return;
    };
    run_timed_release_post_action_state(context, entry, StatePayload::Slow { step: step - 1 }, step, "[1]从[迟缓]中解除");
}

pub fn run_covid_infection_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::CovidInfection {
        mut entries,
        mut mutation_set,
        mut recovered,
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
            .expect("runtime_v2 covid PRE_ACTION state must receive the action smart roll");
        for infection in &mut entries {
            if context.rng_next_u8() < 64 {
                let mutation = context.rng_r127() as i32;
                infection.mutation = mutation;
                if !mutation_set.contains(&mutation) {
                    mutation_set.push(mutation);
                }
            }
        }

        let last_idx = entries.len() - 1;
        let boss = entries[last_idx].boss;
        let mutation = entries[last_idx].mutation;
        let days = entries[last_idx].days;
        let all_alive = context.flat_alive().expect("runtime_v2 covid state must read the complete alive list");

        let boss_team = context.entity(boss).expect("runtime_v2 covid boss must exist").runtime.team;
        let skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                (context
                    .entity(*candidate)
                    .expect("runtime_v2 covid alive candidate must exist")
                    .runtime
                    .team
                    == boss_team)
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
                let candidate_entity = context.entity(candidate).expect("runtime_v2 covid spread candidate must exist");
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
                let owner_team = context.owner().expect("runtime_v2 covid owner must exist").runtime.team;
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
                    .set_owner_state_payload(
                        entry.legacy_order_key,
                        StatePayload::CovidInfection {
                            entries,
                            mutation_set,
                            recovered,
                        },
                    )
                    .expect("runtime_v2 covid state payload must still exist");
                context.push(effect);
                context.intercept_action();
                return;
            }
        }

        entries[last_idx].days += i32::from(context.rng_next_u8() & 3);
        let message = if entries[last_idx].days > 2 {
            "[1]在重症监护室无法行动"
        } else {
            "[1]在家中自我隔离"
        };
        context.add_update(crate::engine::update::RunUpdate::new(
            message,
            boss.0 as usize,
            owner.0 as usize,
            0,
        ));
        context
            .set_owner_state_payload(
                entry.legacy_order_key,
                StatePayload::CovidInfection {
                    entries,
                    mutation_set,
                    recovered,
                },
            )
            .expect("runtime_v2 covid state payload must still exist");
        context.intercept_action();
        return;
    }

    if context.hook().intersects(ProcMask::POST_ACTION) {
        let owner = context.owner_idx();
        let alive = context.owner().is_some_and(|entity| entity.runtime.alive);
        for infection in &entries {
            if alive && infection.days > 1 {
                context.push(QueuedEffect::CovidPneumonia {
                    owner,
                    boss: infection.boss,
                    mutation: infection.mutation,
                });
            }
        }
        entries.retain(|infection| infection.days <= 6);
        if entries.is_empty() && !recovered {
            recovered = true;
        }
        context
            .set_owner_state_payload(
                entry.legacy_order_key,
                StatePayload::CovidInfection {
                    entries,
                    mutation_set,
                    recovered,
                },
            )
            .expect("runtime_v2 covid state payload must still exist");
    }
}

pub fn run_lazy_infection_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::LazyInfection { boss }) = context.owner_state_payload(entry.legacy_order_key) else {
        return;
    };
    if context.hook().intersects(ProcMask::PRE_ACTION) {
        if context.rng_next_u8() >= 128 {
            return;
        }
        let smart = context
            .action_smart()
            .expect("runtime_v2 lazy PRE_ACTION state must receive the action smart roll");
        let all_alive = context.flat_alive().expect("runtime_v2 lazy state must read the complete alive list");
        let boss_team = context.entity(boss).expect("runtime_v2 lazy boss must exist").runtime.team;
        let skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                (context.entity(*candidate).expect("runtime_v2 lazy candidate must exist").runtime.team == boss_team)
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
        let activity = match context.rng_next_u8() {
            0..=49 => "Steam",
            50..=99 => "守望先锋",
            100..=149 => "文明6",
            150..=189 => "英雄联盟",
            190..=229 => "微博",
            _ => "朋友圈",
        };
        let owner = context.owner_idx();
        let owner_name = context.owner().expect("runtime_v2 lazy owner must exist").template.display_name.clone();
        context.add_update(crate::engine::update::RunUpdate::new(
            format!("{owner_name}打开了{activity}, 这回合什么也没做"),
            owner.0 as usize,
            owner.0 as usize,
            0,
        ));
        context.intercept_action();
        return;
    }

    if context.hook().intersects(ProcMask::POST_ACTION) && context.entity(boss).is_ok_and(|boss_entity| boss_entity.runtime.alive)
    {
        context.push(QueuedEffect::LazyFlare {
            owner: context.owner_idx(),
            boss,
        });
    }
}

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
    let damage = context.defend_damage().expect("runtime_v2 saitama state requires POST_DEFEND damage");
    let caster = context.defend_caster().expect("runtime_v2 saitama state requires a damage caster");
    damages += damage;
    let caster_entity = context
        .entity(caster)
        .unwrap_or_else(|error| panic!("runtime_v2 saitama caster lookup failed: {error:?}"));
    let hitter = if caster_entity.runtime.is_minion() && caster_entity.runtime.owner != caster {
        if !minions.contains(&caster) {
            minions.push(caster);
        }
        caster_entity.runtime.owner
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
        .expect("runtime_v2 saitama state owner must exist");
    context.set_defend_damage(damage / 100);
}

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
    context
        .defer_owner_state_clear(entry.legacy_order_key)
        .expect("timed state payload should still exist");
    let alive = context.owner().map(|owner| owner.runtime.alive).unwrap_or(false);
    if alive {
        context.add_newline();
        context.add_update(crate::engine::update::RunUpdate::new(
            release_message,
            context.owner_idx().0 as usize,
            context.owner_idx().0 as usize,
            0,
        ));
    }
}

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
        let defended = context
            .last_non_newline_update()
            .map(|update| {
                update.message == "[0][防御]" && update.caster == target.0 as usize && update.target == caster.0 as usize
            })
            .unwrap_or(false);
        context.set_defend_damage(if defended { 0 } else { 1 });
        return;
    }

    let remaining = damage - protect;
    context
        .set_owner_state_payload(entry.legacy_order_key, StatePayload::Iron { protect: 0, step: 0 })
        .expect("iron state payload should still exist");
    context.set_defend_damage(remaining);
    context.add_newline();
    context.add_update(crate::engine::update::RunUpdate::new(
        "[1]的[铁壁]被打消了",
        caster.0 as usize,
        target.0 as usize,
        0,
    ));
}

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
    context.add_update(crate::engine::update::RunUpdate::new(
        "[1]从[铁壁]中解除",
        context.owner_idx().0 as usize,
        context.owner_idx().0 as usize,
        0,
    ));
}

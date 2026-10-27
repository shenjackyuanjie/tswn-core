use super::*;

fn weapon_registry() -> (ExtensionRegistry, SkillId, SkillId, SkillId) {
    let mut builder = ExtensionRegistryBuilder::default();
    let s11 = builder
        .register_skill(
            "core",
            "weapon-s11",
            DEFAULT_CORE_WEAPON_S11_SKILL_EXPORT,
            TargetPolicy::None,
            SkillPriority(44),
        )
        .expect("s11 weapon skill should register");
    let death_note = builder
        .register_skill(
            "core",
            "weapon-death-note",
            DEFAULT_CORE_WEAPON_DEATH_NOTE_SKILL_EXPORT,
            TargetPolicy::None,
            SkillPriority(45),
        )
        .expect("death note weapon skill should register");
    let xuan_yue = builder
        .register_skill(
            "core",
            "weapon-xuan-yue",
            DEFAULT_CORE_WEAPON_XUAN_YUE_COUNTER_SKILL_EXPORT,
            TargetPolicy::None,
            SkillPriority(46),
        )
        .expect("xuan yue weapon skill should register");
    (builder.build(), s11, death_note, xuan_yue)
}

/// 前移 RNG 到第一个满足 `accept` 抽取序列的位置，返回闭包给出的期望值。
fn tune_weapon_rng<T>(runtime: &mut CombatRuntime, mut accept: impl FnMut(&mut RC4) -> Option<T>) -> T {
    let mut probe = runtime.rng.clone();
    for _ in 0..50_000 {
        let mut candidate = probe.clone();
        if let Some(value) = accept(&mut candidate) {
            runtime.rng = probe;
            return value;
        }
        probe.next_u8();
    }
    panic!("no rng offset satisfies the weapon test predicate");
}

fn non_next_line_messages(updates: &RunUpdates) -> Vec<&str> {
    updates
        .updates
        .iter()
        .filter(|update| !matches!(update.update_type, crate::runtime::update::UpdateType::NextLine))
        .map(|update| update.message.as_ref())
        .collect()
}

fn clone_build_with_attrs(attrs: [u32; 8]) -> CloneBuildData {
    CloneBuildData {
        attrs,
        weapon_attr_bonus: [0; 8],
        name_factor_bits: 0.0_f64.to_bits(),
        child_name_factor_bits: 0.0_f64.to_bits(),
        adjustments: CloneStatAdjustments {
            max_hp: 0,
            attack: 0,
            magic: 0,
            wisdom: 0,
            speed: 0,
            defense: 0,
            resistance: 0,
            agility: 0,
            at_boost_delta_bits: 0.0_f64.to_bits(),
            attr_sum: 0,
            atk_sum: 0,
            attract_delta_bits: 0.0_f64.to_bits(),
        },
        score_skill_boost_plan: None,
    }
}

#[test]
fn weapon_s11_loadout_initializes_runtime_state_and_selects_empty_targets() {
    let (registry, s11, _, _) = weapon_registry();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 100, 10).with_skill_loadout(SkillLoadout::from_skill_levels([(s11, 31)])),
            PlayerTemplate::new(2, "right", 1, 100, 10),
        ],
        registry,
    ));
    let s11_runtime = runtime
        .entities
        .get(EntityIdx(0))
        .unwrap()
        .runtime
        .weapon_s11
        .expect("s11 loadout should build runtime state");
    assert_eq!(s11_runtime.level, 31);
    assert_eq!(s11_runtime.durability, 3);

    let mut expected_rng = runtime.rng.clone();
    let _ = expected_rng.r63();
    let prepared = runtime
        .scan_plain_action_skill_probabilities(EntityIdx(0), false)
        .expect("s11 should be selected");
    assert_eq!(prepared.selected.skill, BuiltinActiveSkill::WeaponS11);
    assert!(prepared.targets.is_empty());
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn weapon_s11_level_zero_never_scans_after_break() {
    let (registry, s11, _, _) = weapon_registry();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 100, 10).with_skill_loadout(SkillLoadout::from_skill_levels([(s11, 31)]))],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.weapon_s11.as_mut().unwrap().level = 0;
    let expected_rng = runtime.rng.clone();
    assert!(runtime.scan_plain_action_skill_probabilities(EntityIdx(0), false).is_none());
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn weapon_s11_attr_branch_raises_attack_and_charges_move_points() {
    let (registry, s11, _, _) = weapon_registry();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 1000, 10).with_skill_loadout(SkillLoadout::from_skill_levels([(s11, 31)])),
            PlayerTemplate::new(2, "right", 1, 1000, 10),
        ],
        registry,
    ));
    tune_weapon_rng(&mut runtime, |rng| {
        if rng.c25() {
            return None;
        }
        if rng.next_i32(7) != 0 {
            return None;
        }
        if rng.next_u8() & 31 != 2 {
            return None;
        }
        (rng.r3() == 1).then_some(())
    });

    let mut updates = RunUpdates::new();
    runtime.drain_plain_weapon_s11_skill_into(EntityIdx(0), &mut updates);

    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(owner.template.attack, 18);
    assert_eq!(owner.runtime.attack, 18);
    assert_eq!(owner.runtime.move_state.speed_points, 1024);
    let s11_runtime = owner.runtime.weapon_s11.unwrap();
    assert_eq!(s11_runtime.level, 31);
    assert_eq!(s11_runtime.durability, 2);
    assert_eq!(
        non_next_line_messages(&updates),
        vec!["[0]在促销日[购买]了武器", "[攻]增加了[2]点"]
    );
    assert_eq!(updates.updates[1].param, Some(8));
    assert_eq!(updates.updates[1].score, 8);
}

#[test]
fn weapon_s11_attr_branch_rebuilds_stats_from_clone_build() {
    let (registry, s11, _, _) = weapon_registry();
    let mut template =
        PlayerTemplate::new(1, "left", 0, 1000, 10).with_skill_loadout(SkillLoadout::from_skill_levels([(s11, 31)]));
    let clone_build = clone_build_with_attrs([128, 0, 0, 0, 0, 0, 0, 100]);
    let mut expected_build = clone_build.clone();
    expected_build.attrs[0] += 8;
    let expected_stats = expected_build.derive_stats();
    template.clone_build = Some(clone_build);
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![template, PlayerTemplate::new(2, "right", 1, 1000, 10)],
        registry,
    ));
    tune_weapon_rng(&mut runtime, |rng| {
        if rng.c25() {
            return None;
        }
        if rng.next_i32(7) != 0 {
            return None;
        }
        if rng.next_u8() & 31 != 2 {
            return None;
        }
        (rng.r3() == 1).then_some(())
    });

    let mut updates = RunUpdates::new();
    runtime.drain_plain_weapon_s11_skill_into(EntityIdx(0), &mut updates);

    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    let build = owner.template.clone_build.as_ref().expect("clone build should survive the attr raise");
    assert_eq!(build.attrs[0], 136);
    assert_eq!(owner.template.attack, expected_stats.attack);
    assert_eq!(owner.template.max_hp, expected_stats.max_hp);
    assert_eq!(owner.runtime.attr_sum, expected_stats.attr_sum);
}

#[test]
fn weapon_s11_break_branch_self_damages_and_rewrites_level() {
    let (registry, s11, _, _) = weapon_registry();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 1000, 10).with_skill_loadout(SkillLoadout::from_skill_levels([(s11, 31)]))],
        registry,
    ));
    let damage = tune_weapon_rng(&mut runtime, |rng| {
        if !rng.c25() {
            return None;
        }
        if rng.r3() != 3 {
            return None;
        }
        Some(rng.r31() as i32 + 16)
    });

    let mut updates = RunUpdates::new();
    runtime.drain_plain_weapon_s11_skill_into(EntityIdx(0), &mut updates);

    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    let s11_runtime = owner.runtime.weapon_s11.unwrap();
    assert_eq!(s11_runtime.level, 1);
    assert_eq!(s11_runtime.durability, -1);
    assert_eq!(owner.runtime.hp, 1000 - damage);
    assert_eq!(owner.runtime.move_state.speed_points, 1024);
    assert_eq!(
        non_next_line_messages(&updates),
        vec![
            "[0]在促销日[购买]了武器",
            "但是并没有什么用",
            "[0]信用卡刷爆",
            "[0]砍下了自己的右手",
            "[1]受到[2]点伤害",
        ]
    );
    assert_eq!(updates.updates.last().unwrap().score, damage as u32);
}

#[test]
fn weapon_s11_break_below_twenty_uses_left_hand_and_disables_weapon() {
    let (registry, s11, _, _) = weapon_registry();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 1000, 10).with_skill_loadout(SkillLoadout::from_skill_levels([(s11, 31)]))],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.weapon_s11.as_mut().unwrap().level = 5;
    tune_weapon_rng(&mut runtime, |rng| {
        if !rng.c25() {
            return None;
        }
        if rng.r3() != 3 {
            return None;
        }
        let _ = rng.r31();
        Some(())
    });

    let mut updates = RunUpdates::new();
    runtime.drain_plain_weapon_s11_skill_into(EntityIdx(0), &mut updates);

    let s11_runtime = runtime.entities.get(EntityIdx(0)).unwrap().runtime.weapon_s11.unwrap();
    assert_eq!(s11_runtime.level, 0);
    assert!(non_next_line_messages(&updates).contains(&"[0]砍下了自己的左手"));

    let expected_rng = runtime.rng.clone();
    assert!(runtime.scan_plain_action_skill_probabilities(EntityIdx(0), false).is_none());
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn weapon_death_note_scan_tracks_live_mark_without_smart_rng() {
    let (registry, _, death_note, _) = weapon_registry();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 1000, 10).with_skill_loadout(SkillLoadout::from_skill_levels([(death_note, 1)])),
            PlayerTemplate::new(2, "right", 1, 1000, 10),
            PlayerTemplate::new(3, "ally", 0, 1000, 10),
        ],
        registry,
    ));
    let before = runtime.rng.clone();
    assert!(runtime.scan_plain_action_skill_probabilities(EntityIdx(0), false).is_none());
    assert_rng_state_eq(&runtime.rng, &before);

    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 0;
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.death_note_mark = Some(EntityIdx(1));
    assert!(runtime.scan_plain_action_skill_probabilities(EntityIdx(0), true).is_none());
    assert_rng_state_eq(&runtime.rng, &before);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 1000;

    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.death_note_mark = Some(EntityIdx(2));
    assert!(runtime.scan_plain_action_skill_probabilities(EntityIdx(0), true).is_none());
    assert_rng_state_eq(&runtime.rng, &before);

    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.death_note_mark = Some(EntityIdx(1));
    let prepared = runtime
        .scan_plain_action_skill_probabilities(EntityIdx(0), true)
        .expect("enemy mark should arm the death note");
    assert_eq!(prepared.selected.skill, BuiltinActiveSkill::WeaponDeathNote);
    assert!(prepared.targets.is_empty());
    assert_rng_state_eq(&runtime.rng, &before);

    tune_weapon_rng(&mut runtime, |rng| rng.c50().then_some(()));
    let mut expected_rng = runtime.rng.clone();
    assert!(expected_rng.c50());
    assert!(runtime.scan_plain_action_skill_probabilities(EntityIdx(0), false).is_some());
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn weapon_death_note_execution_damages_mark_and_zeroes_mp() {
    let (registry, _, death_note, _) = weapon_registry();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 1000, 10)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(death_note, 1)]))
                .with_speed(1000)
                .with_magic_point(500),
            PlayerTemplate::new(2, "right", 1, 40, 10),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.death_note_mark = Some(EntityIdx(1));

    let mut updates = RunUpdates::new();
    runtime.drain_plain_weapon_death_note_skill_into(EntityIdx(0), &mut updates);

    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(owner.template.speed, -24);
    assert_eq!(owner.runtime.speed, -24);
    assert_eq!(owner.runtime.magic_point, 0);
    assert!(owner.runtime.death_note_mark.is_none());
    let victim = runtime.entities.get(EntityIdx(1)).unwrap();
    assert_eq!(victim.runtime.hp, 0);
    assert!(!victim.runtime.alive);
    assert_eq!(
        non_next_line_messages(&updates),
        vec!["[0]在[死亡笔记]写下[1]的名字", "[1]受到[2]点伤害", "[1]被击倒了"]
    );
    assert_eq!(updates.updates[0].score, 20);
}

#[test]
fn weapon_xuan_yue_counter_stays_out_of_action_scan() {
    let (registry, _, _, xuan_yue) = weapon_registry();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 100, 10).with_skill_loadout(SkillLoadout::from_skill_levels([(xuan_yue, 1)]))],
        registry,
    ));
    let expected_rng = runtime.rng.clone();
    assert!(runtime.scan_plain_action_skill_probabilities(EntityIdx(0), false).is_none());
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn weapon_death_note_post_damage_marks_the_attacker_after_dodge() {
    let (registry, _, death_note, _) = weapon_registry();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 1000, 10).with_skill_loadout(SkillLoadout::from_skill_levels([(death_note, 1)])),
            PlayerTemplate::new(2, "right", 1, 1000, 10),
        ],
        registry,
    ));
    let (accuracy, dodge_value) = {
        let caster = runtime.entities.get(EntityIdx(1)).unwrap();
        let owner = runtime.entities.get(EntityIdx(0)).unwrap();
        (
            caster.runtime.wisdom + caster.runtime.resistance,
            owner.runtime.wisdom + owner.runtime.magic,
        )
    };
    tune_weapon_rng(&mut runtime, |rng| {
        PlayerRuntime::dodge(accuracy, dodge_value, rng).then_some(())
    });

    let mut expected_rng = runtime.rng.clone();
    let _ = expected_rng.next_u8();
    let mut updates = RunUpdates::new();
    runtime.drain_plain_post_damage_skill_chain_into(EntityIdx(0), 10, EntityIdx(1), &mut updates);

    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.death_note_mark,
        Some(EntityIdx(1))
    );
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn weapon_death_note_post_damage_failed_dodge_keeps_existing_mark() {
    let (registry, _, death_note, _) = weapon_registry();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 1000, 10).with_skill_loadout(SkillLoadout::from_skill_levels([(death_note, 1)])),
            PlayerTemplate::new(2, "right", 1, 1000, 10),
            PlayerTemplate::new(3, "right2", 1, 1000, 10),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.death_note_mark = Some(EntityIdx(2));
    let (accuracy, dodge_value) = {
        let caster = runtime.entities.get(EntityIdx(1)).unwrap();
        let owner = runtime.entities.get(EntityIdx(0)).unwrap();
        (
            caster.runtime.wisdom + caster.runtime.resistance,
            owner.runtime.wisdom + owner.runtime.magic,
        )
    };
    tune_weapon_rng(&mut runtime, |rng| {
        (!PlayerRuntime::dodge(accuracy, dodge_value, rng)).then_some(())
    });

    let mut expected_rng = runtime.rng.clone();
    let _ = expected_rng.next_u8();
    let mut updates = RunUpdates::new();
    runtime.drain_plain_post_damage_skill_chain_into(EntityIdx(0), 10, EntityIdx(1), &mut updates);

    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.death_note_mark,
        Some(EntityIdx(2))
    );
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn weapon_death_note_post_damage_ignores_self_damage_without_rng() {
    let (registry, _, death_note, _) = weapon_registry();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 1000, 10).with_skill_loadout(SkillLoadout::from_skill_levels([(death_note, 1)]))],
        registry,
    ));
    let before = runtime.rng.clone();
    let mut updates = RunUpdates::new();
    runtime.drain_plain_post_damage_skill_chain_into(EntityIdx(0), 10, EntityIdx(0), &mut updates);

    assert!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.death_note_mark.is_none());
    assert_rng_state_eq(&runtime.rng, &before);
}

#[test]
fn weapon_xuan_yue_post_damage_registers_latest_caster_without_rng() {
    let (registry, _, _, xuan_yue) = weapon_registry();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 1000, 10).with_skill_loadout(SkillLoadout::from_skill_levels([(xuan_yue, 1)])),
            PlayerTemplate::new(2, "right", 1, 1000, 10),
            PlayerTemplate::new(3, "ally", 0, 1000, 10),
            PlayerTemplate::new(4, "right2", 1, 1000, 10),
        ],
        registry,
    ));
    let before = runtime.rng.clone();
    let mut updates = RunUpdates::new();

    runtime.drain_plain_post_damage_skill_chain_into(EntityIdx(0), 10, EntityIdx(1), &mut updates);
    {
        let counter = &runtime.entities.get(EntityIdx(0)).unwrap().runtime.counter_xuan_yue;
        assert!(counter.pending);
        assert_eq!(counter.last_target, Some(EntityIdx(1)));
        assert_eq!(counter.last_updates_id, Some(updates.id));
    }
    assert_eq!(updates.on_update_end.len(), 1);
    assert_rng_state_eq(&runtime.rng, &before);

    runtime.drain_plain_post_damage_skill_chain_into(EntityIdx(0), 10, EntityIdx(3), &mut updates);
    {
        let counter = &runtime.entities.get(EntityIdx(0)).unwrap().runtime.counter_xuan_yue;
        assert_eq!(counter.last_target, Some(EntityIdx(3)));
    }
    assert_eq!(updates.on_update_end.len(), 1);
    assert_rng_state_eq(&runtime.rng, &before);

    runtime.drain_plain_post_damage_skill_chain_into(EntityIdx(0), 10, EntityIdx(2), &mut updates);
    {
        let counter = &runtime.entities.get(EntityIdx(0)).unwrap().runtime.counter_xuan_yue;
        assert_eq!(counter.last_target, Some(EntityIdx(3)));
    }
    assert_eq!(updates.on_update_end.len(), 1);
    assert_rng_state_eq(&runtime.rng, &before);
}

#[test]
fn weapon_xuan_yue_update_end_attacks_the_recorded_caster() {
    let (registry, _, _, xuan_yue) = weapon_registry();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 1000, 10)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(xuan_yue, 1)]))
                .with_magic_point(100),
            PlayerTemplate::new(2, "right", 1, 1000, 10),
        ],
        registry,
    ));
    let mut updates = RunUpdates::new();
    runtime.drain_plain_post_damage_skill_chain_into(EntityIdx(0), 10, EntityIdx(1), &mut updates);

    runtime.drain_plain_update_end_into(&mut updates);

    {
        let counter = &runtime.entities.get(EntityIdx(0)).unwrap().runtime.counter_xuan_yue;
        assert!(!counter.pending);
        assert_eq!(counter.last_updates_id, None);
        assert_eq!(counter.last_target, None);
    }
    assert!(updates.on_update_end.is_empty());
    let counter_update = updates
        .updates
        .iter()
        .find(|update| update.message == "[0]发起[反击][s_counter]")
        .expect("xuan yue counter should attack the recorded caster");
    assert_eq!((counter_update.caster, counter_update.target), (0, 1));
}

#[test]
fn weapon_xuan_yue_update_end_skips_frozen_owner_without_mp_rng() {
    let (registry, _, _, xuan_yue) = weapon_registry();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 1000, 10)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(xuan_yue, 1)]))
                .with_magic_point(100),
            PlayerTemplate::new(2, "right", 1, 1000, 10),
        ],
        registry,
    ));
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 2));
    let mut updates = RunUpdates::new();
    runtime.drain_plain_post_damage_skill_chain_into(EntityIdx(0), 10, EntityIdx(1), &mut updates);

    let before = runtime.rng.clone();
    runtime.drain_plain_update_end_into(&mut updates);

    assert_rng_state_eq(&runtime.rng, &before);
    assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.counter_xuan_yue.pending);
    assert!(!updates.updates.iter().any(|update| update.message == "[0]发起[反击][s_counter]"));
}

#[test]
fn weapon_post_damage_plan_combines_death_note_and_xuan_yue() {
    let (registry, _, death_note, xuan_yue) = weapon_registry();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 1000, 10)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(death_note, 1), (xuan_yue, 1)])),
            PlayerTemplate::new(2, "right", 1, 1000, 10),
        ],
        registry,
    ));
    let (accuracy, dodge_value) = {
        let caster = runtime.entities.get(EntityIdx(1)).unwrap();
        let owner = runtime.entities.get(EntityIdx(0)).unwrap();
        (
            caster.runtime.wisdom + caster.runtime.resistance,
            owner.runtime.wisdom + owner.runtime.magic,
        )
    };
    tune_weapon_rng(&mut runtime, |rng| {
        PlayerRuntime::dodge(accuracy, dodge_value, rng).then_some(())
    });

    let mut updates = RunUpdates::new();
    runtime.drain_plain_post_damage_skill_chain_into(EntityIdx(0), 10, EntityIdx(1), &mut updates);

    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(owner.runtime.death_note_mark, Some(EntityIdx(1)));
    assert!(owner.runtime.counter_xuan_yue.pending);
    assert_eq!(owner.runtime.counter_xuan_yue.last_target, Some(EntityIdx(1)));
    assert_eq!(updates.on_update_end.len(), 1);
}

#[test]
fn gui_yue_permanent_charge_stays_active_without_attack_boost() {
    let registry = ExtensionRegistryBuilder::default().build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "gui", 0, 100, 10).with_permanent_charge(true)],
        registry,
    ));
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert!(owner.runtime.charge.active);
    assert!(owner.runtime.charge.permanent);
    assert_eq!(owner.runtime.at_boost_millionths, 1_000_000);

    assert!(!runtime.entities.get_mut(EntityIdx(0)).unwrap().tick_charge_post_action());
    assert!(!runtime.entities.get_mut(EntityIdx(0)).unwrap().clear_charge_runtime());
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert!(owner.runtime.charge.active);

    assert!(runtime.entities.get_mut(EntityIdx(0)).unwrap().activate_accumulate_runtime());
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(owner.runtime.accumulate.charge_bonus(), 1.0);
    // 聚气自身的 +0.7 与常驻 charge 的 +1.0 相加；桂月标记不提供 charge 的 ×3。
    assert_eq!(owner.runtime.at_boost_millionths, 2_700_000);
}

use super::*;

fn derived_stats(speed: i32) -> CloneDerivedStats {
    CloneDerivedStats {
        max_hp: 157,
        attack: 26,
        magic: 12,
        wisdom: 43,
        speed,
        defense: 42,
        resistance: 41,
        agility: 14,
        at_boost_bits: DEFAULT_AT_BOOST_BITS,
        at_boost_millionths: DEFAULT_AT_BOOST_MILLIONTHS,
        attr_sum: 219,
        atk_sum: 49,
        attract_bits: 32768.0_f64.to_bits(),
    }
}

#[test]
fn applying_derived_stats_replays_upgrade_without_changing_template_base() {
    let mut arena = EntityArena::from_templates(vec![PlayerTemplate::new(1, "owner", 0, 200, 40).with_speed(232)]);
    let owner = arena.get_mut(EntityIdx(0)).unwrap();

    assert!(owner.activate_upgrade_runtime());
    assert_eq!(owner.runtime.speed, 252);
    assert_eq!(owner.runtime.move_state.speed_points, 400);

    owner.apply_derived_stats(derived_stats(201));

    assert_eq!(owner.template.speed, 201);
    assert_eq!(owner.runtime.speed, 221);
    assert_eq!(owner.template.attack, 26);
    assert_eq!(owner.runtime.attack, 56);
    assert_eq!(owner.runtime.move_state.speed_points, 400);
}

#[test]
fn applying_derived_stats_rebuilds_hidden_view_and_snapshot() {
    let mut arena = EntityArena::from_templates(vec![
        PlayerTemplate::new(1, "owner", 0, 200, 40)
            .with_speed(232)
            .with_def_res(50, 60)
            .with_agility(70),
    ]);
    let owner = arena.get_mut(EntityIdx(0)).unwrap();
    assert!(owner.activate_upgrade_runtime());
    owner.runtime.hide = Some(HideRuntime {
        level: 70,
        attract_bits: owner.runtime.attract_bits,
        agility: owner.runtime.agility,
        defense: owner.runtime.defense,
        resistance: owner.runtime.resistance,
    });

    owner.apply_derived_stats(derived_stats(201));

    let hidden = owner.runtime.hide.unwrap();
    assert_eq!(hidden.agility, 44);
    assert_eq!(hidden.defense, 72);
    assert_eq!(hidden.resistance, 71);
    assert_eq!(f64::from_bits(hidden.attract_bits), 32768.0);
    assert_eq!(owner.runtime.agility, 51);
    assert_eq!(owner.runtime.defense, 79);
    assert_eq!(owner.runtime.resistance, 78);
    assert_eq!(owner.runtime.attract(), 3276.8);
}

#[test]
fn clearing_positive_runtime_reverts_upgrade_and_emits_legacy_message() {
    let mut arena = EntityArena::from_templates(vec![PlayerTemplate::new(1, "owner", 0, 200, 40).with_speed(232)]);
    let owner = arena.get_mut(EntityIdx(0)).unwrap();
    assert!(owner.activate_upgrade_runtime());

    let messages = owner.clear_positive_runtime_messages();

    assert_eq!(messages, vec![(500, "[1]的[垂死]属性被打消")]);
    assert!(!owner.runtime.upgrade_active);
    assert_eq!(owner.runtime.attack, 40);
    assert_eq!(owner.runtime.speed, 232);
}

#[test]
fn refreshing_runtime_replays_curse_atk_sum() {
    let mut arena = EntityArena::from_templates(vec![
        PlayerTemplate::new(1, "owner", 0, 200, 40).with_target_score_stats(100, 17, 32768.0),
    ]);
    let owner = arena.get_mut(EntityIdx(0)).unwrap();
    owner.states.add_entry(StateEntry::curse(77, StateId(12), 42, 2, SkillPriority(100)));

    owner.refresh_runtime_stats_from_template();

    assert_eq!(owner.runtime.atk_sum, 68);
}

#[test]
fn clone_child_uses_separate_name_factor_without_changing_owner_decay() {
    let attrs = [39, 39, 43, 43, 37, 36, 42, 149];
    let owner_raw = CloneBuildData::derive_raw(attrs, 0.0);
    let status = PlayerStats {
        max_hp: owner_raw.max_hp,
        attack: owner_raw.attack,
        magic: owner_raw.magic,
        wisdom: owner_raw.wisdom,
        speed: owner_raw.speed,
        defense: owner_raw.defense,
        resistance: owner_raw.resistance,
        agility: owner_raw.agility,
        at_boost: f64::from_bits(owner_raw.at_boost_bits),
        attr_sum: owner_raw.attr_sum,
        atk_sum: owner_raw.atk_sum,
        attract: f64::from_bits(owner_raw.attract_bits),
        ..PlayerStats::default()
    };
    let build = CloneBuildData::from_legacy(attrs, [0; 8], 0.0, &status).with_child_name_factor(-4.909645111040874);

    assert_eq!(build.derive_stats().speed, 203);

    let child = build.child();
    assert_eq!(child.name_factor(), -4.909645111040874);
    assert_eq!(child.derive_stats().speed, 205);
}

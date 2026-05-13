use super::*;

#[test]
/// 测试根据原始输入创建 Player
fn player_raw_new() {
    let storage = Storage::new_arc();

    let player = Player::new_from_namerena_raw("mario".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.name, "mario");
    assert_eq!(player.team, None);
    assert_eq!(player.weapon, None);
    assert_eq!(player.player_type, PlayerType::Normal);

    let player = Player::new_from_namerena_raw("mario@red".to_string(), storage.clone());
    let player = player.unwrap();
    println!("{}", player);
    assert_eq!(player.name, "mario");
    assert_eq!(player.team, Some("red".to_string()));
    assert_eq!(player.weapon, None);
    assert_eq!(player.player_type, PlayerType::Normal);

    let player = Player::new_from_namerena_raw("mario+fire".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.name, "mario");
    assert_eq!(player.team, None);
    assert_eq!(player.weapon, Some("fire".to_string()));
    assert_eq!(player.player_type, PlayerType::Normal);

    let player = Player::new_from_namerena_raw("mario+fire+diy{xxxx}".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.name, "mario");
    assert_eq!(player.team, None);
    assert_eq!(player.weapon, Some("fire+diy{xxxx}".to_string()));
    assert_eq!(player.player_type, PlayerType::Normal);

    let player = Player::new_from_namerena_raw("mario@red+fire".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.name, "mario");
    assert_eq!(player.team, Some("red".to_string()));
    assert_eq!(player.weapon, Some("fire".to_string()));
    assert_eq!(player.player_type, PlayerType::Normal);

    let player = Player::new_from_namerena_raw("mario@red+fire+diy{xxxx}".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.name, "mario");
    assert_eq!(player.team, Some("red".to_string()));
    assert_eq!(player.weapon, Some("fire+diy{xxxx}".to_string()));
    assert_eq!(player.player_type, PlayerType::Normal);
}

#[test]
/// 测试 overlay 解析：compact DIY 格式和 JSON object 格式。
///
/// 验证：
/// - `diy[attrs]{skills}` 解析后属性值正确减 36；
/// - `ol:{...}` 解析后属性值原样保留；
/// - build 后八围和技能等级与 overlay 一致；
/// - overlay 中 weapon 字段生效。
fn player_raw_new_parses_diy_overlay() {
    let storage = Storage::new_arc();

    let mut player = Player::new_from_namerena_raw(
        "mario+diy[72,39,69,76,67,66,0,84]{\"sklfire\":5,\"reflect\":2}".to_string(),
        storage.clone(),
    )
    .unwrap();
    assert_eq!(player.name, "mario");
    assert_eq!(player.weapon, None);
    let overlay = player.overlay.as_ref().expect("应解析出 overlay");
    assert_eq!(overlay.attrs, Some([36, 3, 33, 40, 31, 30, 0, 48]));
    assert_eq!(overlay.skills.as_ref().unwrap().get("sklfire"), Some(&5));
    assert_eq!(overlay.skills.as_ref().unwrap().get("reflect"), Some(&2));

    player.build();
    // build 后八围应使用 overlay 覆盖值
    assert_eq!(player.attr, [36, 3, 33, 40, 31, 30, 0, 48]);
    // 技能等级应与 overlay 指定一致
    assert_eq!(player.skills.skill_by_id(0).level(), 5);
    assert_eq!(player.skills.skill_by_id(27).level(), 2);
    // 技能槽顺序应为 DIY 固定布局
    assert_eq!(player.skills.skill[0], 0);
    assert_eq!(player.skills.skill[25], 25);

    // 测试 ol: JSON 格式 + weapon 字段
    let player = Player::new_from_namerena_raw(
        "luigi+ol:{\"attrs\":[1,2,3,4,5,6,7,8],\"skills\":{\"fire\":4},\"weapon\":\"剁手刀\"}".to_string(),
        storage.clone(),
    )
    .unwrap();
    assert_eq!(player.weapon, Some("剁手刀".to_string()));
    let overlay = player.overlay.as_ref().expect("应解析出 overlay");
    assert_eq!(overlay.attrs, Some([1, 2, 3, 4, 5, 6, 7, 8]));
    assert_eq!(overlay.skills.as_ref().unwrap().get("fire"), Some(&4));
}

#[test]
fn player_raw_keeps_internal_js_trim_chars_in_name_and_team() {
    let storage = Storage::new_arc();

    let player = Player::new_from_namerena_raw("ab\u{3000}cd@red\u{3000}team".to_string(), storage.clone()).unwrap();
    assert_eq!(player.name, "ab\u{3000}cd");
    assert_eq!(player.team, Some("red\u{3000}team".to_string()));
}

#[test]
fn player_raw_trims_js_line_end_and_weapon_name_like() {
    let storage = Storage::new_arc();

    let player = Player::new_from_namerena_raw(
        "mario@red\u{3000}+\u{3000}fire\u{3000}\u{0085}\u{3000}".to_string(),
        storage.clone(),
    )
    .unwrap();
    assert_eq!(player.name, "mario");
    assert_eq!(player.team, Some("red".to_string()));
    assert_eq!(player.weapon, Some("fire".to_string()));

    let player = Player::new_from_namerena_raw("luigi\u{3000}".to_string(), storage.clone()).unwrap();
    assert_eq!(player.name, "luigi");
}

#[test]
fn player_name() {
    let storage = Storage::new_arc();

    let player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();
    assert_eq!(player.id_name(), "aaa");
    assert_eq!(player.display_name(), "aaa");

    // 包含了 @
    let player = Player::new_from_namerena_raw("aaa@bbb".to_string(), storage.clone()).unwrap();
    assert_eq!(player.id_name(), "aaa");
    assert_eq!(player.display_name(), "aaa");

    // 空格分开的名字
    let player = Player::new_from_namerena_raw("aaa bbb".to_string(), storage.clone()).unwrap();
    assert_eq!(player.id_name(), "aaa bbb");
    assert_eq!(player.display_name(), "aaa");

    // 包含了 + 的名字
    let player = Player::new_from_namerena_raw("aaa+bbb".to_string(), storage.clone()).unwrap();
    assert_eq!(player.id_name(), "aaa");
    assert_eq!(player.display_name(), "aaa");
}

#[test]
fn player_raw_types() {
    let storage = Storage::new_arc();

    let player = Player::new_from_namerena_raw("normal@normal".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.player_type, PlayerType::Normal);

    // seed
    let player = Player::new_from_namerena_raw("seed:just seed@!".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.name, "seed:just seed");
    assert_eq!(player.player_type, PlayerType::Seed);

    // testEx
    let player = Player::new_from_namerena_raw("testEx@!".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.player_type, PlayerType::TestEx);

    // test1
    let player = Player::new_from_namerena_raw("test1@\u{0002}".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.team, Some("\u{0002}".to_string()));
    assert_eq!(player.player_type, PlayerType::Test1);

    // test2
    let player = Player::new_from_namerena_raw("test2@\u{0003}".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.team, Some("\u{0003}".to_string()));
    assert_eq!(player.player_type, PlayerType::Test2);

    // boss
    let player = Player::new_from_namerena_raw("mario@!".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.player_type, PlayerType::Boss);

    // boosted
    let player = Player::new_from_namerena_raw("云剑狄卡敢@!".to_string(), storage.clone());
    let player = player.unwrap();
    assert_eq!(player.player_type, PlayerType::Boost);
}

#[allow(dead_code)]
fn noop_on_damage(_: PlrId, _: PlrId, _: i32, _: &mut RC4, _: &mut RunUpdates, _: &std::sync::Arc<Storage>) {}

#[test]
fn check_move_threshold_matches_dart() {
    let mut status = PlayerStatus {
        move_point: MOVE_POINT_THRESHOLD,
        ..Default::default()
    };
    assert!(!status.check_move());
    status.move_point = MOVE_POINT_THRESHOLD + 1;
    assert!(status.check_move());
}

#[test]
fn update_states_does_not_reset_hp_or_mp() {
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw("aaa".to_string(), storage.clone()).unwrap();

    player.attr = [10, 20, 30, 40, 50, 60, 70, 80];
    player.init_values();
    assert_eq!(player.status.hp, player.status.max_hp);
    assert_eq!(player.status.magic_point, player.status.wisdom >> 1);

    player.status.hp = 11;
    player.status.magic_point = 22;
    player.update_states();

    assert_eq!(player.status.hp, 11);
    assert_eq!(player.status.magic_point, 22);
}

#[test]
fn check_immune_matches_player_type_rules() {
    let storage = Storage::new_arc();
    let boost = Player::new_from_namerena_raw("云剑穸跄祇@!".to_string(), storage.clone()).unwrap();
    let normal = Player::new_from_namerena_raw("normal".to_string(), storage.clone()).unwrap();
    let mut randomer = RC4 {
        i: 0,
        j: 0,
        main_val: [0u8; 256],
        #[cfg(not(feature = "no_debug"))]
        byte_count: 0,
    };

    assert!(boost.check_immune("poison", &mut randomer));
    assert!(!normal.check_immune("poison", &mut randomer));
}

#[test]
fn build_generates_expected_attr_for_help_and_aaaaa() {
    let storage = Storage::new_arc();
    let mut help = Player::new_from_namerena_raw("help".to_string(), storage.clone()).unwrap();
    let mut aaaaa = Player::new_from_namerena_raw("aaaaa".to_string(), storage.clone()).unwrap();

    help.build();
    aaaaa.build();

    assert_eq!(help.attr, [28, 51, 21, 32, 25, 43, 40, 261]);
    assert_eq!(aaaaa.attr, [31, 36, 17, 30, 50, 50, 47, 315]);
}

#[test]
fn profile_case_d8c6_players() {
    let storage = Storage::new_arc();
    let mut attacker = Player::new_from_namerena_raw("最光辉的时刻 #8ftphKKCk@Shabby_fish".to_string(), storage.clone()).unwrap();
    let mut defender = Player::new_from_namerena_raw("营救任务 #tmOaPuIoM@Shabby_fish".to_string(), storage.clone()).unwrap();

    attacker.build();
    defender.build();

    for player in [&attacker, &defender] {
        let active = player
            .skills
            .skill
            .iter()
            .filter_map(|key| {
                let skill = player.skills.store.get(key)?;
                (skill.level() > 0).then_some(format!("{key}:{}:lvl{}", skill.debug_skill_type_name(), skill.level()))
            })
            .collect::<Vec<_>>();
        eprintln!(
            "[profile_case_d8c6] name={} slot_skill={:?} skill={:?} pre_action={:?} active={:?}",
            player.id_name(),
            player.skills.slot_skill,
            player.skills.skill,
            player.skills.pre_action,
            active
        );
    }
}

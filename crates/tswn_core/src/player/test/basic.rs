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
    assert_eq!(player.status.mp, player.status.wisdom >> 1);

    player.status.hp = 11;
    player.status.mp = 22;
    player.update_states();

    assert_eq!(player.status.hp, 11);
    assert_eq!(player.status.mp, 22);
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

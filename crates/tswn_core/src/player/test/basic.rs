use super::*;
use std::collections::HashMap;

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
/// - overlay 中 weapon 字段生效；
/// - SkillBoost 三种格式（Normal / SlotBoost / LastBoost）正确解析。
fn player_raw_new_parses_diy_overlay() {
    let storage = Storage::new_arc();

    // === compact DIY 格式：普通技能 ===
    let mut player = Player::new_from_namerena_raw(
        "mario+diy[72,39,69,76,67,66,0,84]{\"sklfire\":5,\"reflect\":2}".to_string(),
        storage.clone(),
    )
    .unwrap();
    assert_eq!(player.name, "mario");
    assert_eq!(player.weapon, None);
    let overlay = player.overlay.as_ref().expect("应解析出 overlay");
    assert_eq!(overlay.attrs, Some([36, 3, 33, 40, 31, 30, 0, 84]));
    assert_eq!(overlay.skills.as_ref().unwrap().get("sklfire"), Some(&SkillBoost::Normal(5)));
    assert_eq!(overlay.skills.as_ref().unwrap().get("reflect"), Some(&SkillBoost::Normal(2)));

    player.build();
    // build 后八围应使用 overlay 覆盖值（HP 不减 36，原样 84）
    assert_eq!(player.attr, [36, 3, 33, 40, 31, 30, 0, 84]);
    // 技能等级应与 overlay 指定一致
    assert_eq!(player.skills.skill_by_id(0).level(), 5);
    assert_eq!(player.skills.skill_by_id(27).level(), 2);
    // DIY 模式下武器不计入
    assert!(player.weapon_state.is_none());
    // 技能槽顺序应为 DIY 固定布局
    assert_eq!(player.skills.skill[0], 0);
    assert_eq!(player.skills.skill[25], 25);
    // SkillStorage 标记为 DIY
    assert!(player.skills.is_diy);

    // === compact DIY 格式：SlotBoost 末尾座位加成 ===
    let player = Player::new_from_namerena_raw(
        "test+diy[72,39,69,76,67,66,0,84]{\"sklfire\":\"40+30\"}".to_string(),
        storage.clone(),
    )
    .unwrap();
    let overlay = player.overlay.as_ref().expect("应解析出 overlay");
    assert_eq!(
        overlay.skills.as_ref().unwrap().get("sklfire"),
        Some(&SkillBoost::SlotBoost { base: 40, boost: 30 })
    );
    assert_eq!(overlay.skills.as_ref().unwrap().get("sklfire").unwrap().final_level(), 70);

    // === compact DIY 格式：LastBoost 末尾主动技翻倍 ===
    let player = Player::new_from_namerena_raw(
        "test+diy[72,39,69,76,67,66,0,84]{\"sklshadow\":\"2*46\"}".to_string(),
        storage.clone(),
    )
    .unwrap();
    let overlay = player.overlay.as_ref().expect("应解析出 overlay");
    assert_eq!(
        overlay.skills.as_ref().unwrap().get("sklshadow"),
        Some(&SkillBoost::LastBoost(46))
    );
    assert_eq!(overlay.skills.as_ref().unwrap().get("sklshadow").unwrap().final_level(), 92);

    // === ol: JSON 格式 + weapon 字段 ===
    // 注意：DIY 模式下（有八围/技能覆盖）weapon 字段不计入武器系统。
    // ol: 格式的八围值也会 -36（前七围），与 compact 格式一致。
    let player = Player::new_from_namerena_raw(
        "luigi+ol:{\"attrs\":[37,38,39,40,41,42,43,300],\"skills\":{\"fire\":4},\"weapon\":\"剁手刀\"}".to_string(),
        storage.clone(),
    )
    .unwrap();
    // weapon 名被记录但 weapon_state 为空（DIY 模式武器不计入）
    assert_eq!(player.weapon, Some("剁手刀".to_string()));
    assert!(player.weapon_state.is_none());
    let overlay = player.overlay.as_ref().expect("应解析出 overlay");
    // 前七围 -36: 37→1, 38→2, 39→3, 40→4, 41→5, 42→6, 43→7, HP=300 不变
    assert_eq!(overlay.attrs, Some([1, 2, 3, 4, 5, 6, 7, 300]));
    assert_eq!(overlay.skills.as_ref().unwrap().get("fire"), Some(&SkillBoost::Normal(4)));
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

// ==============
// DIY overlay 新增功能测试
// ==============

#[test]
/// 测试 name_factor_enabled = false 时强制 name_factor = 0
fn diy_overlay_name_factor_disabled_forces_zero() {
    let storage = Storage::new_arc();
    let overlay = PlayerOverlay {
        attrs: Some([50, 50, 50, 50, 50, 50, 50, 300]),
        name_factor_enabled: false,
        ..Default::default()
    };
    let player = Player::new_and_init_with_overlay(
        None,
        "aaaaa".to_string(),
        None,
        Some(overlay),
        storage.clone(),
    )
    .unwrap();
    assert!((player.name_factor - 0.0).abs() < f64::EPSILON);
}

#[test]
/// 测试 name_factor_enabled 默认为 true 时 name_factor 正常计算。
/// 对比：同一个名字的 overlay 玩家和普通玩家应有相同的 name_factor。
fn diy_overlay_name_factor_defaults_to_enabled() {
    let storage = Storage::new_arc();
    // 普通玩家
    let normal = Player::new_and_init(None, "aaaaa".to_string(), None, storage.clone()).unwrap();
    // overlay 玩家（name_factor_enabled 默认为 true）
    let overlay = PlayerOverlay {
        attrs: Some([50, 50, 50, 50, 50, 50, 50, 300]),
        ..Default::default()
    };
    assert!(overlay.name_factor_enabled);
    let diy = Player::new_and_init_with_overlay(
        None,
        "aaaaa".to_string(),
        None,
        Some(overlay),
        storage.clone(),
    )
    .unwrap();
    // 两个玩家的 name_factor 应相等（overlay 不影响 name_factor 计算）
    assert!((diy.name_factor - normal.name_factor).abs() < f64::EPSILON);
}

#[test]
/// 测试 DIY 模式下武器不计入（weapon_state 为 None）
fn diy_overlay_disables_weapon() {
    let storage = Storage::new_arc();
    // 有八围覆盖 → 武器不计入
    let overlay = PlayerOverlay {
        attrs: Some([50, 50, 50, 50, 50, 50, 50, 300]),
        weapon: Some("剁手刀".to_string()),
        ..Default::default()
    };
    let player = Player::new_and_init_with_overlay(
        Some("team".to_string()),
        "test".to_string(),
        Some("火剑".to_string()),
        Some(overlay),
        storage.clone(),
    )
    .unwrap();
    // weapon 名保留但 weapon_state 为空
    assert!(player.get_weapon_name().is_some());
    assert!(player.weapon_state.is_none());
}

#[test]
/// 测试 DIY 技能 build 后 diy_boost 信息正确存储在 Skill 上
fn diy_skill_boost_info_stored_on_skill() {
    let storage = Storage::new_arc();
    let overlay = PlayerOverlay {
        skills: Some(HashMap::from([
            ("fire".to_string(), SkillBoost::Normal(5)),
            ("ice".to_string(), SkillBoost::SlotBoost { base: 40, boost: 30 }),
            ("shadow".to_string(), SkillBoost::LastBoost(46)),
        ])),
        ..Default::default()
    };
    let mut player = Player::new_and_init_with_overlay(
        Some("team".to_string()),
        "test".to_string(),
        None,
        Some(overlay),
        storage.clone(),
    )
    .unwrap();
    player.build();

    // Normal: level=5, diy_boost=None, boosted=false
    let fire = player.skills.skill_by_id(0);
    assert_eq!(fire.level(), 5);
    assert!(fire.diy_boost.is_none());
    assert!(!fire.boosted);

    // SlotBoost: level=70, diy_boost=Some(SlotBoost{40,30}), boosted=true
    let ice = player.skills.skill_by_id(1);
    assert_eq!(ice.level(), 70);
    assert_eq!(ice.diy_boost, Some(SkillBoost::SlotBoost { base: 40, boost: 30 }));
    assert!(ice.boosted);

    // LastBoost: level=92, diy_boost=Some(LastBoost(46)), boosted=true
    let shadow = player.skills.skill_by_id(24);
    assert_eq!(shadow.level(), 92);
    assert_eq!(shadow.diy_boost, Some(SkillBoost::LastBoost(46)));
    assert!(shadow.boosted);

    // SkillStorage 标记为 DIY
    assert!(player.skills.is_diy);
}

#[test]
/// 测试 SkillBoost::parse 解析各种格式
fn skill_boost_parse_formats() {
    // 普通整数
    assert_eq!(SkillBoost::parse("5"), Some(SkillBoost::Normal(5)));
    assert_eq!(SkillBoost::parse("0"), Some(SkillBoost::Normal(0)));
    // SlotBoost "base+boost"
    assert_eq!(SkillBoost::parse("40+30"), Some(SkillBoost::SlotBoost { base: 40, boost: 30 }));
    assert_eq!(SkillBoost::parse("1+1"), Some(SkillBoost::SlotBoost { base: 1, boost: 1 }));
    // LastBoost "2*base"
    assert_eq!(SkillBoost::parse("2*46"), Some(SkillBoost::LastBoost(46)));
    assert_eq!(SkillBoost::parse("2*1"), Some(SkillBoost::LastBoost(1)));
    // 无效格式
    assert_eq!(SkillBoost::parse("3*40"), None); // 只支持 2*
    assert_eq!(SkillBoost::parse("abc"), None);
    assert_eq!(SkillBoost::parse("1+2+3"), None);
}

#[test]
/// 测试 SkillBoost::final_level / base_level / decayed_base_from_level / final_level_from_decayed_base
fn skill_boost_level_methods() {
    // Normal
    let n = SkillBoost::Normal(5);
    assert_eq!(n.final_level(), 5);
    assert_eq!(n.base_level(), 5);
    assert_eq!(n.decayed_base_from_level(3), 3); // 衰减后基础=当前
    assert_eq!(n.final_level_from_decayed_base(3), 3);

    // SlotBoost
    let s = SkillBoost::SlotBoost { base: 40, boost: 30 };
    assert_eq!(s.final_level(), 70);
    assert_eq!(s.base_level(), 40);
    assert_eq!(s.decayed_base_from_level(35), 5); // 衰减后: 35-30=5
    assert_eq!(s.final_level_from_decayed_base(5), 35);

    // LastBoost
    let l = SkillBoost::LastBoost(46);
    assert_eq!(l.final_level(), 92);
    assert_eq!(l.base_level(), 46);
    assert_eq!(l.decayed_base_from_level(40), 20); // 衰减后: 40/2=20
    assert_eq!(l.final_level_from_decayed_base(20), 40);
}

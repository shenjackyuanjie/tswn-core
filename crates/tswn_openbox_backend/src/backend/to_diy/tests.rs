use super::*;
use std::sync::atomic::AtomicBool;
use tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ;

/// 一行输入按整队构建后应该追加的全部详情块（含每个块前面的空行）。
fn expected_group_blocks(raw: &str) -> String {
    let groups = super::parse_namer_pf_groups(raw);
    let input = NamerenaInput::from_raw_groups(&groups).expect("group should parse");
    let roster = PreparedRoster::build(&input, DEFAULT_EVAL_RQ).expect("group should build");
    roster
        .players
        .iter()
        .map(|player| format!("\n{}", expected_detail_block(player)))
        .collect()
}

/// 多行输入的全部输出：每行之间空一行，行内是导出行 + 该行每个成员的详情块。
fn expected_multiline_output(raw: &str, old: bool) -> String {
    let mut out = String::new();
    for name in super::parse_line_list(raw) {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&tswn_core::cli_api::to_diy(&name, old, false).expect("export should work"));
        out.push('\n');
        out.push_str(&expected_group_blocks(&name));
    }
    out
}

/// 详情块必须逐字段等于同一份导出 attrs / skills（含组队差额）的换算结果。
fn expected_detail_block(player: &PreparedPlayer) -> String {
    let attrs = super::export_attrs(player.attrs);
    let solo = super::solo_player_of(&player.id_key_name).expect("solo build should work");
    let solo_attrs = super::export_attrs(solo.attrs);
    let solo_levels = super::skill_levels(&solo);
    let mut block = format!("=== 原始信息 ===\n{}\n", player.id_key_name);
    block.push_str(&format!(
        "HP {} 攻 {} 防 {} 速 {} 敏 {} 魔 {} 抗 {} 智 {} 八围 {} 嘲讽{}\n",
        super::format_delta(i64::from(attrs[7]), i64::from(solo_attrs[7])),
        super::format_delta(i64::from(attrs[0]), i64::from(solo_attrs[0])),
        super::format_delta(i64::from(attrs[1]), i64::from(solo_attrs[1])),
        super::format_delta(i64::from(attrs[2]), i64::from(solo_attrs[2])),
        super::format_delta(i64::from(attrs[3]), i64::from(solo_attrs[3])),
        super::format_delta(i64::from(attrs[4]), i64::from(solo_attrs[4])),
        super::format_delta(i64::from(attrs[5]), i64::from(solo_attrs[5])),
        super::format_delta(i64::from(attrs[6]), i64::from(solo_attrs[6])),
        super::format_eight_ring(attrs),
        super::taunt_value(attrs),
    ));
    for skill in super::action_order_skills(player, &solo_levels) {
        let level = i64::from(skill.level);
        block.push_str(&format!(
            "  {} {}\n",
            skill.name,
            super::format_delta(level, level - skill.delta)
        ));
    }
    block
}

/// 面板口径的完整示例：每行导出后，按该行整队逐个成员追加详情块；行与行之间空一行。
#[test]
fn to_diy_details_match_panel_layout() {
    let cancel = AtomicBool::new(false);
    let raw = "1@team+2@team\ntest";
    let output = run_to_diy(raw, false, false, true, None, &cancel).unwrap();

    assert_eq!(output, expected_multiline_output(raw, false));
}

/// 多行输入之间必须空行分隔，详情块不能和下一行的导出行贴在一起。
#[test]
fn to_diy_details_separate_lines() {
    let cancel = AtomicBool::new(false);
    let raw = "1@team+2@team\ntest";
    let output = run_to_diy(raw, false, false, true, None, &cancel).unwrap();
    let lines = output.lines().collect::<Vec<_>>();
    let first_export_of_second_line = lines.iter().position(|line| line.starts_with("test+ol:")).expect("second export");

    assert_eq!(lines[first_export_of_second_line - 1], "");
    assert_eq!(lines[first_export_of_second_line - 2], "  潜行 34");
    // 行内成员之间同样空行分隔。
    assert_eq!(lines[12], "");
    assert_eq!(lines[13], "=== 原始信息 ===");
}

/// 同队升级也会改技能熟练度，技能行同样要标差额。
#[test]
fn to_diy_details_mark_team_skill_delta() {
    let cancel = AtomicBool::new(false);
    let output = run_to_diy(
        "冥河 WyO8MUZPPtKH@Afterglow+光 jKLA6V5mirfs@Afterglow",
        false,
        false,
        true,
        None,
        &cancel,
    )
    .unwrap();
    let lines = output.lines().collect::<Vec<_>>();

    // 第一个成员：只有「智」被组队升级（+21），技能没有变化。
    assert_eq!(lines[1], "");
    assert_eq!(lines[2], "=== 原始信息 ===");
    assert_eq!(lines[3], "冥河 WyO8MUZPPtKH@Afterglow");
    assert_eq!(
        lines[4],
        "HP 311 攻 56 防 83 速 98 敏 65 魔 90 抗 96 智 94(+21) 八围 685.7 嘲讽289"
    );
    assert_eq!(lines[5], "  守护 20");
    assert_eq!(lines[13], "  魅惑 80");
    assert_eq!(lines[12], "  分身 56");
    // 第二个成员：护符从单独构建的 84 升到 98，技能行标 `(+14)`。
    assert_eq!(lines[14], "");
    assert_eq!(lines[15], "=== 原始信息 ===");
    assert_eq!(lines[16], "光 jKLA6V5mirfs@Afterglow");
    assert_eq!(lines[17], "HP 315 攻 95 防 73 速 92 敏 77 魔 89 抗 86 智 84 八围 701.0 嘲讽395");
    assert_eq!(lines[18], "  苏生 8");
    assert_eq!(lines[19], "  隐匿 1");
    assert_eq!(lines[20], "  吞噬 14");
    assert_eq!(lines[21], "  反击 6");
    assert_eq!(lines[22], "  命轮 20");
    assert_eq!(lines[23], "  分身 58");
    assert_eq!(lines[24], "  护符 98(+14)");
    assert_eq!(lines.len(), 25);
}

#[test]
fn to_diy_details_reproduce_reported_example() {
    let cancel = AtomicBool::new(false);
    let output = run_to_diy("1@team+2@team\ntest", false, false, true, None, &cancel).unwrap();
    let lines = output.lines().collect::<Vec<_>>();

    // 组队行导出后，紧跟该队两个玩家的详情块。
    assert!(lines[0].starts_with("1@team+ol:{\"attrs\":[78,78,58,64,72,60,77,245]"));
    assert!(lines[0].contains("\"sklprotect\":19"));
    assert!(lines[0].contains("+2@team+ol:{\"attrs\":[81,83,59,70,71,61,61,271]"));
    assert!(lines[0].contains("\"sklassassinate\":\"2*17\""));
    assert_eq!(lines[1], "");
    assert_eq!(lines[2], "=== 原始信息 ===");
    assert_eq!(lines[3], "1@team");
    // 组队后 `1` 的 HP 是 245（单独构队 243），括号里标出 +2；八围 568.7、嘲讽 281。
    assert_eq!(
        lines[4],
        "HP 245(+2) 攻 78 防 78 速 58 敏 64 魔 72 抗 60 智 77 八围 568.7 嘲讽281"
    );
    assert_eq!(lines[5], "  守护 19");
    assert_eq!(lines[6], "  加速 14");
    assert_eq!(lines[7], "  诅咒 29");
    assert_eq!(lines[8], "  分身 4");
    assert_eq!(lines[9], "  聚气 2");
    assert_eq!(lines[10], "  反弹 1");
    assert_eq!(lines[11], "  护符 4");
    // 同一队的第二个玩家也有自己的块（属性与单独构建一致，所以没有括号）。
    assert_eq!(lines[12], "");
    assert_eq!(lines[13], "=== 原始信息 ===");
    assert_eq!(lines[14], "2@team");
    assert_eq!(lines[15], "HP 271 攻 81 防 83 速 59 敏 70 魔 71 抗 61 智 61 八围 576.3 嘲讽265");
    // 第二行单名：导出、空行、详情块。
    let solo_at = lines.iter().position(|line| line.starts_with("test+ol:")).expect("solo export line");
    assert_eq!(lines[solo_at + 1], "");
    assert_eq!(lines[solo_at + 2], "=== 原始信息 ===");
    assert_eq!(lines[solo_at + 3], "test");
    assert!(lines[solo_at + 4].starts_with("HP "));
    assert!(lines[solo_at + 4].contains(" 八围 "));
    assert!(lines[solo_at + 4].contains(" 嘲讽"));
    assert_eq!(lines.len(), solo_at + 5 + 7);
}

/// 用 overlay 固定输入，逐字节校验详情块的排版与算值。
#[test]
fn to_diy_details_follow_single_name_layout() {
    let cancel = AtomicBool::new(false);
    // overlay 的 attrs 会被解码成内部属性，导出行写回时再 +36，取 ≥36 保证与输入一致。
    let raw = r#"mario+ol:{"attrs":[40,50,60,70,80,90,100,200],"skills":{"sklfire":5,"sklheal":40},"name_factor_enabled":true}"#;
    let output = run_to_diy(raw, false, false, true, None, &cancel).unwrap();

    // 八围 = 490 + 200/3 = 556.7
    // 嘲讽 = 50*2 + 90*2 - 40*2 - 80*2 - 60*2 - 70 - 100 = -250
    assert_eq!(
        output,
        format!(
            "{raw}\n\n=== 原始信息 ===\nmario\nHP 200 攻 40 防 50 速 60 敏 70 魔 80 抗 90 智 100 八围 556.7 嘲讽250\n  火球 5\n  治愈 40\n"
        )
    );
}

#[test]
fn to_diy_details_are_only_extra_log_lines() {
    let cancel = AtomicBool::new(false);
    let raw = r#"mario+diy[72,39,69,76,67,66,0,84]{"sklfire":5}"#;
    let without_details = run_to_diy(raw, true, false, false, None, &cancel).unwrap();
    let with_details = run_to_diy(raw, true, false, true, None, &cancel).unwrap();

    assert!(!without_details.contains("=== 原始信息 ==="));
    // 勾选详情只在导出行后面追加日志块，导出行本身逐字节不变。
    assert!(with_details.starts_with(&without_details));
    assert!(with_details.contains("=== 原始信息 ==="));
}

#[test]
fn to_diy_details_apply_per_line_and_per_group_member() {
    let cancel = AtomicBool::new(false);
    let one_line = run_to_diy("mario@team", true, false, true, None, &cancel).unwrap();
    let two_lines = run_to_diy("mario@team\nluigi@team", true, false, true, None, &cancel).unwrap();
    // `+` 是组队分隔符：这行有两个成员，两个成员各出一个详情块。
    let group_line = run_to_diy("mario@team+fire", true, false, true, None, &cancel).unwrap();

    assert!(one_line.contains("=== 原始信息 ==="));
    assert_eq!(two_lines.matches("=== 原始信息 ===").count(), 2);
    assert!(two_lines.contains("mario@team\n"));
    assert!(two_lines.contains("luigi@team\n"));
    assert_eq!(group_line.matches("=== 原始信息 ===").count(), 2);
}

#[test]
fn to_diy_details_never_reach_the_output_file() {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("tswn_openbox_to_diy_details_{}_{seq}.txt", std::process::id()));
    let cancel = AtomicBool::new(false);

    let summary = run_to_diy("mario@team", true, false, true, Some(path.clone()), &cancel).expect("export should succeed");
    let written = std::fs::read_to_string(&path).expect("output file should exist");
    let _ = std::fs::remove_file(&path);

    assert!(summary.starts_with("完成，结果已写入"));
    assert!(!written.contains("=== 原始信息 ==="));
    assert_eq!(written, run_to_diy("mario@team", true, false, false, None, &cancel).unwrap());
}

#[test]
fn to_diy_detail_taunt_uses_absolute_value() {
    assert_eq!(super::taunt_value([78, 78, 58, 64, 72, 60, 77, 245]), 281);
    assert_eq!(super::taunt_value([81, 83, 59, 70, 71, 61, 61, 271]), 265);
    assert_eq!(super::taunt_value([85, 72, 79, 76, 81, 41, 70, 236]), 410);
    assert_eq!(super::taunt_value([0, 0, 0, 0, 0, 0, 0, 0]), 0);
}

#[test]
fn to_diy_detail_eight_ring_keeps_one_decimal() {
    // 面板样例：七围 487 + HP 245 → 487 + 81.7 = 568.7。
    assert_eq!(super::format_eight_ring([78, 78, 58, 64, 72, 60, 77, 245]), "568.7");
    // `2@team`：七围 486 + HP 271 → 486 + 90.3 = 576.3。
    assert_eq!(super::format_eight_ring([81, 83, 59, 70, 71, 61, 61, 271]), "576.3");
    // `test`：七围 504 + HP 236 → 504 + 78.7 = 582.7。
    assert_eq!(super::format_eight_ring([85, 72, 79, 76, 81, 41, 70, 236]), "582.7");
    // 仓库当前 `test` 的构建：七围 478 + HP 259 → 564.3。
    assert_eq!(super::format_eight_ring([71, 80, 59, 70, 75, 56, 67, 259]), "564.3");
    assert_eq!(super::format_eight_ring([0, 0, 0, 0, 0, 0, 0, 0]), "0.0");
    // 四舍五入：HP 1470 / 3 = 490.0 正好落在整数上；1482 / 3 = 494.0。
    assert_eq!(super::format_eight_ring([0, 0, 0, 0, 0, 0, 0, 1470]), "490.0");
    assert_eq!(super::format_eight_ring([0, 0, 0, 0, 0, 0, 0, 1482]), "494.0");
    // 1472 / 3 = 490.66…：进位到 490.7。
    assert_eq!(super::format_eight_ring([0, 0, 0, 0, 0, 0, 0, 1472]), "490.7");
}

#[test]
fn to_diy_plus_line_exports_team_group() {
    let cancel = AtomicBool::new(false);
    let output = run_to_diy("1@a\n2@a\n1@a+2@a", true, false, false, None, &cancel).unwrap();
    let lines = output.lines().collect::<Vec<_>>();

    // 每行结果之间空一行。
    assert_eq!(lines.len(), 5);
    assert!(lines[0].starts_with("1@a+diy["));
    assert_eq!(lines[1], "");
    assert!(lines[2].starts_with("2@a+diy["));
    assert_eq!(lines[3], "");
    assert!(lines[4].starts_with("1@a+diy["));
    assert!(lines[4].contains("+2@a+diy["));
}

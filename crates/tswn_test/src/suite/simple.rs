//! Runner 基础共享回放测试。
//!
//! 这里只保留依赖 raw input、round updates、winner 和 score 的黑盒用例。

use super::*;

pub fn simple_fight<E: crate::EngineAdapter>() {
    const FIGHT_SIMPLE_CASE: &str = r###"aaa
bbb
ccc
ddd
eee
fff


ddd发起攻击, aaa受到53点伤害

ccc发起攻击, bbb受到47点伤害

aaa发起攻击, eee受到38点伤害

 aaa连击, ddd回避了攻击

eee使用诅咒, aaa受到85点伤害, aaa被诅咒了

fff发起攻击, eee受到90点伤害

bbb发起攻击, ddd受到51点伤害

ccc发起攻击, aaa受到63点伤害

eee发起攻击, bbb受到63点伤害

ddd发起攻击, ccc受到120点伤害

bbb发起攻击, fff受到64点伤害

eee发起攻击, ddd受到41点伤害

aaa使用加速术, aaa进入疾走状态

ccc发起攻击, ddd受到96点伤害

ddd发起攻击, bbb受到69点伤害

fff发起攻击, eee受到92点伤害

aaa发起攻击, ddd受到37点伤害

aaa发起攻击, eee受到72点伤害

 eee做出垂死抗争, eee所有属性上升

 aaa从疾走中解除

bbb发起攻击, ccc受到35点伤害

eee发起攻击, 诅咒使伤害加倍, aaa受到130点伤害

 aaa被击倒了

ddd发起攻击, bbb受到44点伤害

fff发起攻击, ccc受到59点伤害

ccc发起攻击, ddd受到84点伤害

 ddd被击倒了

ccc发起攻击, fff受到56点伤害

eee使用诅咒, fff受到74点伤害, fff被诅咒了

bbb发起攻击, fff受到72点伤害

eee发起攻击, ccc受到66点伤害

 ccc被击倒了

bbb发起攻击, eee受到23点伤害

 eee被击倒了

fff发起攻击, bbb受到20点伤害

 bbb被击倒了, bbb使用护身符抵挡了一次死亡, bbb回复体力16点

bbb发起攻击, 诅咒使伤害加倍, fff受到134点伤害

 fff被击倒了"###;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        FIGHT_SIMPLE_CASE,
        "embedded simple fight case must contain a blank separator between input and trace",
        "embedded simple fight trace is empty",
    );

    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 10_000, true);
    assert_eq!(total_score, 2521, "simple_fight score mismatch");

    assert!(guard < 10_000, "fight_simple combat did not finish in expected rounds");
    assert_eq!(actual_lines, expected_lines);
}

pub fn simple_fight_scores<E: crate::EngineAdapter>() {
    let input = "aaa\nbbb\nccc\nddd\neee\nfff";
    let mut runner = E::new_from_raw(input.to_string()).unwrap();
    let (total, by_name) = collect_battle_scores::<E>(&mut runner, 10_000);
    eprintln!("total_score={total}");
    let mut entries: Vec<_> = by_name.iter().collect();
    entries.sort_by(|a, b| b.1.cmp(a.1));
    for (name, score) in &entries {
        eprintln!("  {name}={score}");
    }
    // 快照结果已与 JS（branch/latest/md5.js）的 score hook 对齐验证。
    assert_eq!(total, 2521, "total battle score mismatch");
}

pub fn small_seed_scores<E: crate::EngineAdapter>() {
    let input = "aaaaa\nbbbbb\nseed:tester@!";
    let mut runner = E::new_from_raw(input.to_string()).unwrap();
    let (total, by_name) = collect_battle_scores::<E>(&mut runner, 10_000);
    // 快照结果已与 JS（branch/latest/md5.js）的 score hook 对齐验证。
    assert_eq!(total, 635, "total battle score mismatch");
    assert_eq!(by_name["aaaaa"], 463);
    assert_eq!(by_name["bbbbb"], 172);
}

pub fn case_d8c6_opening_matches_js_trace<E: crate::EngineAdapter>() {
    let raw_input = "最光辉的时刻 #8ftphKKCk@Shabby_fish\n营救任务 #tmOaPuIoM@Shabby_fish".to_string();
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines::<E>(&mut runner, 10_000, true);
    assert!(guard < 10_000, "case_d8c6 combat did not finish");
    assert_eq!(
        actual_lines[..3],
        [
            "最光辉的时刻潜行到营救任务身后".to_string(),
            "营救任务潜行到最光辉的时刻身后".to_string(),
            "营救任务发动背刺, 最光辉的时刻受到242点伤害".to_string(),
        ]
    );
}



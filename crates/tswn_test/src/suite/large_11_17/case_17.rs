use super::*;

pub fn case_17<E: crate::EngineAdapter>() {
    let expected: Vec<String> = vec![
        "aaaaa发起攻击",
        "help受到77点伤害",
        "aaaaa发起攻击",
        "help受到80点伤害",
        "help发起攻击",
        "aaaaa受到87点伤害",
        "help发起攻击",
        "aaaaa受到87点伤害",
        "aaaaa发起攻击",
        "help受到32点伤害",
        "help使用[雷击术]",
        "aaaaa受到26点伤害",
        "aaaaa受到25点伤害",
        "aaaaa受到10点伤害",
        "aaaaa受到9点伤害",
        "aaaaa受到10点伤害",
        "aaaaa受到14点伤害",
        "aaaaa发起攻击",
        "help受到43点伤害",
        "help发起攻击",
        "aaaaa受到94点伤害",
        "aaaaa被击倒了",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    let mut runner = E::new_from_raw("help\naaaaa".to_string()).unwrap();
    let (actual, guard, total_score) = collect_replay_events::<E>(&mut runner, 256, false);
    assert_eq!(total_score, 645, "case_17 score mismatch");

    assert!(guard < 256, "combat did not finish in expected rounds");
    assert_trace_with_context("case_17", &actual, &expected);

    let winner = winner_names::<E>(&runner);
    assert_eq!(winner, vec!["help".to_string()]);
}

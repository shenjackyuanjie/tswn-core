use super::*;

#[test]
fn small_seed() {
    let mut runner = runners::Runner::new_from_namerena_raw("aaaaa\nbbbbb\nseed:tester@!".to_string()).unwrap();
    let (lines, guard) = collect_replay_lines(&mut runner, 256, true);

    assert!(guard < 256, "combat did not finish in expected rounds");
    assert_eq!(
        lines,
        vec![
            "aaaaa发起攻击, bbbbb受到104点伤害",
            "bbbbb发起攻击, aaaaa受到76点伤害",
            "aaaaa发起反击, bbbbb受到119点伤害",
            "bbbbb发起攻击, aaaaa受到41点伤害",
            "aaaaa发起攻击, bbbbb受到45点伤害",
            "bbbbb发起攻击, aaaaa受到55点伤害",
            "aaaaa发起攻击, bbbbb受到144点伤害",
            "bbbbb被击倒了"
        ]
    );

    let winner = winner_names(&runner);
    assert_eq!(winner, vec!["aaaaa".to_string()]);
}

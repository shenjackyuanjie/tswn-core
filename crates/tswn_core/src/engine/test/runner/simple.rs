use super::*;

#[derive(Clone, Copy)]
enum PreparedParityMode {
    OneVsOne,
    TwoVsTwo,
    ThreeVsThreeVsThree,
    FreeForAll(usize),
}

impl PreparedParityMode {
    fn build_input(self, players: &[String], seed: &str) -> String {
        match self {
            Self::OneVsOne | Self::FreeForAll(_) => format!("{}\n{seed}", players.join("\n")),
            Self::TwoVsTwo => format!("{}\n\n{}\n{seed}", players[..2].join("\n"), players[2..4].join("\n")),
            Self::ThreeVsThreeVsThree => format!(
                "{}\n\n{}\n\n{}\n{seed}",
                players[..3].join("\n"),
                players[3..6].join("\n"),
                players[6..9].join("\n")
            ),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::OneVsOne => "1v1",
            Self::TwoVsTwo => "2v2",
            Self::ThreeVsThreeVsThree => "3v3v3",
            Self::FreeForAll(4) => "ffa_4",
            Self::FreeForAll(6) => "ffa_6",
            Self::FreeForAll(8) => "ffa_8",
            Self::FreeForAll(_) => "ffa",
        }
    }

    fn total_players(self) -> usize {
        match self {
            Self::OneVsOne => 2,
            Self::TwoVsTwo => 4,
            Self::ThreeVsThreeVsThree => 9,
            Self::FreeForAll(size) => size,
        }
    }
}

fn prepared_parity_library() -> Vec<String> {
    [
        "114514",
        "1919810",
        "aaa",
        "bbb",
        "ccc",
        "ddd",
        "eee",
        "fff",
        "ggg",
        "hhh",
        "iii",
        "jjj",
        "kkk",
        "lll",
        "mmm",
        "nnn",
        "ooo",
        "ppp",
        "qqq",
        "rrr",
        "sss",
        "ttt",
        "uuu",
        "vvv",
        "www",
        "xxx",
        "yyy",
        "zzz",
        "alpha",
        "beta",
        "gamma",
        "delta",
        "omega",
        "lambda",
        "sigma",
        "theta",
        "喘际瞬爆@昀澤",
        "蕾蒂·怀特洛可-65HEZHB264LFPFQ@Squall",
        "SB",
        "LJ",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn deterministic_rotate<T>(items: &mut [T], shift: usize) {
    if items.is_empty() {
        return;
    }
    items.rotate_left(shift % items.len());
}

fn assert_prepare_vs_raw_case(mode: PreparedParityMode, case_idx: usize, library: &[String]) {
    let total_players = mode.total_players();
    let mut players = library[..total_players].to_vec();
    deterministic_rotate(&mut players, case_idx);
    let seed = format!("seed:{}@!", crate::engine::PROFILE_START as usize + case_idx);
    let raw = mode.build_input(&players, &seed);
    let (groups, parsed_seed) = runners::Runner::split_namerena_into_groups(raw.clone());

    let mut raw_runner = runners::Runner::new_from_namerena_raw(raw).unwrap();
    let prepared = runners::Runner::prepare_groups_with_eval_rq(&groups, crate::player::eval_name::DEFAULT_EVAL_RQ).unwrap();
    let mut prepared_runner = runners::Runner::new_from_prepared_with_seed(&prepared, &parsed_seed).unwrap();

    assert_eq!(
        raw_runner.input_groups,
        prepared_runner.input_groups,
        "input_groups mismatch for mode={} case_idx={case_idx}",
        mode.label()
    );

    let (raw_lines, raw_guard, raw_score) = collect_replay_lines(&mut raw_runner, 10_000, true);
    let (prepared_lines, prepared_guard, prepared_score) = collect_replay_lines(&mut prepared_runner, 10_000, true);

    assert!(
        raw_guard < 10_000,
        "raw runner did not finish for mode={} case_idx={case_idx}",
        mode.label()
    );
    assert!(
        prepared_guard < 10_000,
        "prepared runner did not finish for mode={} case_idx={case_idx}",
        mode.label()
    );

    assert_eq!(
        raw_score,
        prepared_score,
        "battle score mismatch for mode={} case_idx={case_idx}",
        mode.label()
    );
    assert_eq!(
        winner_names(&raw_runner),
        winner_names(&prepared_runner),
        "winner mismatch for mode={} case_idx={case_idx}",
        mode.label()
    );
    assert_eq!(
        raw_lines,
        prepared_lines,
        "full replay trace mismatch for mode={} case_idx={case_idx}",
        mode.label()
    );
}

#[test]
fn simple_fight() {
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

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 10_000, true);
    assert_eq!(total_score, 2521, "simple_fight score mismatch");

    assert!(guard < 10_000, "fight_simple combat did not finish in expected rounds");
    assert_eq!(actual_lines, expected_lines);
}

#[test]
fn simple_fight_scores() {
    let input = "aaa\nbbb\nccc\nddd\neee\nfff";
    let mut runner = runners::Runner::new_from_namerena_raw(input.to_string()).unwrap();
    let (total, by_name) = collect_battle_scores(&mut runner, 10_000);
    eprintln!("total_score={total}");
    let mut entries: Vec<_> = by_name.iter().collect();
    entries.sort_by(|a, b| b.1.cmp(a.1));
    for (name, score) in &entries {
        eprintln!("  {name}={score}");
    }
    // Snapshot: verified against JS (branch/latest/md5.js) score hook
    assert_eq!(total, 2521, "total battle score mismatch");
}

#[test]
fn small_seed_scores() {
    let input = "aaaaa\nbbbbb\nseed:tester@!";
    let mut runner = runners::Runner::new_from_namerena_raw(input.to_string()).unwrap();
    let (total, by_name) = collect_battle_scores(&mut runner, 10_000);
    // Snapshot: verified against JS (branch/latest/md5.js) score hook
    assert_eq!(total, 635, "total battle score mismatch");
    assert_eq!(by_name["aaaaa"], 463);
    assert_eq!(by_name["bbbbb"], 172);
}

#[test]
fn case_d8c6_opening_matches_js_trace() {
    let raw_input = "最光辉的时刻 #8ftphKKCk@Shabby_fish\n营救任务 #tmOaPuIoM@Shabby_fish".to_string();
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines(&mut runner, 10_000, true);
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

#[test]
fn prepared_runner_matches_raw_runner_across_modes_and_cases() {
    let library = prepared_parity_library();
    let modes = [
        PreparedParityMode::OneVsOne,
        PreparedParityMode::TwoVsTwo,
        PreparedParityMode::ThreeVsThreeVsThree,
        PreparedParityMode::FreeForAll(4),
        PreparedParityMode::FreeForAll(6),
        PreparedParityMode::FreeForAll(8),
    ];

    for mode in modes {
        for case_idx in 0..10 {
            assert_prepare_vs_raw_case(mode, case_idx, &library);
        }
    }
}

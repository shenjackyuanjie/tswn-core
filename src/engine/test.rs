use super::*;

/// 酒吧点炒饭列表（确信）。
macro_rules! str_vec {
    () => {{
        let vec: Vec<String> = Vec::with_capacity(0);
        vec
    }};
}

macro_rules! plr {
        () => {
            str_vec!()
        };
        ($($x:expr),+ $(,)?) => (
            vec![
                $($x.to_string()),+
            ]
        );
    }

macro_rules! plrs {
        () => {
            str_vec!(str_vec!())
        };
        ($($x:expr),+ $(,)?) => (
            vec![
                $(vec![
                    $x.to_string()
                ],)+
            ]
        );
    }

mod spilt_namerena_groups {
    use super::*;

    #[test]
    fn basic_spilt() {
        let raw_input = "a\nb\nc".to_string();
        let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
        assert_eq!(groups, (plrs!("a", "b", "c"), plr!()));

        let raw_input = "a\nb\nc\n".to_string();
        let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
        assert_eq!(groups, (plrs!("a", "b", "c"), plr!()));

        let raw_input = "a\nb\nc\n\n".to_string();
        let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
        assert_eq!(groups, (plrs!("a", "b", "c"), plr!()));
    }

    #[test]
    fn spilt_teams() {
        let raw_input = "a\nb\n\nc\nd".to_string();
        let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
        assert_eq!(groups, (vec![plr!["a", "b"], plr!["c", "d"]], plr!()));
    }

    #[test]
    fn more_than_2_newline() {
        for x in 2..10 {
            let new_lines = "\n".repeat(x);
            let raw_input = format!("a\nb{new_lines}c\nd");
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (vec![plr!["a", "b"], plr!["c", "d"]], plr!()));
        }

        for x in 2..10 {
            let new_lines = "\n".repeat(x);
            let raw_input = format!("a\nb{new_lines}c\nd{new_lines}e");
            let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
            assert_eq!(groups, (vec![plr!["a", "b"], plr!["c", "d"], plr!["e"]], plr!()));
        }
    }

    #[test]
    fn lot_of_teams() {
        let raw_input = "a\nb\nc\nd\ne\nf".to_string();
        let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
        assert_eq!(groups, (plrs!("a", "b", "c", "d", "e", "f"), plr!()));
    }

    #[test]
    fn normal_seed() {
        let raw_input = "seed: a@!\nb\nc".to_string();
        let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
        assert_eq!(groups, (plrs!("seed: a@!", "b", "c"), plr!["seed: a@!"]));
    }

    #[test]
    fn need_fix_seed1() {
        let raw_input = "aaaa\nbbbb\n\nseed: a@!".to_string();
        let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
        assert_eq!(groups, (vec![plr!("aaaa", "bbbb", "seed: a@!")], plr!["seed: a@!"]))
    }

    #[test]
    fn need_fix_seed2() {
        let raw_input = "seed: a@!\n\naaaa\nbbbb".to_string();
        let groups = runners::Runner::spilt_namerena_into_groups(raw_input);
        assert_eq!(groups, (vec![plr!("seed: a@!", "aaaa", "bbbb")], plr!["seed: a@!"]))
    }
}

mod runner {
    use super::*;
    use crate::engine::update::{RunUpdate, UpdateType};

    fn format_update_message(runner: &runners::Runner, update: &RunUpdate) -> String {
        let caster = runner
            .storage
            .get_player(&update.caster)
            .map(|plr| plr.id_name())
            .unwrap_or_else(|| format!("#{}", update.caster));
        let target = runner
            .storage
            .get_player(&update.target)
            .map(|plr| plr.id_name())
            .unwrap_or_else(|| format!("#{}", update.target));
        let mut msg = update.message.clone();
        msg = msg.replace("[0]", &caster);
        msg = msg.replace("[1]", &target);
        let param = if update.targets.is_empty() {
            update.score.to_string()
        } else {
            update
                .targets
                .iter()
                .map(|id| runner.storage.get_player(id).map(|plr| plr.id_name()).unwrap_or_else(|| format!("#{id}")))
                .collect::<Vec<String>>()
                .join(",")
        };
        msg.replace("[2]", &param)
    }

    fn normalize_trace_line(line: String) -> String {
        line.replace("[s_counter]", "")
            .replace("[s_dmg160]", "")
            .replace("[s_dmg120]", "")
            .replace("[s_dmg0]", "")
            .replace(['[', ']'], "")
            .replace(' ', "")
            .trim()
            .to_string()
    }

    fn collect_replay_events(runner: &mut runners::Runner, max_rounds: usize, normalize: bool) -> (Vec<String>, usize) {
        let mut events = Vec::new();
        let mut guard = 0usize;
        while !runner.have_winner() && guard < max_rounds {
            let updates = runner.main_round();
            for update in updates.updates {
                if matches!(update.update_type, UpdateType::NextLine) {
                    continue;
                }
                let mut msg = format_update_message(runner, &update);
                if normalize {
                    msg = normalize_trace_line(msg);
                    if msg.is_empty() {
                        continue;
                    }
                }
                events.push(msg);
            }
            guard += 1;
        }
        (events, guard)
    }

    fn collect_replay_lines(runner: &mut runners::Runner, max_rounds: usize, normalize: bool) -> (Vec<String>, usize) {
        let mut lines = Vec::new();
        let mut guard = 0usize;
        while !runner.have_winner() && guard < max_rounds {
            let updates = runner.main_round();
            let mut parts = Vec::new();
            for update in updates.updates {
                if matches!(update.update_type, UpdateType::NextLine) {
                    if !parts.is_empty() {
                        lines.push(parts.join(", "));
                        parts.clear();
                    }
                    continue;
                }
                let mut msg = format_update_message(runner, &update);
                if normalize {
                    msg = normalize_trace_line(msg);
                }
                if !msg.is_empty() {
                    parts.push(msg);
                }
            }
            if !parts.is_empty() {
                lines.push(parts.join(", "));
            }
            guard += 1;
        }
        (lines, guard)
    }

    fn parse_embedded_fight_case(case_text: &str, split_err: &str, empty_err: &str) -> (String, Vec<String>) {
        let fight_text = case_text.replace("\r\n", "\n").replace('\r', "\n");
        let (raw_input, expected_part) = fight_text.split_once("\n\n\n").expect(split_err);
        let expected_lines = expected_part
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| normalize_trace_line(line.to_string()))
            .filter(|line| !line.is_empty())
            .collect::<Vec<String>>();
        assert!(!expected_lines.is_empty(), "{empty_err}");
        (raw_input.trim_end().to_string(), expected_lines)
    }

    fn winner_names(runner: &runners::Runner) -> Vec<String> {
        runner
            .world
            .winner
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|id| {
                runner
                    .storage
                    .get_player(&id)
                    .map(|plr| plr.id_name())
                    .unwrap_or_else(|| format!("#{id}"))
            })
            .collect::<Vec<String>>()
    }

    fn assert_trace_with_context(case_name: &str, actual_lines: &[String], expected_lines: &[String]) {
        if actual_lines == expected_lines {
            return;
        }
        let min_len = actual_lines.len().min(expected_lines.len());
        let mismatch_idx = actual_lines
            .iter()
            .zip(expected_lines.iter())
            .position(|(lhs, rhs)| lhs != rhs)
            .unwrap_or(min_len);
        let ctx_start = mismatch_idx.saturating_sub(3);
        let ctx_end = (mismatch_idx + 3).min(min_len);
        eprintln!("{case_name} mismatch context [{ctx_start}..{ctx_end}):");
        for idx in ctx_start..ctx_end {
            eprintln!(
                "  idx={idx}: actual={:?} | expected={:?}",
                actual_lines.get(idx),
                expected_lines.get(idx)
            );
        }
        panic!(
            "{case_name} mismatch at idx={mismatch_idx}, actual_len={}, expected_len={}, actual={:?}, expected={:?}",
            actual_lines.len(),
            expected_lines.len(),
            actual_lines.get(mismatch_idx),
            expected_lines.get(mismatch_idx)
        );
    }

    fn strip_name_noise_suffix(line: &str) -> String {
        let bytes = line.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0usize;
        'scan: while i < bytes.len() {
            if bytes[i] == b'?' {
                for marker in [b"clone".as_slice(), b"summon".as_slice()] {
                    let marker_start = i + 1;
                    let marker_end = marker_start + marker.len();
                    if marker_end <= bytes.len() && &bytes[marker_start..marker_end] == marker {
                        let mut j = marker_end;
                        while j < bytes.len() && bytes[j].is_ascii_digit() {
                            j += 1;
                        }
                        if j > marker_end {
                            i = j;
                            continue 'scan;
                        }
                    }
                }
            }
            out.push(bytes[i]);
            i += 1;
        }
        String::from_utf8(out).expect("stripping ASCII suffix should keep valid UTF-8")
    }

    fn assert_trace_with_name_noise_ignored(case_name: &str, actual_lines: &[String], expected_lines: &[String]) {
        let normalized_actual = actual_lines.iter().map(|line| strip_name_noise_suffix(line)).collect::<Vec<String>>();
        let normalized_expected = expected_lines.iter().map(|line| strip_name_noise_suffix(line)).collect::<Vec<String>>();
        assert_trace_with_context(case_name, &normalized_actual, &normalized_expected);
    }

    #[test]
    fn sort_int_test() {
        let raw_input = "aaa\nbbb\nseed: aaaa@!";
        let runner = runners::Runner::new_from_namerena_raw(raw_input.to_string()).unwrap();

        let ints = [16_391_432, 11_292_362];
        assert!(!runner.have_winner());

        for (i, plr) in runner
            .world
            .groups
            .iter()
            .flatten()
            .filter(|plr| runner.storage.get_player(plr).expect("wtf").is_seed_plr())
            .enumerate()
        {
            let plr = runner.storage.get_player(plr).expect("plr not found");
            assert_eq!(plr.sort_int as u32, ints[i]);
        }
    }

    #[test]
    fn sort_int_test2() {
        let raw_input = "aaa\nbbb";
        let runner = runners::Runner::new_from_namerena_raw(raw_input.to_string()).unwrap();

        let ints = [7_525_315, 8_712_372];
        assert!(!runner.have_winner());

        for (i, plr) in runner.world.groups.iter().flatten().enumerate() {
            let plr = runner.storage.get_player(plr).expect("plr not found");
            assert_eq!(plr.sort_int as u32, ints[i]);
        }
    }

    #[test]
    fn input_order_should_not_change_initial_state() {
        let runner_ab = runners::Runner::new_from_namerena_raw("aaaaa\nhelp".to_string()).unwrap();
        let runner_ba = runners::Runner::new_from_namerena_raw("help\naaaaa".to_string()).unwrap();

        let mut state_ab = runner_ab
            .world
            .groups
            .iter()
            .flatten()
            .map(|id| runner_ab.storage.get_player(id).expect("plr not found in runner_ab"))
            .map(|plr| (plr.id_name(), plr.get_sort_int(), plr.move_point()))
            .collect::<Vec<(String, i32, i32)>>();
        state_ab.sort_by(|a, b| a.0.cmp(&b.0));

        let mut state_ba = runner_ba
            .world
            .groups
            .iter()
            .flatten()
            .map(|id| runner_ba.storage.get_player(id).expect("plr not found in runner_ba"))
            .map(|plr| (plr.id_name(), plr.get_sort_int(), plr.move_point()))
            .collect::<Vec<(String, i32, i32)>>();
        state_ba.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(state_ab, state_ba);
    }

    #[test]
    fn help_vs_aaaaa_should_match_right_trace_step_by_step() {
        let mut runner = runners::Runner::new_from_namerena_raw("help\naaaaa".to_string()).unwrap();
        let (events, guard) = collect_replay_events(&mut runner, 256, false);

        assert!(guard < 256, "combat did not finish in expected rounds");
        assert_eq!(
            events,
            vec![
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
                "aaaaa被击倒了"
            ]
        );

        let winner = winner_names(&runner);
        assert_eq!(winner, vec!["help".to_string()]);
    }

    #[test]
    fn seed_small_replay_should_match() {
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

    #[test]
    fn fight_simple_replay_should_match() {
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
        let (actual_lines, guard) = collect_replay_lines(&mut runner, 10_000, true);

        assert!(guard < 10_000, "fight_simple combat did not finish in expected rounds");
        assert_eq!(actual_lines, expected_lines);
    }

    // BEGIN sampled large cases (generated from test.md)

    #[test]
    fn sampled_large_case_01_replay_should_match() {
        const CASE: &str = r####"「OS」#c1#bFc71OCDuO35@mwh_425
血谣染硫决@Mithril425
锋利ⅤEGZPVQMY@TigerStar425
(S("p{GE2up',7%^UGrP@czr2012425
针刀霜|U/T)h8J"@四象柯425
东乡幻翎#BCBNRCXFX@无惨425
无惨不等式#YMGTFCOPE@星球结晶425
愞㢯老海@昀澤425
Imperio#4B4UZThv@Shabby_fish425
末otW7sfqOze@807139425


血谣染硫决发起攻击, 末otW7sfqOze受到69点伤害

无惨不等式#YMGTFCOPE发起攻击, 东乡幻翎#BCBNRCXFX受到54点伤害

Imperio#4B4UZThv使用魅惑, 锋利ⅤEGZPVQMY被魅惑了

(S("p{GE2up',7%^UGrP发起攻击, 锋利ⅤEGZPVQMY受到54点伤害

针刀霜|U/T)h8J"发起攻击, Imperio#4B4UZThv受到38点伤害

「OS」#c1#bFc71OCDuO35使用魅惑, (S("p{GE2up',7%^UGrP被魅惑了

末otW7sfqOze发起攻击, 东乡幻翎#BCBNRCXFX受到51点伤害

愞㢯老海发起攻击, 针刀霜|U/T)h8J"受到75点伤害

锋利ⅤEGZPVQMY发起攻击, 东乡幻翎#BCBNRCXFX受到107点伤害

 锋利ⅤEGZPVQMY从魅惑中解除

东乡幻翎#BCBNRCXFX发起攻击, (S("p{GE2up',7%^UGrP受到60点伤害

血谣染硫决使用幻术, 召唤出幻影

东乡幻翎#BCBNRCXFX发起攻击, 锋利ⅤEGZPVQMY回避了攻击

末otW7sfqOze使用诅咒, 「OS」#c1#bFc71OCDuO35受到124点伤害, 「OS」#c1#bFc71OCDuO35被诅咒了

无惨不等式#YMGTFCOPE发起攻击, 东乡幻翎#BCBNRCXFX回避了攻击

Imperio#4B4UZThv发起攻击, 无惨不等式#YMGTFCOPE受到38点伤害

愞㢯老海发起攻击, (S("p{GE2up',7%^UGrP受到73点伤害

「OS」#c1#bFc71OCDuO35发起攻击, 血谣染硫决受到87点伤害

锋利ⅤEGZPVQMY发起攻击, 东乡幻翎#BCBNRCXFX受到82点伤害

血谣染硫决使用血祭, 召唤出使魔

针刀霜|U/T)h8J"发起攻击, 末otW7sfqOze受到38点伤害

末otW7sfqOze发起攻击, 无惨不等式#YMGTFCOPE回避了攻击

无惨不等式#YMGTFCOPE发动会心一击, 幻影受到154点伤害

 幻影消失了

东乡幻翎#BCBNRCXFX发起攻击, 针刀霜|U/T)h8J"受到77点伤害

(S("p{GE2up',7%^UGrP发起攻击, 东乡幻翎#BCBNRCXFX受到36点伤害

 东乡幻翎#BCBNRCXFX被击倒了

 (S("p{GE2up',7%^UGrP从魅惑中解除

使魔发起攻击, 针刀霜|U/T)h8J"受到43点伤害

Imperio#4B4UZThv发起攻击, 诅咒使伤害加倍, 「OS」#c1#bFc71OCDuO35受到130点伤害

 「OS」#c1#bFc71OCDuO35被击倒了

愞㢯老海发起攻击, 血谣染硫决受到41点伤害

 血谣染硫决发起反击, 愞㢯老海受到55点伤害

锋利ⅤEGZPVQMY发起攻击, (S("p{GE2up',7%^UGrP受到107点伤害

血谣染硫决发起攻击, (S("p{GE2up',7%^UGrP受到107点伤害

 (S("p{GE2up',7%^UGrP被击倒了

使魔发起攻击, 无惨不等式#YMGTFCOPE受到25点伤害

Imperio#4B4UZThv使用分身, 出现一个新的Imperio#4B4UZThv

无惨不等式#YMGTFCOPE发起攻击, Imperio#4B4UZThv受到88点伤害

末otW7sfqOze发起攻击, 针刀霜|U/T)h8J"受到101点伤害

针刀霜|U/T)h8J"发起攻击, 锋利ⅤEGZPVQMY受到83点伤害

使魔发起攻击, Imperio#4B4UZThv受到76点伤害

愞㢯老海发起攻击, 锋利ⅤEGZPVQMY回避了攻击

Imperio#4B4UZThv发起攻击, 愞㢯老海受到72点伤害

锋利ⅤEGZPVQMY发起攻击, 末otW7sfqOze受到51点伤害

血谣染硫决发起攻击, 锋利ⅤEGZPVQMY受到77点伤害

末otW7sfqOze发起攻击, 锋利ⅤEGZPVQMY受到50点伤害

Imperio#4B4UZThv发起攻击, 愞㢯老海受到61点伤害

 Imperio#4B4UZThv连击, 针刀霜|U/T)h8J"防御, 针刀霜|U/T)h8J"受到40点伤害

 针刀霜|U/T)h8J"被击倒了

愞㢯老海发起攻击, 血谣染硫决受到32点伤害

无惨不等式#YMGTFCOPE发动会心一击, 使魔受到126点伤害, 血谣染硫决受到63点伤害

 血谣染硫决被击倒了

 使魔消失了

Imperio#4B4UZThv使用魅惑, 无惨不等式#YMGTFCOPE被魅惑了

锋利ⅤEGZPVQMY发起攻击, Imperio#4B4UZThv受到74点伤害

 Imperio#4B4UZThv被击倒了

无惨不等式#YMGTFCOPE发动会心一击, 末otW7sfqOze受到99点伤害

 无惨不等式#YMGTFCOPE从魅惑中解除

末otW7sfqOze发起攻击, 无惨不等式#YMGTFCOPE受到35点伤害

Imperio#4B4UZThv发起攻击, 愞㢯老海受到105点伤害

 愞㢯老海被击倒了

锋利ⅤEGZPVQMY发起攻击, 无惨不等式#YMGTFCOPE防御, 无惨不等式#YMGTFCOPE受到23点伤害

无惨不等式#YMGTFCOPE发起攻击, Imperio#4B4UZThv受到51点伤害

 Imperio#4B4UZThv被击倒了

末otW7sfqOze发起攻击, 无惨不等式#YMGTFCOPE受到47点伤害

无惨不等式#YMGTFCOPE发起攻击, 末otW7sfqOze使用伤害反弹, 无惨不等式#YMGTFCOPE受到44点伤害

锋利ⅤEGZPVQMY发起攻击, 末otW7sfqOze受到103点伤害

 末otW7sfqOze被击倒了

无惨不等式#YMGTFCOPE使用冰冻术, 锋利ⅤEGZPVQMY受到40点伤害, 锋利ⅤEGZPVQMY被冰冻了

无惨不等式#YMGTFCOPE发起攻击, 锋利ⅤEGZPVQMY受到134点伤害

 锋利ⅤEGZPVQMY被击倒了"####;
        let (raw_input, expected_lines) = parse_embedded_fight_case(
            CASE,
            "sampled case-01 must contain a blank separator between input and trace",
            "sampled case-01 trace is empty",
        );

        let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
        let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);

        assert!(guard < 20_000, "sampled case-01 combat did not finish in expected rounds");
        assert_trace_with_context("sampled case-01", &actual_lines, &expected_lines);
    }

    #[test]
    fn sampled_large_case_02_replay_should_match() {
        const CASE: &str = r####"「OS」#H1#YoRmfG4zW9@mwh_425
「OS」#c1#E7WGTekQTugF@mwh_425
RedOT<{f2=v}67w@流浪冒险者425
mVf4YCPDlRm@tyakasha425
十六夜咲夜zgJ6eH3TkLFp@芒萁425
Sayakagh8yaICYo@candle425
我力7#W2ib8D@仙蛊屋425
SDPC#AZLZJQUPN@星球结晶425
稗田阿求OQL68NN8@Squall425
跙坥咀诅阻珇伹伹怚@涵虚425


RedOT<{f2=v}67w发起攻击, 「OS」#H1#YoRmfG4zW9回避了攻击

稗田阿求OQL68NN8发起攻击, Sayakagh8yaICYo受到27点伤害

Sayakagh8yaICYo使用幻术, 召唤出幻影

我力7#W2ib8D发动会心一击, RedOT<{f2=v}67w受到154点伤害

十六夜咲夜zgJ6eH3TkLFp发起攻击, SDPC#AZLZJQUPN回避了攻击

跙坥咀诅阻珇伹伹怚发起攻击, 稗田阿求OQL68NN8受到44点伤害

mVf4YCPDlRm发起攻击, SDPC#AZLZJQUPN受到88点伤害

SDPC#AZLZJQUPN发起攻击, Sayakagh8yaICYo受到47点伤害

「OS」#c1#E7WGTekQTugF发起攻击, Sayakagh8yaICYo受到79点伤害

RedOT<{f2=v}67w发起攻击, 「OS」#c1#E7WGTekQTugF回避了攻击

稗田阿求OQL68NN8发起攻击, 「OS」#c1#E7WGTekQTugF受到76点伤害

「OS」#H1#YoRmfG4zW9发起攻击, 「OS」#c1#E7WGTekQTugF受到80点伤害

十六夜咲夜zgJ6eH3TkLFp发起攻击, 稗田阿求OQL68NN8受到56点伤害

Sayakagh8yaICYo发起攻击, 十六夜咲夜zgJ6eH3TkLFp受到52点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 我力7#W2ib8D受到65点伤害

mVf4YCPDlRm发起攻击, 幻影受到73点伤害

SDPC#AZLZJQUPN发起攻击, 稗田阿求OQL68NN8受到138点伤害

稗田阿求OQL68NN8发起攻击, Sayakagh8yaICYo受到74点伤害

我力7#W2ib8D发起攻击, Sayakagh8yaICYo受到71点伤害

 Sayakagh8yaICYo被击倒了

 幻影消失了

 我力7#W2ib8D吞噬了Sayakagh8yaICYo, 我力7#W2ib8D属性上升

我力7#W2ib8D投毒, mVf4YCPDlRm受到77点伤害, mVf4YCPDlRm中毒

RedOT<{f2=v}67w使用净化, 「OS」#c1#E7WGTekQTugF受到47点伤害

「OS」#c1#E7WGTekQTugF发起攻击, 「OS」#H1#YoRmfG4zW9受到46点伤害

十六夜咲夜zgJ6eH3TkLFp发起攻击, RedOT<{f2=v}67w受到32点伤害

「OS」#H1#YoRmfG4zW9发起攻击, RedOT<{f2=v}67w受到134点伤害

 RedOT<{f2=v}67w被击倒了

跙坥咀诅阻珇伹伹怚使用魅惑, 十六夜咲夜zgJ6eH3TkLFp被魅惑了

稗田阿求OQL68NN8发起攻击, 十六夜咲夜zgJ6eH3TkLFp受到103点伤害

「OS」#c1#E7WGTekQTugF发起攻击, 我力7#W2ib8D受到24点伤害

我力7#W2ib8D发动会心一击, 跙坥咀诅阻珇伹伹怚受到166点伤害

SDPC#AZLZJQUPN发起攻击, 跙坥咀诅阻珇伹伹怚受到52点伤害

mVf4YCPDlRm发起攻击, SDPC#AZLZJQUPN受到37点伤害

 mVf4YCPDlRm毒性发作, mVf4YCPDlRm受到36点伤害

「OS」#c1#E7WGTekQTugF发起吸血攻击, SDPC#AZLZJQUPN受到132点伤害, 「OS」#c1#E7WGTekQTugF回复体力66点

稗田阿求OQL68NN8使用幻术, 召唤出幻影

十六夜咲夜zgJ6eH3TkLFp发起攻击, 「OS」#H1#YoRmfG4zW9受到34点伤害

 十六夜咲夜zgJ6eH3TkLFp从魅惑中解除

SDPC#AZLZJQUPN发起攻击, 我力7#W2ib8D受到54点伤害

跙坥咀诅阻珇伹伹怚发起吸血攻击, 「OS」#H1#YoRmfG4zW9受到128点伤害, 跙坥咀诅阻珇伹伹怚回复体力64点

我力7#W2ib8D发动会心一击, 跙坥咀诅阻珇伹伹怚受到128点伤害

「OS」#H1#YoRmfG4zW9使用减速术, 幻影进入迟缓状态

SDPC#AZLZJQUPN潜行到mVf4YCPDlRm身后

跙坥咀诅阻珇伹伹怚发起攻击, 幻影受到43点伤害

mVf4YCPDlRm发起攻击, 十六夜咲夜zgJ6eH3TkLFp防御, 十六夜咲夜zgJ6eH3TkLFp受到66点伤害

 mVf4YCPDlRm毒性发作, mVf4YCPDlRm受到30点伤害

稗田阿求OQL68NN8发起攻击, 我力7#W2ib8D受到36点伤害

「OS」#c1#E7WGTekQTugF使用冰冻术, 我力7#W2ib8D受到17点伤害, 我力7#W2ib8D被冰冻了

十六夜咲夜zgJ6eH3TkLFp发起攻击, SDPC#AZLZJQUPN受到28点伤害

 SDPC#AZLZJQUPN的潜行被识破

SDPC#AZLZJQUPN发起攻击, 幻影受到51点伤害

 SDPC#AZLZJQUPN连击, 幻影受到29点伤害

 SDPC#AZLZJQUPN连击, 幻影受到29点伤害

「OS」#H1#YoRmfG4zW9发起攻击, 幻影受到81点伤害

 幻影消失了

稗田阿求OQL68NN8发起攻击, 「OS」#c1#E7WGTekQTugF受到110点伤害

 「OS」#c1#E7WGTekQTugF做出垂死抗争, 「OS」#c1#E7WGTekQTugF所有属性上升

我力7#W2ib8D从冰冻中解除

跙坥咀诅阻珇伹伹怚使用魅惑, SDPC#AZLZJQUPN被魅惑了

「OS」#c1#E7WGTekQTugF发起攻击, mVf4YCPDlRm受到54点伤害

SDPC#AZLZJQUPN发起攻击, 「OS」#c1#E7WGTekQTugF受到29点伤害

 「OS」#c1#E7WGTekQTugF被击倒了

 SDPC#AZLZJQUPN连击, 「OS」#H1#YoRmfG4zW9受到37点伤害

 SDPC#AZLZJQUPN从魅惑中解除

我力7#W2ib8D发起攻击, 稗田阿求OQL68NN8受到78点伤害

 稗田阿求OQL68NN8被击倒了

mVf4YCPDlRm发起攻击, 十六夜咲夜zgJ6eH3TkLFp受到96点伤害

 十六夜咲夜zgJ6eH3TkLFp被击倒了

 mVf4YCPDlRm毒性发作, mVf4YCPDlRm受到25点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 我力7#W2ib8D受到53点伤害

 我力7#W2ib8D被击倒了

SDPC#AZLZJQUPN发起攻击, 「OS」#H1#YoRmfG4zW9受到66点伤害

 「OS」#H1#YoRmfG4zW9被击倒了

mVf4YCPDlRm发起攻击, SDPC#AZLZJQUPN受到38点伤害

 SDPC#AZLZJQUPN被击倒了

 mVf4YCPDlRm毒性发作, mVf4YCPDlRm受到20点伤害

 mVf4YCPDlRm被击倒了"####;
        let (raw_input, expected_lines) = parse_embedded_fight_case(
            CASE,
            "sampled case-02 must contain a blank separator between input and trace",
            "sampled case-02 trace is empty",
        );

        let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
        let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);

        assert!(guard < 20_000, "sampled case-02 combat did not finish in expected rounds");
        assert_trace_with_context("sampled case-02", &actual_lines, &expected_lines);
    }

    #[test]
    fn sampled_large_case_03_replay_should_match() {
        const CASE: &str = r####"#念-GP8LKM21D4JZ@柚子不是油渍425
Wakaba_mutsumi#pjFhEhSbjy@🥒425
Tachibana_akira#BydbIMidbs@🥒425
十六夜咲夜zgJ6eH3TkLFp@芒萁425
MeltelabRC3P3Go7@RbCl425
三田一重TxtrdTN4l8nT@fx425
SDPC#AZLZJQUPN@星球结晶425
七七#EUEMIGPI@暗黑突击425
Hypochondriac#TtwN3jZ@Unbound425
跙坥咀诅阻珇伹伹怚@涵虚425


三田一重TxtrdTN4l8nT发起攻击, SDPC#AZLZJQUPN受到93点伤害

Wakaba_mutsumi#pjFhEhSbjy发起攻击, 七七#EUEMIGPI回避了攻击

跙坥咀诅阻珇伹伹怚发起攻击, 七七#EUEMIGPI受到114点伤害

SDPC#AZLZJQUPN发起攻击, #念-GP8LKM21D4JZ受到86点伤害

 SDPC#AZLZJQUPN连击, Wakaba_mutsumi#pjFhEhSbjy受到40点伤害

七七#EUEMIGPI发起攻击, #念-GP8LKM21D4JZ受到70点伤害

Tachibana_akira#BydbIMidbs发起攻击, 跙坥咀诅阻珇伹伹怚受到109点伤害

#念-GP8LKM21D4JZ发起攻击, 十六夜咲夜zgJ6eH3TkLFp回避了攻击

MeltelabRC3P3Go7发起攻击, Tachibana_akira#BydbIMidbs受到110点伤害

Hypochondriac#TtwN3jZ使用魅惑, MeltelabRC3P3Go7被魅惑了

十六夜咲夜zgJ6eH3TkLFp发起攻击, MeltelabRC3P3Go7受到81点伤害

Tachibana_akira#BydbIMidbs发起攻击, SDPC#AZLZJQUPN受到58点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 三田一重TxtrdTN4l8nT受到115点伤害

七七#EUEMIGPI使用净化, MeltelabRC3P3Go7受到105点伤害

Wakaba_mutsumi#pjFhEhSbjy发起攻击, 七七#EUEMIGPI受到100点伤害

#念-GP8LKM21D4JZ发起攻击, MeltelabRC3P3Go7受到67点伤害

MeltelabRC3P3Go7发起攻击, 三田一重TxtrdTN4l8nT受到152点伤害

 MeltelabRC3P3Go7从魅惑中解除

三田一重TxtrdTN4l8nT发起攻击, Hypochondriac#TtwN3jZ回避了攻击

Hypochondriac#TtwN3jZ发起攻击, 跙坥咀诅阻珇伹伹怚受到42点伤害

SDPC#AZLZJQUPN发起攻击, 七七#EUEMIGPI受到86点伤害

十六夜咲夜zgJ6eH3TkLFp使用地裂术

 Wakaba_mutsumi#pjFhEhSbjy受到42点伤害

 三田一重TxtrdTN4l8nT受到15点伤害

 MeltelabRC3P3Go7受到20点伤害

 跙坥咀诅阻珇伹伹怚受到37点伤害

 SDPC#AZLZJQUPN受到30点伤害

Tachibana_akira#BydbIMidbs使用狂暴术, 十六夜咲夜zgJ6eH3TkLFp受到43点伤害, 十六夜咲夜zgJ6eH3TkLFp进入狂暴状态

MeltelabRC3P3Go7发起攻击, 三田一重TxtrdTN4l8nT受到119点伤害

 三田一重TxtrdTN4l8nT被击倒了

Hypochondriac#TtwN3jZ发起攻击, 十六夜咲夜zgJ6eH3TkLFp受到131点伤害

#念-GP8LKM21D4JZ发起攻击, Wakaba_mutsumi#pjFhEhSbjy受到61点伤害

跙坥咀诅阻珇伹伹怚发起攻击, #念-GP8LKM21D4JZ受到106点伤害

MeltelabRC3P3Go7发起攻击, SDPC#AZLZJQUPN受到32点伤害

SDPC#AZLZJQUPN发起攻击, 十六夜咲夜zgJ6eH3TkLFp防御, 十六夜咲夜zgJ6eH3TkLFp受到85点伤害

七七#EUEMIGPI发起攻击, 跙坥咀诅阻珇伹伹怚受到64点伤害

Wakaba_mutsumi#pjFhEhSbjy发起攻击, Hypochondriac#TtwN3jZ受到119点伤害

Hypochondriac#TtwN3jZ发动铁壁, Hypochondriac#TtwN3jZ防御力大幅上升

跙坥咀诅阻珇伹伹怚使用魅惑, 七七#EUEMIGPI回避了攻击

十六夜咲夜zgJ6eH3TkLFp发起狂暴攻击, #念-GP8LKM21D4JZ受到73点伤害

 #念-GP8LKM21D4JZ被击倒了

 十六夜咲夜zgJ6eH3TkLFp从狂暴中解除

七七#EUEMIGPI使用狂暴术, Hypochondriac#TtwN3jZ受到1点伤害, Hypochondriac#TtwN3jZ进入狂暴状态

MeltelabRC3P3Go7发起攻击, 十六夜咲夜zgJ6eH3TkLFp受到117点伤害

 十六夜咲夜zgJ6eH3TkLFp被击倒了

Tachibana_akira#BydbIMidbs发起攻击, Hypochondriac#TtwN3jZ受到1点伤害

SDPC#AZLZJQUPN发起攻击, Wakaba_mutsumi#pjFhEhSbjy受到88点伤害

Hypochondriac#TtwN3jZ发起狂暴攻击, Tachibana_akira#BydbIMidbs受到97点伤害

 Hypochondriac#TtwN3jZ从狂暴中解除

七七#EUEMIGPI发起攻击, Hypochondriac#TtwN3jZ受到1点伤害

MeltelabRC3P3Go7潜行到Hypochondriac#TtwN3jZ身后

跙坥咀诅阻珇伹伹怚发起吸血攻击, Hypochondriac#TtwN3jZ回避了攻击

Wakaba_mutsumi#pjFhEhSbjy使用诅咒, Hypochondriac#TtwN3jZ受到1点伤害, Hypochondriac#TtwN3jZ被诅咒了

Tachibana_akira#BydbIMidbs发起攻击, 跙坥咀诅阻珇伹伹怚受到92点伤害

 跙坥咀诅阻珇伹伹怚被击倒了

SDPC#AZLZJQUPN发起攻击, Wakaba_mutsumi#pjFhEhSbjy受到92点伤害

 Wakaba_mutsumi#pjFhEhSbjy被击倒了

Hypochondriac#TtwN3jZ发起攻击, SDPC#AZLZJQUPN受到79点伤害

 Hypochondriac#TtwN3jZ从铁壁中解除

MeltelabRC3P3Go7发动背刺, Hypochondriac#TtwN3jZ受到325点伤害

 Hypochondriac#TtwN3jZ被击倒了

七七#EUEMIGPI发起攻击, Tachibana_akira#BydbIMidbs受到82点伤害

 Tachibana_akira#BydbIMidbs被击倒了

SDPC#AZLZJQUPN发起攻击, MeltelabRC3P3Go7受到37点伤害

 MeltelabRC3P3Go7被击倒了

 SDPC#AZLZJQUPN连击, 七七#EUEMIGPI受到53点伤害

 七七#EUEMIGPI被击倒了"####;
        let (raw_input, expected_lines) = parse_embedded_fight_case(
            CASE,
            "sampled case-03 must contain a blank separator between input and trace",
            "sampled case-03 trace is empty",
        );

        let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
        let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);

        assert!(guard < 20_000, "sampled case-03 combat did not finish in expected rounds");
        assert_trace_with_context("sampled case-03", &actual_lines, &expected_lines);
    }

    #[test]
    fn sampled_large_case_04_replay_should_match() {
        const CASE: &str = r####"沉睡在悲伤的海洋中#056ARx3e@爱425
「OS」#c1#E7WGTekQTugF@mwh_425
RedOT<{f2=v}67w@流浪冒险者425
血谣染硫决@Mithril425
MeltelabRC3P3Go7@RbCl425
10l-DYWg@Hell425
(S("p{GE2up',7%^UGrP@czr2012425
东乡幻翎#BCBNRCXFX@无惨425
Hypochondriac#TtwN3jZ@Unbound425
seed:第十八届武术大赛抽签:425-0@!


Hypochondriac#TtwN3jZ发起攻击, 沉睡在悲伤的海洋中#056ARx3e受到50点伤害

东乡幻翎#BCBNRCXFX使用火球术, 血谣染硫决受到66点伤害

RedOT<{f2=v}67w发起攻击, 东乡幻翎#BCBNRCXFX受到69点伤害

血谣染硫决使用血祭, 召唤出使魔

(S("p{GE2up',7%^UGrP发起攻击, 使魔回避了攻击

10l-DYWg发起攻击, Hypochondriac#TtwN3jZ受到123点伤害

MeltelabRC3P3Go7潜行到沉睡在悲伤的海洋中#056ARx3e身后

「OS」#c1#E7WGTekQTugF使用幻术, 召唤出幻影

Hypochondriac#TtwN3jZ发起攻击, RedOT<{f2=v}67w受到55点伤害

沉睡在悲伤的海洋中#056ARx3e发起攻击, Hypochondriac#TtwN3jZ受到54点伤害

使魔发起攻击, Hypochondriac#TtwN3jZ受到67点伤害

10l-DYWg发起攻击, 「OS」#c1#E7WGTekQTugF受到108点伤害

(S("p{GE2up',7%^UGrP发动会心一击, 血谣染硫决受到104点伤害

东乡幻翎#BCBNRCXFX发起攻击, 10l-DYWg受到86点伤害

RedOT<{f2=v}67w使用魅惑, Hypochondriac#TtwN3jZ回避了攻击

「OS」#c1#E7WGTekQTugF使用治愈魔法, 「OS」#c1#E7WGTekQTugF回复体力108点

血谣染硫决发起攻击, (S("p{GE2up',7%^UGrP受到40点伤害

使魔发起攻击, 东乡幻翎#BCBNRCXFX受到47点伤害

MeltelabRC3P3Go7发动背刺, 沉睡在悲伤的海洋中#056ARx3e受到346点伤害

 沉睡在悲伤的海洋中#056ARx3e被击倒了

10l-DYWg发起攻击, (S("p{GE2up',7%^UGrP受到72点伤害

Hypochondriac#TtwN3jZ发起攻击, MeltelabRC3P3Go7受到60点伤害

使魔发起攻击, 幻影受到79点伤害

血谣染硫决发动铁壁, 血谣染硫决防御力大幅上升

RedOT<{f2=v}67w发起攻击, MeltelabRC3P3Go7受到87点伤害

东乡幻翎#BCBNRCXFX发起攻击, (S("p{GE2up',7%^UGrP受到73点伤害

「OS」#c1#E7WGTekQTugF发起攻击, 10l-DYWg受到55点伤害

使魔发起攻击, (S("p{GE2up',7%^UGrP受到80点伤害

幻影发起攻击, MeltelabRC3P3Go7受到101点伤害

MeltelabRC3P3Go7发起攻击, 「OS」#c1#E7WGTekQTugF受到38点伤害

(S("p{GE2up',7%^UGrP发起攻击, 血谣染硫决受到1点伤害

10l-DYWg发起攻击, 东乡幻翎#BCBNRCXFX受到63点伤害

Hypochondriac#TtwN3jZ发起攻击, (S("p{GE2up',7%^UGrP受到58点伤害

 (S("p{GE2up',7%^UGrP被击倒了

血谣染硫决发起攻击, RedOT<{f2=v}67w受到52点伤害

东乡幻翎#BCBNRCXFX发起攻击, MeltelabRC3P3Go7受到87点伤害

 MeltelabRC3P3Go7被击倒了

RedOT<{f2=v}67w发起攻击, 血谣染硫决受到1点伤害

 血谣染硫决发起反击, RedOT<{f2=v}67w受到128点伤害

使魔发起攻击, 「OS」#c1#E7WGTekQTugF回避了攻击

幻影发起攻击, RedOT<{f2=v}67w受到29点伤害

 RedOT<{f2=v}67w被击倒了

「OS」#c1#E7WGTekQTugF使用冰冻术, 10l-DYWg回避了攻击

10l-DYWg发起攻击, 幻影受到88点伤害

 幻影消失了

Hypochondriac#TtwN3jZ使用魅惑, 使魔被魅惑了

东乡幻翎#BCBNRCXFX发起攻击, 使魔受到111点伤害, 血谣染硫决受到55点伤害

 血谣染硫决被击倒了

 使魔消失了

10l-DYWg发起攻击, 「OS」#c1#E7WGTekQTugF受到70点伤害

Hypochondriac#TtwN3jZ发起攻击, 10l-DYWg受到100点伤害

「OS」#c1#E7WGTekQTugF使用冰冻术, 东乡幻翎#BCBNRCXFX回避了攻击

东乡幻翎#BCBNRCXFX发起攻击, 「OS」#c1#E7WGTekQTugF受到82点伤害

Hypochondriac#TtwN3jZ发起攻击, 东乡幻翎#BCBNRCXFX受到58点伤害

10l-DYWg发起攻击, 东乡幻翎#BCBNRCXFX受到69点伤害

「OS」#c1#E7WGTekQTugF发起攻击, 10l-DYWg受到88点伤害

 10l-DYWg被击倒了

Hypochondriac#TtwN3jZ发起攻击, 东乡幻翎#BCBNRCXFX受到122点伤害

 东乡幻翎#BCBNRCXFX被击倒了

「OS」#c1#E7WGTekQTugF使用净化, Hypochondriac#TtwN3jZ受到62点伤害

 Hypochondriac#TtwN3jZ被击倒了"####;
        let (raw_input, expected_lines) = parse_embedded_fight_case(
            CASE,
            "sampled case-04 must contain a blank separator between input and trace",
            "sampled case-04 trace is empty",
        );

        let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
        let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);

        assert!(guard < 20_000, "sampled case-04 combat did not finish in expected rounds");
        assert_trace_with_context("sampled case-04", &actual_lines, &expected_lines);
    }

    #[test]
    fn sampled_large_case_05_replay_should_match() {
        const CASE: &str = r####"「OS」#c1#bFc71OCDuO35@mwh_425
GordonALYJDXORPTER@nan425
"铁胆"哈拉文领主-ksbGnquBbq-@新纪元425
冥河WyO8MUZPPtKH@Afterglow425
tCtrVweRgshV@Afterglow425
湖心SHVPEMAPV@TigerStar425
地气14#emOKVY@仙蛊屋425
SDPC#AZLZJQUPN@星球结晶425
缇亚卡#WOVLHAESD@星球结晶425
直接命中#Dfdt3d2uT@Shabby_fish425


SDPC#AZLZJQUPN发起攻击, 「OS」#c1#bFc71OCDuO35受到67点伤害

 SDPC#AZLZJQUPN连击, 「OS」#c1#bFc71OCDuO35受到31点伤害

 「OS」#c1#bFc71OCDuO35发起反击, SDPC#AZLZJQUPN受到74点伤害

GordonALYJDXORPTER发起攻击, 冥河WyO8MUZPPtKH受到16点伤害

湖心SHVPEMAPV发起攻击, 地气14#emOKVY回避了攻击

直接命中#Dfdt3d2uT发起攻击, 「OS」#c1#bFc71OCDuO35受到67点伤害

"铁胆"哈拉文领主-ksbGnquBbq-发起攻击, 缇亚卡#WOVLHAESD受到97点伤害

「OS」#c1#bFc71OCDuO35发起攻击, 地气14#emOKVY受到144点伤害

tCtrVweRgshV发起攻击, GordonALYJDXORPTER受到62点伤害

缇亚卡#WOVLHAESD发起攻击, 直接命中#Dfdt3d2uT受到114点伤害

冥河WyO8MUZPPtKH发起攻击, 直接命中#Dfdt3d2uT受到37点伤害

地气14#emOKVY发起攻击, 冥河WyO8MUZPPtKH受到60点伤害

湖心SHVPEMAPV发起攻击, 缇亚卡#WOVLHAESD受到54点伤害

"铁胆"哈拉文领主-ksbGnquBbq-发起攻击, 地气14#emOKVY受到30点伤害

GordonALYJDXORPTER发起攻击, tCtrVweRgshV受到17点伤害

tCtrVweRgshV发起攻击, "铁胆"哈拉文领主-ksbGnquBbq-受到113点伤害

SDPC#AZLZJQUPN发起攻击, 地气14#emOKVY受到116点伤害

 地气14#emOKVY被击倒了

「OS」#c1#bFc71OCDuO35发起攻击, GordonALYJDXORPTER受到88点伤害

缇亚卡#WOVLHAESD发起攻击, 湖心SHVPEMAPV受到117点伤害

直接命中#Dfdt3d2uT发起攻击, 冥河WyO8MUZPPtKH受到42点伤害

"铁胆"哈拉文领主-ksbGnquBbq-发起攻击, 缇亚卡#WOVLHAESD受到24点伤害

SDPC#AZLZJQUPN发起攻击, 冥河WyO8MUZPPtKH受到77点伤害

冥河WyO8MUZPPtKH发起攻击, 湖心SHVPEMAPV回避了攻击

湖心SHVPEMAPV发起攻击, SDPC#AZLZJQUPN回避了攻击

GordonALYJDXORPTER发起攻击, "铁胆"哈拉文领主-ksbGnquBbq-受到51点伤害

「OS」#c1#bFc71OCDuO35发起攻击, SDPC#AZLZJQUPN受到73点伤害

缇亚卡#WOVLHAESD使用减速术, tCtrVweRgshV回避了攻击

SDPC#AZLZJQUPN发起攻击, 湖心SHVPEMAPV受到65点伤害

tCtrVweRgshV发起攻击, 直接命中#Dfdt3d2uT受到86点伤害

"铁胆"哈拉文领主-ksbGnquBbq-发起攻击, 直接命中#Dfdt3d2uT受到106点伤害

 直接命中#Dfdt3d2uT被击倒了

冥河WyO8MUZPPtKH开始蓄力

缇亚卡#WOVLHAESD发起攻击, SDPC#AZLZJQUPN受到57点伤害

tCtrVweRgshV发起攻击, "铁胆"哈拉文领主-ksbGnquBbq-受到94点伤害

GordonALYJDXORPTER发起攻击, 湖心SHVPEMAPV受到62点伤害

「OS」#c1#bFc71OCDuO35发起攻击, GordonALYJDXORPTER受到100点伤害

湖心SHVPEMAPV发起攻击, 冥河WyO8MUZPPtKH受到94点伤害

SDPC#AZLZJQUPN开始聚气, SDPC#AZLZJQUPN攻击力上升

冥河WyO8MUZPPtKH发起攻击, 「OS」#c1#bFc71OCDuO35受到305点伤害

 「OS」#c1#bFc71OCDuO35被击倒了

缇亚卡#WOVLHAESD使用冰冻术, 湖心SHVPEMAPV受到32点伤害

 湖心SHVPEMAPV被击倒了

"铁胆"哈拉文领主-ksbGnquBbq-发起攻击, GordonALYJDXORPTER受到44点伤害

 GordonALYJDXORPTER被击倒了

tCtrVweRgshV使用净化, 缇亚卡#WOVLHAESD受到92点伤害

SDPC#AZLZJQUPN发起攻击, 缇亚卡#WOVLHAESD受到124点伤害

 缇亚卡#WOVLHAESD被击倒了

冥河WyO8MUZPPtKH发起攻击, tCtrVweRgshV受到74点伤害

"铁胆"哈拉文领主-ksbGnquBbq-发起攻击, tCtrVweRgshV受到43点伤害

 "铁胆"哈拉文领主-ksbGnquBbq-连击, tCtrVweRgshV受到42点伤害

冥河WyO8MUZPPtKH使用魅惑, tCtrVweRgshV被魅惑了

tCtrVweRgshV发起攻击, "铁胆"哈拉文领主-ksbGnquBbq-受到90点伤害

 "铁胆"哈拉文领主-ksbGnquBbq-被击倒了

 tCtrVweRgshV从魅惑中解除

SDPC#AZLZJQUPN发起攻击, tCtrVweRgshV受到233点伤害

 tCtrVweRgshV被击倒了

SDPC#AZLZJQUPN发起攻击, 冥河WyO8MUZPPtKH受到108点伤害

 冥河WyO8MUZPPtKH被击倒了, 冥河WyO8MUZPPtKH使用护身符抵挡了一次死亡, 冥河WyO8MUZPPtKH回复体力8点

冥河WyO8MUZPPtKH使用魅惑, SDPC#AZLZJQUPN回避了攻击

SDPC#AZLZJQUPN发起攻击, 冥河WyO8MUZPPtKH受到68点伤害

 冥河WyO8MUZPPtKH被击倒了"####;
        let (raw_input, expected_lines) = parse_embedded_fight_case(
            CASE,
            "sampled case-05 must contain a blank separator between input and trace",
            "sampled case-05 trace is empty",
        );

        let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
        let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);

        assert!(guard < 20_000, "sampled case-05 combat did not finish in expected rounds");
        assert_trace_with_context("sampled case-05", &actual_lines, &expected_lines);
    }

    #[test]
    fn sampled_large_case_06_replay_should_match() {
        const CASE: &str = r####"都江堰00217109183087@abruce425
血谣染硫决@Mithril425
Straight_into_the_lights#VpdbCrcFJV@🥒425
仇决clFJZCMHS@candle425
1^GNC.%F@Hell425
前尘如梦UYGMHRNX@LuoTianyi425
我力7#W2ib8D@仙蛊屋425
东乡幻翎#BCBNRCXFX@无惨425
咲夜bJjbFYez@Squall425
Tik_Tok#IBxWzGZtr@Shabby_fish425


Straight_into_the_lights#VpdbCrcFJV发起攻击, 都江堰00217109183087受到77点伤害

咲夜bJjbFYez使用血祭, 召唤出使魔

1^GNC.%F发起攻击, 东乡幻翎#BCBNRCXFX受到81点伤害

都江堰00217109183087发动会心一击, 咲夜bJjbFYez受到131点伤害

前尘如梦UYGMHRNX发起攻击, 东乡幻翎#BCBNRCXFX受到43点伤害

Tik_Tok#IBxWzGZtr发起攻击, 我力7#W2ib8D受到81点伤害

东乡幻翎#BCBNRCXFX发起攻击, 前尘如梦UYGMHRNX受到110点伤害

血谣染硫决发起攻击, Tik_Tok#IBxWzGZtr受到114点伤害

我力7#W2ib8D发起攻击, Straight_into_the_lights#VpdbCrcFJV回避了攻击

使魔发起攻击, Straight_into_the_lights#VpdbCrcFJV受到21点伤害

Tik_Tok#IBxWzGZtr发起攻击, 前尘如梦UYGMHRNX受到73点伤害

Straight_into_the_lights#VpdbCrcFJV发起攻击, Tik_Tok#IBxWzGZtr受到67点伤害

咲夜bJjbFYez发起攻击, 1^GNC.%F受到85点伤害

仇决clFJZCMHS发起攻击, 1^GNC.%F受到62点伤害

血谣染硫决发起攻击, 仇决clFJZCMHS受到116点伤害

前尘如梦UYGMHRNX发起攻击, 都江堰00217109183087受到69点伤害

Straight_into_the_lights#VpdbCrcFJV发起攻击, 仇决clFJZCMHS受到88点伤害

1^GNC.%F发起攻击, Tik_Tok#IBxWzGZtr受到35点伤害

都江堰00217109183087发起攻击, 血谣染硫决受到96点伤害

Tik_Tok#IBxWzGZtr发起攻击, 1^GNC.%F受到73点伤害

我力7#W2ib8D发起攻击, 东乡幻翎#BCBNRCXFX受到69点伤害

东乡幻翎#BCBNRCXFX使用冰冻术, 前尘如梦UYGMHRNX受到50点伤害, 前尘如梦UYGMHRNX被冰冻了

咲夜bJjbFYez使用净化, Straight_into_the_lights#VpdbCrcFJV受到25点伤害

使魔发起攻击, 都江堰00217109183087受到63点伤害

仇决clFJZCMHS发起攻击, 都江堰00217109183087回避了攻击

我力7#W2ib8D发起攻击, Straight_into_the_lights#VpdbCrcFJV受到53点伤害

Straight_into_the_lights#VpdbCrcFJV发起攻击, 1^GNC.%F受到168点伤害

 1^GNC.%F被击倒了

血谣染硫决发动铁壁, 血谣染硫决防御力大幅上升

使魔使用自爆, 我力7#W2ib8D受到119点伤害

 使魔消失了

咲夜bJjbFYez使用治愈魔法, 咲夜bJjbFYez回复体力85点

仇决clFJZCMHS发起攻击, Tik_Tok#IBxWzGZtr受到40点伤害

Tik_Tok#IBxWzGZtr发起攻击, 血谣染硫决受到1点伤害

都江堰00217109183087发起攻击, 仇决clFJZCMHS回避了攻击

咲夜bJjbFYez发起攻击, Straight_into_the_lights#VpdbCrcFJV受到40点伤害

 咲夜bJjbFYez连击, 都江堰00217109183087受到48点伤害

 咲夜bJjbFYez连击, Straight_into_the_lights#VpdbCrcFJV受到36点伤害

Straight_into_the_lights#VpdbCrcFJV发起攻击, 我力7#W2ib8D受到65点伤害

 我力7#W2ib8D被击倒了

前尘如梦UYGMHRNX从冰冻中解除

东乡幻翎#BCBNRCXFX使用冰冻术, Straight_into_the_lights#VpdbCrcFJV回避了攻击

前尘如梦UYGMHRNX发起攻击, 咲夜bJjbFYez受到31点伤害

Tik_Tok#IBxWzGZtr发起攻击, 咲夜bJjbFYez受到69点伤害

咲夜bJjbFYez发起攻击, 都江堰00217109183087回避了攻击

仇决clFJZCMHS发起攻击, 血谣染硫决受到1点伤害

Straight_into_the_lights#VpdbCrcFJV发起攻击, 咲夜bJjbFYez受到64点伤害

血谣染硫决发起攻击, 仇决clFJZCMHS回避了攻击

东乡幻翎#BCBNRCXFX发起攻击, Straight_into_the_lights#VpdbCrcFJV受到36点伤害

Tik_Tok#IBxWzGZtr发起攻击, 血谣染硫决受到1点伤害

 血谣染硫决发起反击, Tik_Tok#IBxWzGZtr防御, Tik_Tok#IBxWzGZtr受到29点伤害

前尘如梦UYGMHRNX使用生命之轮, 东乡幻翎#BCBNRCXFX的体力值与前尘如梦UYGMHRNX互换

咲夜bJjbFYez发起攻击, 前尘如梦UYGMHRNX受到80点伤害

都江堰00217109183087发起攻击, Straight_into_the_lights#VpdbCrcFJV受到99点伤害

 Straight_into_the_lights#VpdbCrcFJV被击倒了

仇决clFJZCMHS发起攻击, 血谣染硫决受到1点伤害

血谣染硫决发起攻击, 都江堰00217109183087受到18点伤害

 血谣染硫决从铁壁中解除

Tik_Tok#IBxWzGZtr发起攻击, 血谣染硫决防御, 血谣染硫决受到71点伤害

东乡幻翎#BCBNRCXFX发起攻击, 血谣染硫决受到70点伤害

 血谣染硫决被击倒了

咲夜bJjbFYez发起攻击, 仇决clFJZCMHS受到61点伤害

都江堰00217109183087发起攻击, 东乡幻翎#BCBNRCXFX受到30点伤害

东乡幻翎#BCBNRCXFX发起攻击, 咲夜bJjbFYez受到80点伤害

 咲夜bJjbFYez被击倒了

前尘如梦UYGMHRNX发起攻击, 东乡幻翎#BCBNRCXFX受到99点伤害

 东乡幻翎#BCBNRCXFX被击倒了

仇决clFJZCMHS发起攻击, Tik_Tok#IBxWzGZtr受到60点伤害

 Tik_Tok#IBxWzGZtr被击倒了

都江堰00217109183087使用火球术, 仇决clFJZCMHS受到156点伤害

 仇决clFJZCMHS被击倒了

前尘如梦UYGMHRNX发起攻击, 都江堰00217109183087受到44点伤害

都江堰00217109183087使用地裂术

 前尘如梦UYGMHRNX受到103点伤害

 前尘如梦UYGMHRNX被击倒了"####;
        let (raw_input, expected_lines) = parse_embedded_fight_case(
            CASE,
            "sampled case-06 must contain a blank separator between input and trace",
            "sampled case-06 trace is empty",
        );

        let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
        let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);

        assert!(guard < 20_000, "sampled case-06 combat did not finish in expected rounds");
        assert_trace_with_context("sampled case-06", &actual_lines, &expected_lines);
    }

    #[test]
    fn sampled_large_case_07_replay_should_match() {
        const CASE: &str = r####"mVf4YCPDlRm@tyakasha425
➐M1jC95o@新纪元425
锋利ⅤEGZPVQMY@TigerStar425
BZoPIow@酸橙425
10l-DYWg@Hell425
冷霞洞.鸣湘榔狞@四象柯425
樱井光#CQMQFHIEV@无惨425
运松翁nkJspy1Oh54A@橙红耀阳425
Hypochondriac#TtwN3jZ@Unbound425
跙坥咀诅阻珇伹伹怚@涵虚425


冷霞洞.鸣湘榔狞使用血祭, 召唤出使魔

BZoPIow发起攻击, mVf4YCPDlRm使用伤害反弹, BZoPIow受到18点伤害

运松翁nkJspy1Oh54A发起攻击, 跙坥咀诅阻珇伹伹怚受到45点伤害

樱井光#CQMQFHIEV发起攻击, 运松翁nkJspy1Oh54A受到53点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 锋利ⅤEGZPVQMY受到19点伤害

10l-DYWg发起攻击, 冷霞洞.鸣湘榔狞受到65点伤害

Hypochondriac#TtwN3jZ使用地裂术

 锋利ⅤEGZPVQMY受到40点伤害

 冷霞洞.鸣湘榔狞受到40点伤害

 ➐M1jC95o受到22点伤害

 BZoPIow受到21点伤害

 mVf4YCPDlRm受到13点伤害

mVf4YCPDlRm发起攻击, 冷霞洞.鸣湘榔狞受到52点伤害

使魔发起攻击, mVf4YCPDlRm受到76点伤害

锋利ⅤEGZPVQMY发起攻击, Hypochondriac#TtwN3jZ受到57点伤害

➐M1jC95o发起攻击, 锋利ⅤEGZPVQMY受到65点伤害

BZoPIow发起攻击, 樱井光#CQMQFHIEV受到50点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 10l-DYWg受到111点伤害

樱井光#CQMQFHIEV发起攻击, 运松翁nkJspy1Oh54A回避了攻击

Hypochondriac#TtwN3jZ发起攻击, 运松翁nkJspy1Oh54A回避了攻击

10l-DYWg发起攻击, 运松翁nkJspy1Oh54A受到81点伤害

运松翁nkJspy1Oh54A潜行到使魔身后

使魔发起攻击, ➐M1jC95o回避了攻击

冷霞洞.鸣湘榔狞使用魅惑, BZoPIow被魅惑了

➐M1jC95o发起攻击, BZoPIow回避了攻击

BZoPIow发起攻击, 10l-DYWg受到73点伤害

 BZoPIow从魅惑中解除

跙坥咀诅阻珇伹伹怚发起攻击, 樱井光#CQMQFHIEV受到83点伤害

Hypochondriac#TtwN3jZ发起攻击, 冷霞洞.鸣湘榔狞受到33点伤害

mVf4YCPDlRm发起攻击, BZoPIow受到61点伤害

锋利ⅤEGZPVQMY使用减速术, mVf4YCPDlRm进入迟缓状态

运松翁nkJspy1Oh54A发动背刺, 使魔受到374点伤害, 冷霞洞.鸣湘榔狞受到187点伤害

 冷霞洞.鸣湘榔狞被击倒了

 使魔消失了

10l-DYWg发起攻击, ➐M1jC95o受到83点伤害

樱井光#CQMQFHIEV发起攻击, 跙坥咀诅阻珇伹伹怚受到55点伤害

跙坥咀诅阻珇伹伹怚发起攻击, Hypochondriac#TtwN3jZ受到68点伤害

运松翁nkJspy1Oh54A使用瘟疫, BZoPIow体力减少56%

➐M1jC95o使用加速术, ➐M1jC95o进入疾走状态

Hypochondriac#TtwN3jZ发起攻击, 樱井光#CQMQFHIEV受到72点伤害

BZoPIow发起攻击, Hypochondriac#TtwN3jZ受到90点伤害

10l-DYWg发起攻击, 跙坥咀诅阻珇伹伹怚受到123点伤害

锋利ⅤEGZPVQMY发起攻击, Hypochondriac#TtwN3jZ回避了攻击

➐M1jC95o发起攻击, 锋利ⅤEGZPVQMY受到52点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 10l-DYWg受到114点伤害

 10l-DYWg被击倒了

 跙坥咀诅阻珇伹伹怚召唤亡灵, 10l-DYWg变成了丧尸

运松翁nkJspy1Oh54A使用分身, 出现一个新的运松翁nkJspy1Oh54A

樱井光#CQMQFHIEV发起攻击, mVf4YCPDlRm受到101点伤害

➐M1jC95o发起攻击, 跙坥咀诅阻珇伹伹怚受到54点伤害

 ➐M1jC95o从疾走中解除

mVf4YCPDlRm发起攻击, 锋利ⅤEGZPVQMY使用伤害反弹, mVf4YCPDlRm回避了攻击

BZoPIow发起攻击, 运松翁nkJspy1Oh54A受到42点伤害

➐M1jC95o发起攻击, BZoPIow受到65点伤害

 BZoPIow做出垂死抗争, BZoPIow所有属性上升

丧尸发起攻击, 运松翁nkJspy1Oh54A受到48点伤害

 运松翁nkJspy1Oh54A被击倒了

跙坥咀诅阻珇伹伹怚发起攻击, BZoPIow受到68点伤害

 BZoPIow被击倒了

Hypochondriac#TtwN3jZ发起攻击, ➐M1jC95o受到65点伤害

锋利ⅤEGZPVQMY发起攻击, Hypochondriac#TtwN3jZ受到50点伤害

运松翁nkJspy1Oh54A发起攻击, 锋利ⅤEGZPVQMY受到21点伤害

樱井光#CQMQFHIEV使用幻术, 召唤出幻影

跙坥咀诅阻珇伹伹怚发起攻击, 幻影受到82点伤害

Hypochondriac#TtwN3jZ使用地裂术

 丧尸受到17点伤害

 樱井光#CQMQFHIEV受到36点伤害

 幻影受到38点伤害

 运松翁nkJspy1Oh54A受到0点伤害

 ➐M1jC95o受到24点伤害

➐M1jC95o发起攻击, 樱井光#CQMQFHIEV受到103点伤害

 樱井光#CQMQFHIEV被击倒了

 幻影消失了

丧尸发起攻击, ➐M1jC95o受到11点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 运松翁nkJspy1Oh54A受到109点伤害

 运松翁nkJspy1Oh54A被击倒了

mVf4YCPDlRm发起攻击, 锋利ⅤEGZPVQMY受到69点伤害

 mVf4YCPDlRm从迟缓中解除

锋利ⅤEGZPVQMY发起攻击, 丧尸受到74点伤害

Hypochondriac#TtwN3jZ发起攻击, 丧尸受到47点伤害

丧尸发起攻击, 锋利ⅤEGZPVQMY受到49点伤害

 锋利ⅤEGZPVQMY被击倒了

➐M1jC95o使用分身, 出现一个新的➐M1jC95o

Hypochondriac#TtwN3jZ发起攻击, ➐M1jC95o受到99点伤害

 ➐M1jC95o被击倒了

➐M1jC95o发起攻击, 跙坥咀诅阻珇伹伹怚受到41点伤害

 跙坥咀诅阻珇伹伹怚被击倒了

 丧尸消失了

mVf4YCPDlRm发起攻击, ➐M1jC95o受到152点伤害

 ➐M1jC95o被击倒了

Hypochondriac#TtwN3jZ发起攻击, mVf4YCPDlRm受到72点伤害

 mVf4YCPDlRm被击倒了"####;
        let (raw_input, expected_lines) = parse_embedded_fight_case(
            CASE,
            "sampled case-07 must contain a blank separator between input and trace",
            "sampled case-07 trace is empty",
        );

        let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
        let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);

        assert!(guard < 20_000, "sampled case-07 combat did not finish in expected rounds");
        assert_trace_with_name_noise_ignored("sampled case-07", &actual_lines, &expected_lines);
    }

    #[test]
    fn sampled_large_case_08_replay_should_match() {
        const CASE: &str = r####"「OS」#c1#bFc71OCDuO35@mwh_425
#念-GP8LKM21D4JZ@柚子不是油渍425
RedOT<{f2=v}67w@流浪冒险者425
➐M1jC95o@新纪元425
Reku_Mochizuki#494460162188@新纪元425
Sayakagh8yaICYo@candle425
Eaquirasd2D5HoYES@RbCl425
tCtrVweRgshV@Afterglow425
神谷紫苑#EUKSOXAA@暗黑突击425
Obsession#EYNIZRX@Unbound425


Sayakagh8yaICYo发起攻击, 神谷紫苑#EUKSOXAA回避了攻击

➐M1jC95o发起攻击, Reku_Mochizuki#494460162188受到80点伤害

Reku_Mochizuki#494460162188发起攻击, 神谷紫苑#EUKSOXAA受到61点伤害

#念-GP8LKM21D4JZ发起攻击, Eaquirasd2D5HoYES回避了攻击

Eaquirasd2D5HoYES使用地裂术

 Obsession#EYNIZRX受到21点伤害

 Sayakagh8yaICYo受到8点伤害

 Reku_Mochizuki#494460162188回避了攻击

 #念-GP8LKM21D4JZ受到39点伤害

 tCtrVweRgshV回避了攻击

tCtrVweRgshV发起攻击, #念-GP8LKM21D4JZ受到123点伤害

RedOT<{f2=v}67w发起攻击, Reku_Mochizuki#494460162188受到62点伤害

Obsession#EYNIZRX发起攻击, Eaquirasd2D5HoYES回避了攻击

「OS」#c1#bFc71OCDuO35开始聚气, 「OS」#c1#bFc71OCDuO35攻击力上升

Reku_Mochizuki#494460162188发起攻击, RedOT<{f2=v}67w受到94点伤害

➐M1jC95o发起攻击, Obsession#EYNIZRX受到89点伤害

神谷紫苑#EUKSOXAA发起攻击, Obsession#EYNIZRX受到64点伤害

tCtrVweRgshV使用加速术, tCtrVweRgshV进入疾走状态

#念-GP8LKM21D4JZ发起攻击, Obsession#EYNIZRX受到75点伤害

RedOT<{f2=v}67w发起攻击, tCtrVweRgshV回避了攻击

Obsession#EYNIZRX发起攻击, tCtrVweRgshV受到52点伤害

tCtrVweRgshV发起攻击, RedOT<{f2=v}67w受到115点伤害

Sayakagh8yaICYo发起攻击, 「OS」#c1#bFc71OCDuO35受到39点伤害

Eaquirasd2D5HoYES发起攻击, 神谷紫苑#EUKSOXAA受到115点伤害

「OS」#c1#bFc71OCDuO35发起攻击, tCtrVweRgshV受到127点伤害

Reku_Mochizuki#494460162188使用雷击术

 Obsession#EYNIZRX受到25点伤害

 Obsession#EYNIZRX受到44点伤害

 Obsession#EYNIZRX被击倒了

tCtrVweRgshV发起攻击, ➐M1jC95o受到28点伤害

 tCtrVweRgshV从疾走中解除

➐M1jC95o发起攻击, Eaquirasd2D5HoYES受到63点伤害

神谷紫苑#EUKSOXAA发起攻击, #念-GP8LKM21D4JZ受到63点伤害

Sayakagh8yaICYo发起攻击, RedOT<{f2=v}67w受到47点伤害

 RedOT<{f2=v}67w被击倒了

#念-GP8LKM21D4JZ发起攻击, Eaquirasd2D5HoYES受到75点伤害

神谷紫苑#EUKSOXAA发起攻击, Sayakagh8yaICYo受到72点伤害

Eaquirasd2D5HoYES发起攻击, Sayakagh8yaICYo受到82点伤害

➐M1jC95o使用分身, 出现一个新的➐M1jC95o

「OS」#c1#bFc71OCDuO35发起攻击, ➐M1jC95o受到160点伤害

 ➐M1jC95o被击倒了

Reku_Mochizuki#494460162188发起攻击, Eaquirasd2D5HoYES受到75点伤害

tCtrVweRgshV发起攻击, 神谷紫苑#EUKSOXAA受到77点伤害

Sayakagh8yaICYo发起攻击, ➐M1jC95o受到86点伤害

#念-GP8LKM21D4JZ使用地裂术

 Sayakagh8yaICYo受到10点伤害

 ➐M1jC95o受到44点伤害

 ➐M1jC95o被击倒了

 #念-GP8LKM21D4JZ吞噬了➐M1jC95o, #念-GP8LKM21D4JZ属性上升

 Reku_Mochizuki#494460162188受到24点伤害

 神谷紫苑#EUKSOXAA受到29点伤害

 「OS」#c1#bFc71OCDuO35受到29点伤害

Eaquirasd2D5HoYES发起攻击, Sayakagh8yaICYo受到86点伤害

 Sayakagh8yaICYo被击倒了

#念-GP8LKM21D4JZ使用瘟疫, Reku_Mochizuki#494460162188体力减少64%

神谷紫苑#EUKSOXAA使用生命之轮, #念-GP8LKM21D4JZ的体力值与神谷紫苑#EUKSOXAA互换

tCtrVweRgshV使用加速术, tCtrVweRgshV进入疾走状态

神谷紫苑#EUKSOXAA发起攻击, 「OS」#c1#bFc71OCDuO35受到41点伤害

「OS」#c1#bFc71OCDuO35发起攻击, 神谷紫苑#EUKSOXAA受到158点伤害

 神谷紫苑#EUKSOXAA被击倒了

Reku_Mochizuki#494460162188发起攻击, #念-GP8LKM21D4JZ受到71点伤害

 #念-GP8LKM21D4JZ被击倒了

tCtrVweRgshV发起攻击, 「OS」#c1#bFc71OCDuO35受到126点伤害

 「OS」#c1#bFc71OCDuO35被击倒了

Reku_Mochizuki#494460162188发起攻击, Eaquirasd2D5HoYES受到22点伤害

tCtrVweRgshV发起攻击, Eaquirasd2D5HoYES回避了攻击

 tCtrVweRgshV从疾走中解除

Eaquirasd2D5HoYES发起攻击, tCtrVweRgshV受到53点伤害

tCtrVweRgshV发起攻击, Eaquirasd2D5HoYES受到31点伤害

Reku_Mochizuki#494460162188发起攻击, Eaquirasd2D5HoYES受到68点伤害

 Eaquirasd2D5HoYES被击倒了

tCtrVweRgshV发起攻击, Reku_Mochizuki#494460162188受到93点伤害

 Reku_Mochizuki#494460162188被击倒了"####;
        let (raw_input, expected_lines) = parse_embedded_fight_case(
            CASE,
            "sampled case-08 must contain a blank separator between input and trace",
            "sampled case-08 trace is empty",
        );

        let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
        let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);

        assert!(guard < 20_000, "sampled case-08 combat did not finish in expected rounds");
        assert_trace_with_context("sampled case-08", &actual_lines, &expected_lines);
    }

    #[test]
    fn sampled_large_case_09_replay_should_match() {
        const CASE: &str = r####"#念-GP8LKM21D4JZ@柚子不是油渍425
血谣染硫决@Mithril425
仇决clFJZCMHS@candle425
态度jX2HoULfsFU9@Afterglow425
wangif9nWzNbxCJ7wXi8E@WDGod425
Imperio#4B4UZThv@Shabby_fish425
Obsession#EYNIZRX@Unbound425
末otW7sfqOze@807139425
<ζε>-fhepgq2n@ReturnVoid425
seed:第十八届武术大赛抽签:425-0@!


末otW7sfqOze使用诅咒, #念-GP8LKM21D4JZ回避了攻击

Imperio#4B4UZThv使用分身, 出现一个新的Imperio#4B4UZThv

Obsession#EYNIZRX发起攻击, #念-GP8LKM21D4JZ受到57点伤害

wangif9nWzNbxCJ7wXi8E使用火球术, 态度jX2HoULfsFU9受到64点伤害

<ζε>-fhepgq2n发起攻击, Imperio#4B4UZThv受到45点伤害

血谣染硫决发起攻击, 末otW7sfqOze受到79点伤害

末otW7sfqOze发起攻击, wangif9nWzNbxCJ7wXi8E受到72点伤害

态度jX2HoULfsFU9使用地裂术

 #念-GP8LKM21D4JZ受到40点伤害

 Imperio#4B4UZThv受到35点伤害

 Obsession#EYNIZRX受到58点伤害

 血谣染硫决受到44点伤害

仇决clFJZCMHS发起攻击, wangif9nWzNbxCJ7wXi8E受到80点伤害

#念-GP8LKM21D4JZ发起攻击, 态度jX2HoULfsFU9受到122点伤害

Imperio#4B4UZThv发起攻击, Obsession#EYNIZRX受到63点伤害

Obsession#EYNIZRX发起攻击, Imperio#4B4UZThv受到50点伤害

Imperio#4B4UZThv使用生命之轮, <ζε>-fhepgq2n的体力值与Imperio#4B4UZThv互换

<ζε>-fhepgq2n发起攻击, 血谣染硫决受到142点伤害

wangif9nWzNbxCJ7wXi8E发起攻击, 末otW7sfqOze回避了攻击

Obsession#EYNIZRX发起攻击, 末otW7sfqOze回避了攻击

末otW7sfqOze发起攻击, wangif9nWzNbxCJ7wXi8E受到73点伤害

态度jX2HoULfsFU9发起攻击, 末otW7sfqOze受到127点伤害

仇决clFJZCMHS发起攻击, 态度jX2HoULfsFU9受到90点伤害

#念-GP8LKM21D4JZ发起攻击, 末otW7sfqOze受到17点伤害

Imperio#4B4UZThv发起攻击, wangif9nWzNbxCJ7wXi8E受到114点伤害

 wangif9nWzNbxCJ7wXi8E被击倒了

血谣染硫决发起攻击, Obsession#EYNIZRX受到93点伤害

末otW7sfqOze发起攻击, #念-GP8LKM21D4JZ受到44点伤害

 #念-GP8LKM21D4JZ发起反击, 末otW7sfqOze受到35点伤害

Imperio#4B4UZThv发起攻击, #念-GP8LKM21D4JZ受到81点伤害

#念-GP8LKM21D4JZ使用地裂术

 仇决clFJZCMHS受到40点伤害

 末otW7sfqOze回避了攻击

 血谣染硫决回避了攻击

 Obsession#EYNIZRX受到58点伤害

 Imperio#4B4UZThv回避了攻击

Imperio#4B4UZThv发起攻击, <ζε>-fhepgq2n受到47点伤害

<ζε>-fhepgq2n发起攻击, 态度jX2HoULfsFU9受到35点伤害

 态度jX2HoULfsFU9被击倒了

血谣染硫决发起攻击, 末otW7sfqOze受到81点伤害

 末otW7sfqOze被击倒了

Obsession#EYNIZRX发动铁壁, Obsession#EYNIZRX防御力大幅上升

仇决clFJZCMHS使用血祭, 召唤出使魔

Imperio#4B4UZThv使用魅惑, 使魔被魅惑了

<ζε>-fhepgq2n发起攻击, #念-GP8LKM21D4JZ受到47点伤害

Imperio#4B4UZThv发起攻击, <ζε>-fhepgq2n受到36点伤害

 <ζε>-fhepgq2n被击倒了

仇决clFJZCMHS发起攻击, Obsession#EYNIZRX受到1点伤害

Obsession#EYNIZRX发起攻击, 血谣染硫决防御, 血谣染硫决受到36点伤害

 血谣染硫决被击倒了

 Obsession#EYNIZRX召唤亡灵, 血谣染硫决变成了丧尸

#念-GP8LKM21D4JZ发起攻击, Imperio#4B4UZThv回避了攻击

Imperio#4B4UZThv使用魅惑, 仇决clFJZCMHS被魅惑了

使魔发起攻击, 丧尸受到58点伤害

 使魔从魅惑中解除

Imperio#4B4UZThv发起攻击, 仇决clFJZCMHS受到48点伤害

使魔发起攻击, Imperio#4B4UZThv受到52点伤害

Obsession#EYNIZRX发起攻击, 使魔受到113点伤害, 仇决clFJZCMHS受到56点伤害

 使魔消失了

 Obsession#EYNIZRX从铁壁中解除

仇决clFJZCMHS发起攻击, 丧尸受到118点伤害

 丧尸消失了

 仇决clFJZCMHS从魅惑中解除

#念-GP8LKM21D4JZ发起攻击, Obsession#EYNIZRX受到81点伤害

 Obsession#EYNIZRX被击倒了

Imperio#4B4UZThv发起攻击, 仇决clFJZCMHS受到62点伤害

仇决clFJZCMHS发起攻击, Imperio#4B4UZThv回避了攻击

Imperio#4B4UZThv使用魅惑, #念-GP8LKM21D4JZ被魅惑了

#念-GP8LKM21D4JZ使用治愈魔法, Imperio#4B4UZThv回复体力102点

 #念-GP8LKM21D4JZ从魅惑中解除

仇决clFJZCMHS发起攻击, Imperio#4B4UZThv受到31点伤害

Imperio#4B4UZThv发起攻击, #念-GP8LKM21D4JZ受到112点伤害

 #念-GP8LKM21D4JZ被击倒了

Imperio#4B4UZThv发起攻击, 仇决clFJZCMHS受到51点伤害

仇决clFJZCMHS发起攻击, Imperio#4B4UZThv受到19点伤害

Imperio#4B4UZThv使用魅惑, 仇决clFJZCMHS被魅惑了

Imperio#4B4UZThv发起攻击, 仇决clFJZCMHS受到57点伤害

 仇决clFJZCMHS被击倒了"####;
        let (raw_input, expected_lines) = parse_embedded_fight_case(
            CASE,
            "sampled case-09 must contain a blank separator between input and trace",
            "sampled case-09 trace is empty",
        );

        let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
        let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);

        assert!(guard < 20_000, "sampled case-09 combat did not finish in expected rounds");
        assert_trace_with_name_noise_ignored("sampled case-09", &actual_lines, &expected_lines);
    }

    #[test]
    fn sampled_large_case_10_replay_should_match() {
        const CASE: &str = r####"子子油渍柚不子油不是子柚渍不不渍柚油柚子@柚子不是油渍425
#念-GP8LKM21D4JZ@柚子不是油渍425
权计WN13vmJnn@candle425
ImmutableZYsdlabOOz@RbCl425
MeltelabRC3P3Go7@RbCl425
Eaquirasd2D5HoYES@RbCl425
氯化钠8UJMGcZ@fx425
[oWmjI_$'4Z#[GK,,BX2@czr2012425
wangifc5NuJx52y1cMSaD@WDGod425
跙坥咀诅阻珇伹伹怚@涵虚425


氯化钠8UJMGcZ发起攻击, 跙坥咀诅阻珇伹伹怚受到85点伤害

MeltelabRC3P3Go7发起攻击, wangifc5NuJx52y1cMSaD回避了攻击

#念-GP8LKM21D4JZ发起攻击, 氯化钠8UJMGcZ受到57点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到56点伤害

权计WN13vmJnn发起攻击, ImmutableZYsdlabOOz受到42点伤害

[oWmjI_$'4Z#[GK,,BX2发起攻击, ImmutableZYsdlabOOz回避了攻击

wangifc5NuJx52y1cMSaD发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到70点伤害

ImmutableZYsdlabOOz发起攻击, 跙坥咀诅阻珇伹伹怚受到77点伤害

子子油渍柚不子油不是子柚渍不不渍柚油柚子使用幻术, 召唤出幻影

氯化钠8UJMGcZ发起攻击, #念-GP8LKM21D4JZ受到77点伤害

 #念-GP8LKM21D4JZ发起反击, 氯化钠8UJMGcZ回避了攻击

MeltelabRC3P3Go7发起攻击, #念-GP8LKM21D4JZ受到42点伤害

Eaquirasd2D5HoYES发起攻击, 权计WN13vmJnn受到86点伤害

权计WN13vmJnn发起攻击, ImmutableZYsdlabOOz受到63点伤害

#念-GP8LKM21D4JZ使用净化, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到97点伤害

ImmutableZYsdlabOOz使用净化, wangifc5NuJx52y1cMSaD受到31点伤害

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, 权计WN13vmJnn受到60点伤害

[oWmjI_$'4Z#[GK,,BX2发起攻击, wangifc5NuJx52y1cMSaD受到58点伤害

wangifc5NuJx52y1cMSaD发起攻击, [oWmjI_$'4Z#[GK,,BX2受到87点伤害

跙坥咀诅阻珇伹伹怚发起攻击, MeltelabRC3P3Go7受到147点伤害

Eaquirasd2D5HoYES使用地裂术

 幻影受到16点伤害

 wangifc5NuJx52y1cMSaD受到43点伤害

 权计WN13vmJnn受到10点伤害

 MeltelabRC3P3Go7受到35点伤害

氯化钠8UJMGcZ使用净化, ImmutableZYsdlabOOz受到118点伤害

MeltelabRC3P3Go7使用分身, 出现一个新的MeltelabRC3P3Go7

权计WN13vmJnn发起攻击, [oWmjI_$'4Z#[GK,,BX2受到91点伤害

ImmutableZYsdlabOOz发起攻击, 幻影受到87点伤害

#念-GP8LKM21D4JZ使用净化, Eaquirasd2D5HoYES受到86点伤害

[oWmjI_$'4Z#[GK,,BX2发起攻击, 权计WN13vmJnn回避了攻击

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, 跙坥咀诅阻珇伹伹怚受到94点伤害

wangifc5NuJx52y1cMSaD发起攻击, 跙坥咀诅阻珇伹伹怚受到113点伤害

 跙坥咀诅阻珇伹伹怚被击倒了

幻影发起攻击, #念-GP8LKM21D4JZ受到36点伤害

MeltelabRC3P3Go7潜行到Eaquirasd2D5HoYES身后

氯化钠8UJMGcZ发起攻击, [oWmjI_$'4Z#[GK,,BX2受到93点伤害

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, #念-GP8LKM21D4JZ受到140点伤害

[oWmjI_$'4Z#[GK,,BX2发起攻击, MeltelabRC3P3Go7受到75点伤害

 MeltelabRC3P3Go7的潜行被识破

 MeltelabRC3P3Go7被击倒了

MeltelabRC3P3Go7发起攻击, 权计WN13vmJnn受到27点伤害

Eaquirasd2D5HoYES发起攻击, wangifc5NuJx52y1cMSaD受到62点伤害

ImmutableZYsdlabOOz发起攻击, Eaquirasd2D5HoYES受到62点伤害

幻影发起攻击, 氯化钠8UJMGcZ受到47点伤害

权计WN13vmJnn使用诅咒, [oWmjI_$'4Z#[GK,,BX2受到38点伤害

 [oWmjI_$'4Z#[GK,,BX2被击倒了

wangifc5NuJx52y1cMSaD发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到83点伤害

#念-GP8LKM21D4JZ发起攻击, 氯化钠8UJMGcZ回避了攻击

氯化钠8UJMGcZ投毒, Eaquirasd2D5HoYES受到69点伤害, Eaquirasd2D5HoYES中毒

子子油渍柚不子油不是子柚渍不不渍柚油柚子发动铁壁, 子子油渍柚不子油不是子柚渍不不渍柚油柚子防御力大幅上升

ImmutableZYsdlabOOz发起攻击, wangifc5NuJx52y1cMSaD受到54点伤害

幻影发起攻击, ImmutableZYsdlabOOz受到45点伤害

MeltelabRC3P3Go7使用生命之轮, Eaquirasd2D5HoYES的体力值与MeltelabRC3P3Go7互换

氯化钠8UJMGcZ发起攻击, 权计WN13vmJnn受到77点伤害

 权计WN13vmJnn被击倒了

Eaquirasd2D5HoYES发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到0点伤害

 Eaquirasd2D5HoYES毒性发作, Eaquirasd2D5HoYES受到35点伤害

wangifc5NuJx52y1cMSaD发起攻击, 幻影受到67点伤害

 幻影消失了

 wangifc5NuJx52y1cMSaD吞噬了幻影, wangifc5NuJx52y1cMSaD属性上升

wangifc5NuJx52y1cMSaD使用净化, 氯化钠8UJMGcZ受到68点伤害

#念-GP8LKM21D4JZ发起攻击, Eaquirasd2D5HoYES受到45点伤害

 Eaquirasd2D5HoYES被击倒了

ImmutableZYsdlabOOz使用净化, MeltelabRC3P3Go7受到98点伤害

 MeltelabRC3P3Go7被击倒了

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, wangifc5NuJx52y1cMSaD受到50点伤害

氯化钠8UJMGcZ发起攻击, #念-GP8LKM21D4JZ受到87点伤害

 #念-GP8LKM21D4JZ被击倒了

ImmutableZYsdlabOOz发起攻击, wangifc5NuJx52y1cMSaD受到42点伤害

 wangifc5NuJx52y1cMSaD被击倒了

ImmutableZYsdlabOOz发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到0点伤害

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, 氯化钠8UJMGcZ受到63点伤害

 氯化钠8UJMGcZ被击倒了

 子子油渍柚不子油不是子柚渍不不渍柚油柚子从铁壁中解除

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, ImmutableZYsdlabOOz回避了攻击

ImmutableZYsdlabOOz发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到56点伤害

 子子油渍柚不子油不是子柚渍不不渍柚油柚子被击倒了"####;
        let (raw_input, expected_lines) = parse_embedded_fight_case(
            CASE,
            "sampled case-10 must contain a blank separator between input and trace",
            "sampled case-10 trace is empty",
        );

        let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
        let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);

        assert!(guard < 20_000, "sampled case-10 combat did not finish in expected rounds");
        assert_trace_with_context("sampled case-10", &actual_lines, &expected_lines);
    }

    // END sampled large cases (generated from test.md)

    #[test]
    fn fight_large_replay_should_match() {
        const FIGHT_CASE: &str = r###"兔蛙智仁$0a0LD4Dh@爱425
沉睡在悲伤的海洋中#056ARx3e@爱425
都江堰00217109183087@abruce425
「OS」#H1#YoRmfG4zW9@mwh_425
「OS」#c1#bFc71OCDuO35@mwh_425
「OS」#c1#E7WGTekQTugF@mwh_425
看到这个号说明你要豹了Xa2Zuiqj@柚子不是油渍425
子子油渍柚不子油不是子柚渍不不渍柚油柚子@柚子不是油渍425
#念-GP8LKM21D4JZ@柚子不是油渍425
RedOT<{f2=v}67w@流浪冒险者425
血谣染硫决@Mithril425
XIV9lcdAvS9MRKVQPL@Mithril425
Wakaba_mutsumi#pjFhEhSbjy@🥒425
Straight_into_the_lights#VpdbCrcFJV@🥒425
Tachibana_akira#BydbIMidbs@🥒425
mVf4YCPDlRm@tyakasha425
CWni0rXxe1j@tyakasha425
OxnCFhsBdeN@tyakasha425
DianmuYKFMWRPXIMCQ@nan425
xMq4EGwwCTieuZT@XJ联队425
GordonALYJDXORPTER@nan425
虚空咦aubHluOMPElA@芒萁425
十六夜咲夜zgJ6eH3TkLFp@芒萁425
回归hkbSNqezR3dr@芒萁425
"铁胆"哈拉文领主-ksbGnquBbq-@新纪元425
➐M1jC95o@新纪元425
Reku_Mochizuki#494460162188@新纪元425
权计WN13vmJnn@candle425
仇决clFJZCMHS@candle425
Sayakagh8yaICYo@candle425
ImmutableZYsdlabOOz@RbCl425
MeltelabRC3P3Go7@RbCl425
Eaquirasd2D5HoYES@RbCl425
冥河WyO8MUZPPtKH@Afterglow425
态度jX2HoULfsFU9@Afterglow425
tCtrVweRgshV@Afterglow425
锋利ⅤEGZPVQMY@TigerStar425
湖心SHVPEMAPV@TigerStar425
涵虚不等式PFVKEUPBU@TigerStar425
三田一重TxtrdTN4l8nT@fx425
氯化钠8UJMGcZ@fx425
石之自由jV3zf35@fx425
BZoPIow@酸橙425
Grey647638673419@酸橙425
空8089904649511796@酸橙425
1^GNC.%F@Hell425
'Yz|AS}@Hell425
10l-DYWg@Hell425
前尘如梦UYGMHRNX@LuoTianyi425
(S("p{GE2up',7%^UGrP@czr2012425
f{kD}WgZQgbV(&".fFMq@czr2012425
[oWmjI_$'4Z#[GK,,BX2@czr2012425
哈莫雷特m6bi9z3ZWg9y@WDGod425
wangifc5NuJx52y1cMSaD@WDGod425
wangif9nWzNbxCJ7wXi8E@WDGod425
力气30#zxQ4y6@仙蛊屋425
我力7#W2ib8D@仙蛊屋425
地气14#emOKVY@仙蛊屋425
冷霞洞.鸣湘榔狞@四象柯425
灀瑈篆狓鵃@四象柯425
针刀霜|U/T)h8J"@四象柯425
东乡幻翎#BCBNRCXFX@无惨425
封魔宣夜8uW56ll@无惨425
樱井光#CQMQFHIEV@无惨425
无惨不等式#YMGTFCOPE@星球结晶425
SDPC#AZLZJQUPN@星球结晶425
缇亚卡#WOVLHAESD@星球结晶425
咲夜bJjbFYez@Squall425
稗田阿求OQL68NN8@Squall425
愞㢯老海@昀澤425
Tik_Tok#IBxWzGZtr@Shabby_fish425
直接命中#Dfdt3d2uT@Shabby_fish425
Imperio#4B4UZThv@Shabby_fish425
PHKBUUPNHGMI@云淡风轻425
PraykxtsMobhMzey@橙红耀阳425
运松翁nkJspy1Oh54A@橙红耀阳425
虚空辉光舰FpNQjf4keMaT0TB@橙红耀阳425
七七#EUEMIGPI@暗黑突击425
可可萝#EZBAOSOOV@公主连结425
神谷紫苑#EUKSOXAA@暗黑突击425
桃v66wy7tgu27xp@asyncTales425
Obsession#EYNIZRX@Unbound425
Hypochondriac#TtwN3jZ@Unbound425
Reality#ke10TrY@Unbound425
末otW7sfqOze@807139425
泠珞[VmRt[ntb@807139425
渊HGbwigVwI@807139425
<ζο>-2ny1o5sk@ReturnVoid425
<ζε>-fhepgq2n@ReturnVoid425
跙坥咀诅阻珇伹伹怚@涵虚425
seed:第十八届武术大赛抽签:425-0@!


湖心SHVPEMAPV发起攻击, "铁胆"哈拉文领主-ksbGnquBbq-受到59点伤害

Imperio#4B4UZThv发起攻击, <ζο>-2ny1o5sk受到85点伤害

ImmutableZYsdlabOOz发起攻击, 看到这个号说明你要豹了Xa2Zuiqj受到29点伤害

xMq4EGwwCTieuZT发起攻击, 封魔宣夜8uW56ll受到56点伤害

MeltelabRC3P3Go7发起攻击, 末otW7sfqOze回避了攻击

Reality#ke10TrY发起攻击, 泠珞[VmRt[ntb受到104点伤害

f{kD}WgZQgbV(&".fFMq发起攻击, DianmuYKFMWRPXIMCQ受到84点伤害

空8089904649511796发起攻击, 前尘如梦UYGMHRNX受到83点伤害

(S("p{GE2up',7%^UGrP发起攻击, Tik_Tok#IBxWzGZtr受到57点伤害

Grey647638673419发起攻击, OxnCFhsBdeN受到66点伤害

灀瑈篆狓鵃发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到65点伤害

我力7#W2ib8D投毒, 1^GNC.%F受到10点伤害, 1^GNC.%F中毒

Sayakagh8yaICYo发起攻击, Grey647638673419受到72点伤害

GordonALYJDXORPTER发起攻击, Reku_Mochizuki#494460162188受到49点伤害

哈莫雷特m6bi9z3ZWg9y使用狂暴术, BZoPIow受到95点伤害, BZoPIow进入狂暴状态

Straight_into_the_lights#VpdbCrcFJV发起攻击, 「OS」#c1#bFc71OCDuO35受到75点伤害

三田一重TxtrdTN4l8nT发起攻击, 10l-DYWg受到86点伤害

神谷紫苑#EUKSOXAA发起攻击, 锋利ⅤEGZPVQMY受到24点伤害

'Yz|AS}发起攻击, "铁胆"哈拉文领主-ksbGnquBbq-受到91点伤害

七七#EUEMIGPI发起攻击, Eaquirasd2D5HoYES受到55点伤害

樱井光#CQMQFHIEV发起攻击, PHKBUUPNHGMI受到70点伤害

#念-GP8LKM21D4JZ发起攻击, DianmuYKFMWRPXIMCQ受到62点伤害

Tachibana_akira#BydbIMidbs发起攻击, 无惨不等式#YMGTFCOPE受到22点伤害

Hypochondriac#TtwN3jZ发起攻击, <ζο>-2ny1o5sk受到46点伤害

DianmuYKFMWRPXIMCQ发起攻击, tCtrVweRgshV受到87点伤害

血谣染硫决使用幻术, 召唤出幻影

wangif9nWzNbxCJ7wXi8E发起攻击, 末otW7sfqOze受到62点伤害

泠珞[VmRt[ntb发起攻击, 愞㢯老海受到53点伤害

tCtrVweRgshV发起攻击, Reality#ke10TrY受到99点伤害

「OS」#c1#bFc71OCDuO35发动会心一击, Obsession#EYNIZRX回避了攻击

氯化钠8UJMGcZ投毒, Wakaba_mutsumi#pjFhEhSbjy回避了攻击

末otW7sfqOze发起攻击, Reality#ke10TrY受到109点伤害

冷霞洞.鸣湘榔狞发起攻击, 看到这个号说明你要豹了Xa2Zuiqj受到21点伤害

十六夜咲夜zgJ6eH3TkLFp发起攻击, 冷霞洞.鸣湘榔狞受到42点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 七七#EUEMIGPI受到68点伤害

可可萝#EZBAOSOOV发起攻击, wangif9nWzNbxCJ7wXi8E受到134点伤害

涵虚不等式PFVKEUPBU发起攻击, 樱井光#CQMQFHIEV受到50点伤害

稗田阿求OQL68NN8发起攻击, 泠珞[VmRt[ntb防御, 泠珞[VmRt[ntb受到25点伤害

权计WN13vmJnn使用诅咒, 三田一重TxtrdTN4l8nT受到61点伤害, 三田一重TxtrdTN4l8nT被诅咒了

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, Tik_Tok#IBxWzGZtr受到46点伤害

「OS」#c1#E7WGTekQTugF发起攻击, 东乡幻翎#BCBNRCXFX受到29点伤害

封魔宣夜8uW56ll发起攻击, 十六夜咲夜zgJ6eH3TkLFp受到108点伤害

BZoPIow发起狂暴攻击, 兔蛙智仁$0a0LD4Dh受到115点伤害

 BZoPIow从狂暴中解除

仇决clFJZCMHS发起攻击, 石之自由jV3zf35受到69点伤害

「OS」#H1#YoRmfG4zW9发起攻击, MeltelabRC3P3Go7受到43点伤害

针刀霜|U/T)h8J"发起攻击, 七七#EUEMIGPI回避了攻击

咲夜bJjbFYez发起攻击, <ζο>-2ny1o5sk回避了攻击

虚空咦aubHluOMPElA发起攻击, 回归hkbSNqezR3dr受到60点伤害

地气14#emOKVY发起攻击, 稗田阿求OQL68NN8受到19点伤害

桃v66wy7tgu27xp发起攻击, 'Yz|AS}回避了攻击

Reku_Mochizuki#494460162188发起攻击, Straight_into_the_lights#VpdbCrcFJV受到33点伤害

1^GNC.%F发起攻击, 虚空咦aubHluOMPElA受到77点伤害

 1^GNC.%F毒性发作, 1^GNC.%F受到28点伤害

PHKBUUPNHGMI发起攻击, 缇亚卡#WOVLHAESD受到70点伤害

力气30#zxQ4y6发起攻击, 湖心SHVPEMAPV受到39点伤害

态度jX2HoULfsFU9发起攻击, 冥河WyO8MUZPPtKH受到85点伤害

SDPC#AZLZJQUPN开始聚气, SDPC#AZLZJQUPN攻击力上升

PraykxtsMobhMzey发起攻击, <ζε>-fhepgq2n受到77点伤害

湖心SHVPEMAPV使用生命之轮, 锋利ⅤEGZPVQMY的体力值与湖心SHVPEMAPV互换

"铁胆"哈拉文领主-ksbGnquBbq-发起攻击, 可可萝#EZBAOSOOV回避了攻击

<ζε>-fhepgq2n发起攻击, 力气30#zxQ4y6受到122点伤害

看到这个号说明你要豹了Xa2Zuiqj发起攻击, 可可萝#EZBAOSOOV受到111点伤害

冥河WyO8MUZPPtKH发起攻击, <ζο>-2ny1o5sk受到33点伤害

RedOT<{f2=v}67w发起攻击, 回归hkbSNqezR3dr受到68点伤害

[oWmjI_$'4Z#[GK,,BX2发起攻击, wangifc5NuJx52y1cMSaD受到60点伤害

Wakaba_mutsumi#pjFhEhSbjy发起攻击, 稗田阿求OQL68NN8受到77点伤害

沉睡在悲伤的海洋中#056ARx3e发起攻击, Eaquirasd2D5HoYES受到35点伤害

ImmutableZYsdlabOOz发起攻击, <ζο>-2ny1o5sk受到78点伤害

回归hkbSNqezR3dr发起攻击, ImmutableZYsdlabOOz受到83点伤害

无惨不等式#YMGTFCOPE使用冰冻术, 湖心SHVPEMAPV回避了攻击

➐M1jC95o发起攻击, RedOT<{f2=v}67w受到53点伤害

都江堰00217109183087发起攻击, 神谷紫苑#EUKSOXAA受到53点伤害

f{kD}WgZQgbV(&".fFMq发起攻击, PraykxtsMobhMzey回避了攻击

(S("p{GE2up',7%^UGrP发起攻击, 跙坥咀诅阻珇伹伹怚受到54点伤害

前尘如梦UYGMHRNX发起攻击, Straight_into_the_lights#VpdbCrcFJV受到83点伤害

兔蛙智仁$0a0LD4Dh发起攻击, wangifc5NuJx52y1cMSaD受到59点伤害

渊HGbwigVwI发起攻击, (S("p{GE2up',7%^UGrP受到74点伤害

XIV9lcdAvS9MRKVQPL使用净化, Grey647638673419受到43点伤害

虚空辉光舰FpNQjf4keMaT0TB发起攻击, "铁胆"哈拉文领主-ksbGnquBbq-受到87点伤害

运松翁nkJspy1Oh54A使用雷击术

 SDPC#AZLZJQUPN受到12点伤害

 SDPC#AZLZJQUPN受到12点伤害

 SDPC#AZLZJQUPN受到21点伤害

 SDPC#AZLZJQUPN受到21点伤害

mVf4YCPDlRm发起攻击, 冷霞洞.鸣湘榔狞受到54点伤害

OxnCFhsBdeN发起攻击, [oWmjI_$'4Z#[GK,,BX2受到101点伤害

Tachibana_akira#BydbIMidbs发起攻击, Straight_into_the_lights#VpdbCrcFJV受到54点伤害

Obsession#EYNIZRX发起攻击, ImmutableZYsdlabOOz受到36点伤害

CWni0rXxe1j发起攻击, 咲夜bJjbFYez受到79点伤害

缇亚卡#WOVLHAESD发起攻击, 涵虚不等式PFVKEUPBU受到83点伤害

'Yz|AS}发起攻击, 缇亚卡#WOVLHAESD受到84点伤害

血谣染硫决发起攻击, f{kD}WgZQgbV(&".fFMq回避了攻击

xMq4EGwwCTieuZT发起攻击, f{kD}WgZQgbV(&".fFMq受到116点伤害

Sayakagh8yaICYo开始聚气, Sayakagh8yaICYo攻击力上升

哈莫雷特m6bi9z3ZWg9y使用狂暴术, 兔蛙智仁$0a0LD4Dh受到150点伤害, 兔蛙智仁$0a0LD4Dh进入狂暴状态

愞㢯老海发动铁壁, 愞㢯老海防御力大幅上升

神谷紫苑#EUKSOXAA发起攻击, 七七#EUEMIGPI受到67点伤害

10l-DYWg发起攻击, 都江堰00217109183087受到76点伤害

wangifc5NuJx52y1cMSaD发起攻击, CWni0rXxe1j受到40点伤害

Tik_Tok#IBxWzGZtr发起攻击, Eaquirasd2D5HoYES受到77点伤害

MeltelabRC3P3Go7发起攻击, wangif9nWzNbxCJ7wXi8E受到47点伤害

➐M1jC95o发起攻击, 沉睡在悲伤的海洋中#056ARx3e受到73点伤害

Straight_into_the_lights#VpdbCrcFJV发起攻击, 渊HGbwigVwI受到67点伤害

三田一重TxtrdTN4l8nT发起攻击, 1^GNC.%F回避了攻击

氯化钠8UJMGcZ发起攻击, #念-GP8LKM21D4JZ受到69点伤害

仇决clFJZCMHS发起攻击, 封魔宣夜8uW56ll受到27点伤害

跙坥咀诅阻珇伹伹怚使用生命之轮, 仇决clFJZCMHS的体力值与跙坥咀诅阻珇伹伹怚互换

Grey647638673419发起攻击, Obsession#EYNIZRX受到103点伤害

石之自由jV3zf35发起攻击, 1^GNC.%F受到94点伤害

七七#EUEMIGPI发起攻击, 「OS」#c1#E7WGTekQTugF受到58点伤害

可可萝#EZBAOSOOV发起攻击, xMq4EGwwCTieuZT受到75点伤害

桃v66wy7tgu27xp发起攻击, ➐M1jC95o受到42点伤害

涵虚不等式PFVKEUPBU发起攻击, 湖心SHVPEMAPV受到114点伤害

樱井光#CQMQFHIEV发起攻击, 咲夜bJjbFYez受到121点伤害

#念-GP8LKM21D4JZ发起攻击, 无惨不等式#YMGTFCOPE受到109点伤害

Eaquirasd2D5HoYES发起攻击, mVf4YCPDlRm受到81点伤害

Hypochondriac#TtwN3jZ发起攻击, 虚空辉光舰FpNQjf4keMaT0TB受到21点伤害

直接命中#Dfdt3d2uT发起攻击, 泠珞[VmRt[ntb回避了攻击

我力7#W2ib8D发动会心一击, 'Yz|AS}受到126点伤害

1^GNC.%F发起攻击, 血谣染硫决受到70点伤害

 1^GNC.%F毒性发作, 1^GNC.%F受到23点伤害

东乡幻翎#BCBNRCXFX发起攻击, 仇决clFJZCMHS受到57点伤害

权计WN13vmJnn发起攻击, 无惨不等式#YMGTFCOPE回避了攻击

末otW7sfqOze发起攻击, Straight_into_the_lights#VpdbCrcFJV受到66点伤害

f{kD}WgZQgbV(&".fFMq发起攻击, OxnCFhsBdeN受到48点伤害

锋利ⅤEGZPVQMY发起攻击, BZoPIow受到85点伤害

十六夜咲夜zgJ6eH3TkLFp发起攻击, 氯化钠8UJMGcZ受到80点伤害

Imperio#4B4UZThv使用魅惑, 子子油渍柚不子油不是子柚渍不不渍柚油柚子被魅惑了

冥河WyO8MUZPPtKH使用魅惑, 「OS」#H1#YoRmfG4zW9被魅惑了

咲夜bJjbFYez使用净化, 运松翁nkJspy1Oh54A受到57点伤害

虚空咦aubHluOMPElA发起攻击, 涵虚不等式PFVKEUPBU受到70点伤害

<ζο>-2ny1o5sk发起攻击, 权计WN13vmJnn受到41点伤害

灀瑈篆狓鵃发起攻击, Reku_Mochizuki#494460162188受到67点伤害

[oWmjI_$'4Z#[GK,,BX2发动会心一击, 神谷紫苑#EUKSOXAA受到85点伤害

泠珞[VmRt[ntb使用瘟疫, 都江堰00217109183087体力减少64%

稗田阿求OQL68NN8发起攻击, 看到这个号说明你要豹了Xa2Zuiqj受到88点伤害

GordonALYJDXORPTER发起攻击, 东乡幻翎#BCBNRCXFX受到91点伤害

tCtrVweRgshV发起攻击, <ζο>-2ny1o5sk受到45点伤害

 <ζο>-2ny1o5sk被击倒了

湖心SHVPEMAPV发起攻击, 愞㢯老海受到1点伤害

Reality#ke10TrY使用血祭, 召唤出使魔

空8089904649511796使用血祭, 召唤出使魔

冷霞洞.鸣湘榔狞发起攻击, XIV9lcdAvS9MRKVQPL受到24点伤害

兔蛙智仁$0a0LD4Dh发起攻击, wangif9nWzNbxCJ7wXi8E防御, wangif9nWzNbxCJ7wXi8E受到55点伤害

RedOT<{f2=v}67w发起攻击, Sayakagh8yaICYo受到66点伤害

DianmuYKFMWRPXIMCQ发起攻击, 石之自由jV3zf35受到69点伤害

xMq4EGwwCTieuZT发起攻击, 'Yz|AS}受到31点伤害

态度jX2HoULfsFU9发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到77点伤害

SDPC#AZLZJQUPN发起攻击, 涵虚不等式PFVKEUPBU受到157点伤害

 涵虚不等式PFVKEUPBU被击倒了

封魔宣夜8uW56ll发起攻击, 东乡幻翎#BCBNRCXFX回避了攻击

前尘如梦UYGMHRNX发起攻击, 渊HGbwigVwI受到74点伤害

'Yz|AS}发起攻击, 十六夜咲夜zgJ6eH3TkLFp受到46点伤害

10l-DYWg发起攻击, 力气30#zxQ4y6受到146点伤害

地气14#emOKVY发起攻击, 无惨不等式#YMGTFCOPE受到32点伤害

运松翁nkJspy1Oh54A发起攻击, 泠珞[VmRt[ntb受到68点伤害

Wakaba_mutsumi#pjFhEhSbjy发起攻击, 使魔受到86点伤害, Reality#ke10TrY受到43点伤害

 使魔消失了

OxnCFhsBdeN发起攻击, 「OS」#c1#E7WGTekQTugF受到86点伤害

ImmutableZYsdlabOOz发起攻击, 力气30#zxQ4y6受到65点伤害

 力气30#zxQ4y6被击倒了

CWni0rXxe1j发起攻击, 愞㢯老海受到1点伤害

幻影发起攻击, ➐M1jC95o受到67点伤害

MeltelabRC3P3Go7发起攻击, f{kD}WgZQgbV(&".fFMq受到90点伤害

Sayakagh8yaICYo使用幻术, 召唤出幻影

PHKBUUPNHGMI使用生命之轮, #念-GP8LKM21D4JZ的体力值与PHKBUUPNHGMI互换

无惨不等式#YMGTFCOPE发起攻击, 针刀霜|U/T)h8J"受到50点伤害

「OS」#c1#bFc71OCDuO35发起攻击, Straight_into_the_lights#VpdbCrcFJV受到109点伤害

 Straight_into_the_lights#VpdbCrcFJV被击倒了

PraykxtsMobhMzey发起攻击, <ζε>-fhepgq2n受到42点伤害

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, 使魔受到45点伤害, 空8089904649511796受到22点伤害

 子子油渍柚不子油不是子柚渍不不渍柚油柚子从魅惑中解除

(S("p{GE2up',7%^UGrP发起攻击, PraykxtsMobhMzey受到34点伤害

神谷紫苑#EUKSOXAA发起攻击, 冥河WyO8MUZPPtKH受到108点伤害

针刀霜|U/T)h8J"发起攻击, 权计WN13vmJnn受到42点伤害

Reku_Mochizuki#494460162188使用雷击术

 Sayakagh8yaICYo受到36点伤害

 Sayakagh8yaICYo受到12点伤害

 Sayakagh8yaICYo受到28点伤害

 Sayakagh8yaICYo受到22点伤害

 Sayakagh8yaICYo受到16点伤害

wangifc5NuJx52y1cMSaD使用分身, 出现一个新的wangifc5NuJx52y1cMSaD

沉睡在悲伤的海洋中#056ARx3e发起攻击, 仇决clFJZCMHS受到98点伤害

血谣染硫决发起攻击, 1^GNC.%F受到56点伤害

哈莫雷特m6bi9z3ZWg9y发起攻击, 跙坥咀诅阻珇伹伹怚受到45点伤害

都江堰00217109183087发起攻击, PraykxtsMobhMzey受到65点伤害

「OS」#c1#E7WGTekQTugF发起攻击, 10l-DYWg受到73点伤害

渊HGbwigVwI发起攻击, ImmutableZYsdlabOOz受到54点伤害

Grey647638673419发起攻击, [oWmjI_$'4Z#[GK,,BX2受到50点伤害

"铁胆"哈拉文领主-ksbGnquBbq-发动会心一击, <ζε>-fhepgq2n受到74点伤害

XIV9lcdAvS9MRKVQPL发起攻击, 可可萝#EZBAOSOOV受到66点伤害

樱井光#CQMQFHIEV发起攻击, 空8089904649511796受到58点伤害

mVf4YCPDlRm发起攻击, 血谣染硫决受到135点伤害

回归hkbSNqezR3dr发起攻击, SDPC#AZLZJQUPN受到72点伤害

愞㢯老海发起攻击, 跙坥咀诅阻珇伹伹怚回避了攻击

氯化钠8UJMGcZ发起攻击, PraykxtsMobhMzey受到100点伤害

空8089904649511796使用魅惑, mVf4YCPDlRm被魅惑了

BZoPIow使用冰冻术, DianmuYKFMWRPXIMCQ回避了攻击

「OS」#H1#YoRmfG4zW9发起攻击, 兔蛙智仁$0a0LD4Dh回避了攻击

 「OS」#H1#YoRmfG4zW9从魅惑中解除

看到这个号说明你要豹了Xa2Zuiqj发起攻击, 幻影回避了攻击

咲夜bJjbFYez使用治愈魔法, 咲夜bJjbFYez回复体力80点

Tik_Tok#IBxWzGZtr使用诅咒, <ζε>-fhepgq2n受到48点伤害, <ζε>-fhepgq2n被诅咒了

1^GNC.%F发起攻击, wangifc5NuJx52y1cMSaD受到59点伤害

 1^GNC.%F毒性发作, 1^GNC.%F受到19点伤害

wangif9nWzNbxCJ7wXi8E发起攻击, 1^GNC.%F受到39点伤害

稗田阿求OQL68NN8发起攻击, 针刀霜|U/T)h8J"受到61点伤害

➐M1jC95o发起攻击, 东乡幻翎#BCBNRCXFX受到133点伤害

权计WN13vmJnn发起攻击, 幻影受到92点伤害

f{kD}WgZQgbV(&".fFMq发动会心一击, 跙坥咀诅阻珇伹伹怚受到86点伤害

仇决clFJZCMHS发起攻击, "铁胆"哈拉文领主-ksbGnquBbq-受到28点伤害

石之自由jV3zf35使用冰冻术, 运松翁nkJspy1Oh54A受到48点伤害, 运松翁nkJspy1Oh54A被冰冻了

冥河WyO8MUZPPtKH发起攻击, 权计WN13vmJnn受到20点伤害

可可萝#EZBAOSOOV发起攻击, 兔蛙智仁$0a0LD4Dh受到131点伤害

 兔蛙智仁$0a0LD4Dh被击倒了

Tachibana_akira#BydbIMidbs发起攻击, 渊HGbwigVwI受到25点伤害

直接命中#Dfdt3d2uT发起攻击, 灀瑈篆狓鵃受到48点伤害

我力7#W2ib8D发起攻击, 石之自由jV3zf35受到42点伤害

xMq4EGwwCTieuZT发起攻击, 「OS」#c1#bFc71OCDuO35受到82点伤害

东乡幻翎#BCBNRCXFX发起攻击, BZoPIow受到137点伤害

湖心SHVPEMAPV发起攻击, 直接命中#Dfdt3d2uT受到59点伤害

冷霞洞.鸣湘榔狞发起攻击, 针刀霜|U/T)h8J"受到56点伤害

十六夜咲夜zgJ6eH3TkLFp发起攻击, Wakaba_mutsumi#pjFhEhSbjy受到63点伤害

跙坥咀诅阻珇伹伹怚发起攻击, ➐M1jC95o受到125点伤害

'Yz|AS}发起攻击, 态度jX2HoULfsFU9受到100点伤害

<ζε>-fhepgq2n发起攻击, 诅咒使伤害加倍, 三田一重TxtrdTN4l8nT受到160点伤害

虚空辉光舰FpNQjf4keMaT0TB开始蓄力

#念-GP8LKM21D4JZ发起攻击, 桃v66wy7tgu27xp受到46点伤害

Eaquirasd2D5HoYES发起攻击, 沉睡在悲伤的海洋中#056ARx3e受到71点伤害

Obsession#EYNIZRX发起攻击, BZoPIow受到33点伤害

 BZoPIow被击倒了

wangifc5NuJx52y1cMSaD发起攻击, 虚空辉光舰FpNQjf4keMaT0TB受到82点伤害

GordonALYJDXORPTER发起攻击, 灀瑈篆狓鵃受到17点伤害

tCtrVweRgshV发起攻击, 愞㢯老海受到1点伤害

三田一重TxtrdTN4l8nT发起攻击, Wakaba_mutsumi#pjFhEhSbjy回避了攻击

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, (S("p{GE2up',7%^UGrP受到58点伤害

Reality#ke10TrY发起攻击, Imperio#4B4UZThv受到24点伤害

末otW7sfqOze发起攻击, Imperio#4B4UZThv受到58点伤害

七七#EUEMIGPI发起攻击, 氯化钠8UJMGcZ受到82点伤害

桃v66wy7tgu27xp发起攻击, Imperio#4B4UZThv受到104点伤害

灀瑈篆狓鵃发起攻击, 虚空辉光舰FpNQjf4keMaT0TB受到63点伤害

Wakaba_mutsumi#pjFhEhSbjy开始聚气, Wakaba_mutsumi#pjFhEhSbjy攻击力上升

CWni0rXxe1j发起攻击, 地气14#emOKVY受到59点伤害

 地气14#emOKVY发起反击, CWni0rXxe1j受到32点伤害

幻影发起攻击, Tachibana_akira#BydbIMidbs受到42点伤害

使魔发起攻击, 运松翁nkJspy1Oh54A受到73点伤害

缇亚卡#WOVLHAESD发起攻击, Tachibana_akira#BydbIMidbs受到135点伤害

MeltelabRC3P3Go7发起攻击, tCtrVweRgshV受到38点伤害

 tCtrVweRgshV发起反击, MeltelabRC3P3Go7受到64点伤害

泠珞[VmRt[ntb发起攻击, 灀瑈篆狓鵃回避了攻击

无惨不等式#YMGTFCOPE使用冰冻术, #念-GP8LKM21D4JZ受到59点伤害, #念-GP8LKM21D4JZ被冰冻了

封魔宣夜8uW56ll发起攻击, Hypochondriac#TtwN3jZ受到77点伤害

神谷紫苑#EUKSOXAA发起攻击, RedOT<{f2=v}67w受到84点伤害

渊HGbwigVwI使用幻术, 召唤出幻影

"铁胆"哈拉文领主-ksbGnquBbq-使用瘟疫, tCtrVweRgshV体力减少53%

RedOT<{f2=v}67w使用净化, 诅咒使伤害加倍, 三田一重TxtrdTN4l8nT受到116点伤害

 三田一重TxtrdTN4l8nT被击倒了

地气14#emOKVY发起攻击, 前尘如梦UYGMHRNX受到43点伤害

Reku_Mochizuki#494460162188使用雷击术

 回归hkbSNqezR3dr受到35点伤害

 回归hkbSNqezR3dr受到37点伤害

 回归hkbSNqezR3dr受到17点伤害

 回归hkbSNqezR3dr受到16点伤害

OxnCFhsBdeN发起攻击, 'Yz|AS}受到70点伤害

ImmutableZYsdlabOOz使用净化, PraykxtsMobhMzey受到6点伤害

Hypochondriac#TtwN3jZ发起攻击, 七七#EUEMIGPI受到68点伤害

PHKBUUPNHGMI发起攻击, 针刀霜|U/T)h8J"受到56点伤害

态度jX2HoULfsFU9发起攻击, 稗田阿求OQL68NN8受到63点伤害

「OS」#c1#bFc71OCDuO35发起攻击, 针刀霜|U/T)h8J"受到68点伤害

SDPC#AZLZJQUPN发起攻击, 地气14#emOKVY受到215点伤害

「OS」#c1#E7WGTekQTugF使用冰冻术, 都江堰00217109183087受到56点伤害, 都江堰00217109183087被冰冻了

跙坥咀诅阻珇伹伹怚发起攻击, Wakaba_mutsumi#pjFhEhSbjy受到54点伤害

虚空咦aubHluOMPElA发起攻击, 前尘如梦UYGMHRNX受到54点伤害

[oWmjI_$'4Z#[GK,,BX2发动会心一击, 针刀霜|U/T)h8J"受到134点伤害

 针刀霜|U/T)h8J"被击倒了

wangifc5NuJx52y1cMSaD发起攻击, 地气14#emOKVY受到30点伤害

 地气14#emOKVY被击倒了

樱井光#CQMQFHIEV发起攻击, 神谷紫苑#EUKSOXAA回避了攻击

mVf4YCPDlRm发起攻击, 愞㢯老海受到1点伤害

 mVf4YCPDlRm从魅惑中解除

血谣染硫决发起攻击, mVf4YCPDlRm受到117点伤害

Sayakagh8yaICYo发起攻击, RedOT<{f2=v}67w受到130点伤害

 RedOT<{f2=v}67w被击倒了

哈莫雷特m6bi9z3ZWg9y使用狂暴术, 回归hkbSNqezR3dr受到64点伤害, 回归hkbSNqezR3dr进入狂暴状态

PraykxtsMobhMzey发起攻击, Reality#ke10TrY受到67点伤害

 Reality#ke10TrY被击倒了

氯化钠8UJMGcZ发起攻击, 「OS」#H1#YoRmfG4zW9受到88点伤害

(S("p{GE2up',7%^UGrP发起攻击, XIV9lcdAvS9MRKVQPL回避了攻击

前尘如梦UYGMHRNX发起攻击, XIV9lcdAvS9MRKVQPL受到92点伤害

锋利ⅤEGZPVQMY发起攻击, mVf4YCPDlRm受到115点伤害

 mVf4YCPDlRm被击倒了

Grey647638673419发起攻击, f{kD}WgZQgbV(&".fFMq受到76点伤害

 f{kD}WgZQgbV(&".fFMq被击倒了

10l-DYWg发起攻击, #念-GP8LKM21D4JZ受到64点伤害

冥河WyO8MUZPPtKH开始蓄力

稗田阿求OQL68NN8发起攻击, Eaquirasd2D5HoYES受到72点伤害

➐M1jC95o发起攻击, 哈莫雷特m6bi9z3ZWg9y回避了攻击

空8089904649511796发起攻击, 仇决clFJZCMHS受到64点伤害

Imperio#4B4UZThv使用生命之轮, CWni0rXxe1j的体力值与Imperio#4B4UZThv互换

<ζε>-fhepgq2n发起攻击, 1^GNC.%F受到146点伤害

 1^GNC.%F被击倒了

看到这个号说明你要豹了Xa2Zuiqj发起攻击, 空8089904649511796受到23点伤害

咲夜bJjbFYez发起攻击, wangifc5NuJx52y1cMSaD受到85点伤害

回归hkbSNqezR3dr发起攻击, 幻影受到65点伤害

tCtrVweRgshV使用减速术, 灀瑈篆狓鵃进入迟缓状态

权计WN13vmJnn发起攻击, 哈莫雷特m6bi9z3ZWg9y受到101点伤害

子子油渍柚不子油不是子柚渍不不渍柚油柚子发动铁壁, 子子油渍柚不子油不是子柚渍不不渍柚油柚子防御力大幅上升

仇决clFJZCMHS开始蓄力

石之自由jV3zf35开始蓄力

XIV9lcdAvS9MRKVQPL投毒, GordonALYJDXORPTER受到121点伤害, GordonALYJDXORPTER中毒

Wakaba_mutsumi#pjFhEhSbjy使用诅咒, 前尘如梦UYGMHRNX回避了攻击

Eaquirasd2D5HoYES发起攻击, 封魔宣夜8uW56ll受到64点伤害

Tachibana_akira#BydbIMidbs发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到0点伤害

幻影发起攻击, Reku_Mochizuki#494460162188回避了攻击

wangifc5NuJx52y1cMSaD发起攻击, 末otW7sfqOze受到86点伤害

缇亚卡#WOVLHAESD发起攻击, 诅咒使伤害加倍, <ζε>-fhepgq2n受到66点伤害

 <ζε>-fhepgq2n被击倒了

wangif9nWzNbxCJ7wXi8E发起攻击, 幻影受到63点伤害

东乡幻翎#BCBNRCXFX发起攻击, 末otW7sfqOze受到30点伤害

PHKBUUPNHGMI发起攻击, xMq4EGwwCTieuZT受到63点伤害

态度jX2HoULfsFU9使用魅惑, Tachibana_akira#BydbIMidbs被魅惑了

愞㢯老海发起吸血攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子回避了攻击

 愞㢯老海从铁壁中解除

SDPC#AZLZJQUPN发起攻击, 幻影受到90点伤害

 幻影消失了

湖心SHVPEMAPV使用地裂术

 封魔宣夜8uW56ll回避了攻击

 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到0点伤害

 态度jX2HoULfsFU9回避了攻击

 看到这个号说明你要豹了Xa2Zuiqj回避了攻击

 我力7#W2ib8D受到33点伤害

「OS」#c1#E7WGTekQTugF发起攻击, "铁胆"哈拉文领主-ksbGnquBbq-受到121点伤害

 "铁胆"哈拉文领主-ksbGnquBbq-被击倒了

锋利ⅤEGZPVQMY发起攻击, Hypochondriac#TtwN3jZ受到93点伤害

'Yz|AS}发起攻击, Hypochondriac#TtwN3jZ受到43点伤害

 Hypochondriac#TtwN3jZ发起反击, 'Yz|AS}受到51点伤害

 'Yz|AS}被击倒了

运松翁nkJspy1Oh54A从冰冻中解除

ImmutableZYsdlabOOz使用净化, DianmuYKFMWRPXIMCQ受到20点伤害

xMq4EGwwCTieuZT发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到0点伤害

封魔宣夜8uW56ll发起攻击, 泠珞[VmRt[ntb受到53点伤害

冷霞洞.鸣湘榔狞发起攻击, GordonALYJDXORPTER受到98点伤害

「OS」#H1#YoRmfG4zW9发起攻击, 东乡幻翎#BCBNRCXFX受到68点伤害

 东乡幻翎#BCBNRCXFX被击倒了

冥河WyO8MUZPPtKH发起攻击, Tik_Tok#IBxWzGZtr受到108点伤害

可可萝#EZBAOSOOV发起攻击, 看到这个号说明你要豹了Xa2Zuiqj受到67点伤害

运松翁nkJspy1Oh54A发起攻击, wangif9nWzNbxCJ7wXi8E受到67点伤害

Reku_Mochizuki#494460162188发起攻击, 桃v66wy7tgu27xp回避了攻击

沉睡在悲伤的海洋中#056ARx3e开始聚气, 沉睡在悲伤的海洋中#056ARx3e攻击力上升

Tik_Tok#IBxWzGZtr发起攻击, 石之自由jV3zf35回避了攻击

我力7#W2ib8D发起攻击, 10l-DYWg受到64点伤害

CWni0rXxe1j发起攻击, [oWmjI_$'4Z#[GK,,BX2受到84点伤害

使魔发起攻击, SDPC#AZLZJQUPN受到42点伤害

GordonALYJDXORPTER发起攻击, 湖心SHVPEMAPV受到74点伤害

 GordonALYJDXORPTER毒性发作, GordonALYJDXORPTER受到46点伤害

「OS」#c1#bFc71OCDuO35发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到0点伤害

SDPC#AZLZJQUPN发起攻击, 咲夜bJjbFYez受到255点伤害

 咲夜bJjbFYez被击倒了

氯化钠8UJMGcZ发起攻击, 灀瑈篆狓鵃受到86点伤害

神谷紫苑#EUKSOXAA发起攻击, DianmuYKFMWRPXIMCQ受到78点伤害

十六夜咲夜zgJ6eH3TkLFp发起攻击, 我力7#W2ib8D受到51点伤害

OxnCFhsBdeN投毒, 樱井光#CQMQFHIEV受到65点伤害, 樱井光#CQMQFHIEV中毒

血谣染硫决发起攻击, Tachibana_akira#BydbIMidbs受到118点伤害

 Tachibana_akira#BydbIMidbs被击倒了

Sayakagh8yaICYo发起攻击, 樱井光#CQMQFHIEV回避了攻击

无惨不等式#YMGTFCOPE发起攻击, 虚空咦aubHluOMPElA受到51点伤害

➐M1jC95o发起攻击, 「OS」#c1#E7WGTekQTugF受到35点伤害

空8089904649511796发起攻击, 幻影受到96点伤害

 幻影消失了

(S("p{GE2up',7%^UGrP发动会心一击, Wakaba_mutsumi#pjFhEhSbjy回避了攻击

虚空咦aubHluOMPElA发动会心一击, 冷霞洞.鸣湘榔狞受到93点伤害

虚空辉光舰FpNQjf4keMaT0TB发起攻击, 跙坥咀诅阻珇伹伹怚回避了攻击

桃v66wy7tgu27xp发起攻击, 虚空辉光舰FpNQjf4keMaT0TB受到69点伤害

[oWmjI_$'4Z#[GK,,BX2发起攻击, 渊HGbwigVwI受到36点伤害

wangifc5NuJx52y1cMSaD发起攻击, 「OS」#c1#bFc71OCDuO35受到106点伤害

 「OS」#c1#bFc71OCDuO35被击倒了

#念-GP8LKM21D4JZ从冰冻中解除

Hypochondriac#TtwN3jZ使用地裂术

 PraykxtsMobhMzey受到15点伤害

 缇亚卡#WOVLHAESD受到18点伤害

 DianmuYKFMWRPXIMCQ受到17点伤害

 哈莫雷特m6bi9z3ZWg9y受到28点伤害

Obsession#EYNIZRX发动会心一击, 十六夜咲夜zgJ6eH3TkLFp受到132点伤害

 十六夜咲夜zgJ6eH3TkLFp被击倒了

MeltelabRC3P3Go7潜行到仇决clFJZCMHS身后

泠珞[VmRt[ntb发起攻击, 七七#EUEMIGPI受到48点伤害

哈莫雷特m6bi9z3ZWg9y发起攻击, Tik_Tok#IBxWzGZtr受到29点伤害

10l-DYWg发起攻击, Grey647638673419受到87点伤害

灀瑈篆狓鵃使用地裂术

 PHKBUUPNHGMI受到19点伤害

 OxnCFhsBdeN受到20点伤害

 ImmutableZYsdlabOOz受到41点伤害

 无惨不等式#YMGTFCOPE受到44点伤害

#念-GP8LKM21D4JZ发起攻击, Grey647638673419回避了攻击

DianmuYKFMWRPXIMCQ发起攻击, 看到这个号说明你要豹了Xa2Zuiqj受到71点伤害

 看到这个号说明你要豹了Xa2Zuiqj被击倒了

回归hkbSNqezR3dr发起攻击, 我力7#W2ib8D使用伤害反弹, 回归hkbSNqezR3dr受到28点伤害

 回归hkbSNqezR3dr被击倒了

稗田阿求OQL68NN8发起攻击, 锋利ⅤEGZPVQMY受到47点伤害

态度jX2HoULfsFU9发起攻击, ➐M1jC95o受到102点伤害

 ➐M1jC95o被击倒了

都江堰00217109183087从冰冻中解除

PraykxtsMobhMzey发起攻击, 我力7#W2ib8D受到52点伤害

湖心SHVPEMAPV发起攻击, Wakaba_mutsumi#pjFhEhSbjy受到47点伤害

末otW7sfqOze发起攻击, 湖心SHVPEMAPV受到94点伤害

 湖心SHVPEMAPV被击倒了

 末otW7sfqOze吞噬了湖心SHVPEMAPV, 末otW7sfqOze属性上升

仇决clFJZCMHS发起攻击, 直接命中#Dfdt3d2uT受到294点伤害

 直接命中#Dfdt3d2uT被击倒了

前尘如梦UYGMHRNX发起攻击, MeltelabRC3P3Go7受到46点伤害

 MeltelabRC3P3Go7的潜行被识破

跙坥咀诅阻珇伹伹怚发起攻击, 末otW7sfqOze受到49点伤害

Grey647638673419发起攻击, 使魔受到66点伤害, 空8089904649511796受到33点伤害

 使魔消失了

石之自由jV3zf35发起攻击, 神谷紫苑#EUKSOXAA受到231点伤害

 神谷紫苑#EUKSOXAA被击倒了

七七#EUEMIGPI发起攻击, tCtrVweRgshV受到47点伤害

都江堰00217109183087发起攻击, Reku_Mochizuki#494460162188受到115点伤害

SDPC#AZLZJQUPN发起攻击, 「OS」#H1#YoRmfG4zW9受到179点伤害

 「OS」#H1#YoRmfG4zW9被击倒了

空8089904649511796发起攻击, tCtrVweRgshV受到104点伤害

 tCtrVweRgshV被击倒了

XIV9lcdAvS9MRKVQPL发起攻击, 跙坥咀诅阻珇伹伹怚受到79点伤害

幻影使用附体, 冥河WyO8MUZPPtKH进入狂暴状态

 幻影消失了

wangifc5NuJx52y1cMSaD发起攻击, Sayakagh8yaICYo受到82点伤害

 Sayakagh8yaICYo被击倒了

 wangifc5NuJx52y1cMSaD吞噬了Sayakagh8yaICYo, wangifc5NuJx52y1cMSaD属性上升

wangif9nWzNbxCJ7wXi8E发起攻击, #念-GP8LKM21D4JZ受到85点伤害

权计WN13vmJnn发起攻击, Wakaba_mutsumi#pjFhEhSbjy受到81点伤害

冷霞洞.鸣湘榔狞发起攻击, 态度jX2HoULfsFU9回避了攻击

Imperio#4B4UZThv发起攻击, 愞㢯老海受到66点伤害

运松翁nkJspy1Oh54A发起攻击, #念-GP8LKM21D4JZ受到21点伤害

 #念-GP8LKM21D4JZ被击倒了

Wakaba_mutsumi#pjFhEhSbjy使用诅咒, 缇亚卡#WOVLHAESD受到93点伤害, 缇亚卡#WOVLHAESD被诅咒了

樱井光#CQMQFHIEV发起攻击, (S("p{GE2up',7%^UGrP受到80点伤害

 樱井光#CQMQFHIEV毒性发作, 樱井光#CQMQFHIEV受到20点伤害

沉睡在悲伤的海洋中#056ARx3e发起攻击, Reku_Mochizuki#494460162188受到199点伤害

 Reku_Mochizuki#494460162188被击倒了, Reku_Mochizuki#494460162188使用护身符抵挡了一次死亡, Reku_Mochizuki#494460162188回复体力1点

血谣染硫决发起攻击, 封魔宣夜8uW56ll受到49点伤害

GordonALYJDXORPTER发起攻击, 我力7#W2ib8D受到104点伤害

 我力7#W2ib8D被击倒了

 GordonALYJDXORPTER毒性发作, GordonALYJDXORPTER受到38点伤害

 GordonALYJDXORPTER被击倒了

PHKBUUPNHGMI发起攻击, 冥河WyO8MUZPPtKH受到74点伤害

哈莫雷特m6bi9z3ZWg9y发起攻击, 权计WN13vmJnn受到80点伤害

无惨不等式#YMGTFCOPE发起攻击, 愞㢯老海受到76点伤害

(S("p{GE2up',7%^UGrP发起攻击, 冷霞洞.鸣湘榔狞受到81点伤害

 冷霞洞.鸣湘榔狞被击倒了

渊HGbwigVwI发起攻击, CWni0rXxe1j受到102点伤害

 CWni0rXxe1j被击倒了

ImmutableZYsdlabOOz发起攻击, Eaquirasd2D5HoYES受到106点伤害

 Eaquirasd2D5HoYES被击倒了

xMq4EGwwCTieuZT发起攻击, PHKBUUPNHGMI回避了攻击

缇亚卡#WOVLHAESD发起吸血攻击, 桃v66wy7tgu27xp受到31点伤害, 缇亚卡#WOVLHAESD回复体力16点

氯化钠8UJMGcZ发起攻击, 渊HGbwigVwI受到47点伤害

「OS」#c1#E7WGTekQTugF使用生命之轮, Imperio#4B4UZThv的体力值与「OS」#c1#E7WGTekQTugF互换

锋利ⅤEGZPVQMY发起攻击, xMq4EGwwCTieuZT受到87点伤害

 xMq4EGwwCTieuZT做出垂死抗争, xMq4EGwwCTieuZT所有属性上升

虚空咦aubHluOMPElA发起攻击, ImmutableZYsdlabOOz受到83点伤害

 ImmutableZYsdlabOOz被击倒了

wangifc5NuJx52y1cMSaD发起攻击, 锋利ⅤEGZPVQMY受到82点伤害

OxnCFhsBdeN发起攻击, 沉睡在悲伤的海洋中#056ARx3e受到16点伤害

wangifc5NuJx52y1cMSaD发起攻击, 愞㢯老海受到87点伤害

末otW7sfqOze发起攻击, 愞㢯老海受到82点伤害

 愞㢯老海被击倒了

七七#EUEMIGPI发动铁壁, 七七#EUEMIGPI防御力大幅上升

冥河WyO8MUZPPtKH发起狂暴攻击, 血谣染硫决受到29点伤害

 血谣染硫决被击倒了

 冥河WyO8MUZPPtKH吞噬了血谣染硫决, 冥河WyO8MUZPPtKH属性上升

虚空辉光舰FpNQjf4keMaT0TB发起攻击, 态度jX2HoULfsFU9受到70点伤害

运松翁nkJspy1Oh54A发起攻击, 桃v66wy7tgu27xp受到53点伤害

前尘如梦UYGMHRNX发起攻击, 稗田阿求OQL68NN8受到110点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 灀瑈篆狓鵃受到88点伤害

冥河WyO8MUZPPtKH发起狂暴攻击, 稗田阿求OQL68NN8受到112点伤害

 稗田阿求OQL68NN8被击倒了

桃v66wy7tgu27xp发起攻击, 泠珞[VmRt[ntb受到37点伤害

 泠珞[VmRt[ntb被击倒了

[oWmjI_$'4Z#[GK,,BX2发起攻击, 沉睡在悲伤的海洋中#056ARx3e受到41点伤害

Reku_Mochizuki#494460162188发起攻击, 石之自由jV3zf35受到20点伤害

Tik_Tok#IBxWzGZtr发起攻击, 七七#EUEMIGPI受到1点伤害

Hypochondriac#TtwN3jZ使用分身, 出现一个新的Hypochondriac#TtwN3jZ

DianmuYKFMWRPXIMCQ发起攻击, 都江堰00217109183087受到86点伤害

 都江堰00217109183087被击倒了

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, 可可萝#EZBAOSOOV受到71点伤害

空8089904649511796发起攻击, OxnCFhsBdeN受到85点伤害

封魔宣夜8uW56ll发起攻击, 末otW7sfqOze受到26点伤害

10l-DYWg发起攻击, 桃v66wy7tgu27xp受到91点伤害

可可萝#EZBAOSOOV发起攻击, Obsession#EYNIZRX受到48点伤害

MeltelabRC3P3Go7发起攻击, OxnCFhsBdeN回避了攻击

PraykxtsMobhMzey发动会心一击, 锋利ⅤEGZPVQMY受到80点伤害

Imperio#4B4UZThv发起攻击, Grey647638673419受到68点伤害

Wakaba_mutsumi#pjFhEhSbjy发起攻击, 空8089904649511796受到77点伤害

PHKBUUPNHGMI发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子回避了攻击

权计WN13vmJnn使用诅咒, 哈莫雷特m6bi9z3ZWg9y回避了攻击

仇决clFJZCMHS发起攻击, MeltelabRC3P3Go7受到118点伤害

跙坥咀诅阻珇伹伹怚使用魅惑, 虚空辉光舰FpNQjf4keMaT0TB回避了攻击

虚空咦aubHluOMPElA使用瘟疫, PHKBUUPNHGMI体力减少69%

虚空辉光舰FpNQjf4keMaT0TB使用净化, Hypochondriac#TtwN3jZ受到69点伤害

 Hypochondriac#TtwN3jZ被击倒了

wangif9nWzNbxCJ7wXi8E发起攻击, Obsession#EYNIZRX受到76点伤害

态度jX2HoULfsFU9发起攻击, Imperio#4B4UZThv受到70点伤害

SDPC#AZLZJQUPN发起攻击, Obsession#EYNIZRX受到133点伤害

 Obsession#EYNIZRX被击倒了

「OS」#c1#E7WGTekQTugF使用冰冻术, PraykxtsMobhMzey受到44点伤害, PraykxtsMobhMzey被冰冻了

锋利ⅤEGZPVQMY发起攻击, Grey647638673419受到63点伤害

 Grey647638673419被击倒了

渊HGbwigVwI发起攻击, 灀瑈篆狓鵃受到64点伤害

 灀瑈篆狓鵃被击倒了

樱井光#CQMQFHIEV发起攻击, 沉睡在悲伤的海洋中#056ARx3e受到98点伤害

 樱井光#CQMQFHIEV毒性发作, 樱井光#CQMQFHIEV受到16点伤害

xMq4EGwwCTieuZT发起攻击, XIV9lcdAvS9MRKVQPL受到70点伤害

wangifc5NuJx52y1cMSaD发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到0点伤害

Hypochondriac#TtwN3jZ发起攻击, wangifc5NuJx52y1cMSaD受到94点伤害

 wangifc5NuJx52y1cMSaD被击倒了

哈莫雷特m6bi9z3ZWg9y发起攻击, 氯化钠8UJMGcZ受到43点伤害

(S("p{GE2up',7%^UGrP发起攻击, 末otW7sfqOze受到47点伤害

石之自由jV3zf35发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到0点伤害

wangifc5NuJx52y1cMSaD发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到0点伤害

沉睡在悲伤的海洋中#056ARx3e发起攻击, wangif9nWzNbxCJ7wXi8E受到106点伤害

 wangif9nWzNbxCJ7wXi8E被击倒了

缇亚卡#WOVLHAESD发起吸血攻击, Wakaba_mutsumi#pjFhEhSbjy受到84点伤害, 缇亚卡#WOVLHAESD回复体力42点

 Wakaba_mutsumi#pjFhEhSbjy被击倒了

末otW7sfqOze发起攻击, 态度jX2HoULfsFU9受到41点伤害

空8089904649511796使用血祭, 召唤出使魔

冥河WyO8MUZPPtKH发起狂暴攻击, xMq4EGwwCTieuZT回避了攻击

XIV9lcdAvS9MRKVQPL投毒, [oWmjI_$'4Z#[GK,,BX2受到32点伤害, [oWmjI_$'4Z#[GK,,BX2中毒

Tik_Tok#IBxWzGZtr使用狂暴术, SDPC#AZLZJQUPN受到71点伤害, SDPC#AZLZJQUPN进入狂暴状态

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, 诅咒使伤害加倍, 缇亚卡#WOVLHAESD受到78点伤害

 子子油渍柚不子油不是子柚渍不不渍柚油柚子从铁壁中解除

Imperio#4B4UZThv发起攻击, 虚空辉光舰FpNQjf4keMaT0TB受到78点伤害

 虚空辉光舰FpNQjf4keMaT0TB被击倒了

七七#EUEMIGPI发起攻击, 可可萝#EZBAOSOOV回避了攻击

Reku_Mochizuki#494460162188使用雷击术

 哈莫雷特m6bi9z3ZWg9y受到24点伤害

 哈莫雷特m6bi9z3ZWg9y受到20点伤害

 哈莫雷特m6bi9z3ZWg9y受到20点伤害

 哈莫雷特m6bi9z3ZWg9y受到13点伤害

DianmuYKFMWRPXIMCQ发起攻击, 樱井光#CQMQFHIEV受到74点伤害

SDPC#AZLZJQUPN发起狂暴攻击, Hypochondriac#TtwN3jZ受到129点伤害

 Hypochondriac#TtwN3jZ被击倒了

 SDPC#AZLZJQUPN从狂暴中解除

前尘如梦UYGMHRNX发起攻击, 「OS」#c1#E7WGTekQTugF受到86点伤害

桃v66wy7tgu27xp发起攻击, 石之自由jV3zf35受到23点伤害

运松翁nkJspy1Oh54A发起攻击, 使魔受到69点伤害, 空8089904649511796受到34点伤害

 空8089904649511796被击倒了

 使魔消失了

樱井光#CQMQFHIEV发起攻击, 运松翁nkJspy1Oh54A受到94点伤害

 运松翁nkJspy1Oh54A被击倒了

 樱井光#CQMQFHIEV毒性发作, 樱井光#CQMQFHIEV受到14点伤害

OxnCFhsBdeN发起攻击, 「OS」#c1#E7WGTekQTugF受到96点伤害

 「OS」#c1#E7WGTekQTugF被击倒了

氯化钠8UJMGcZ发起攻击, PraykxtsMobhMzey受到58点伤害

 PraykxtsMobhMzey被击倒了

跙坥咀诅阻珇伹伹怚发起攻击, 态度jX2HoULfsFU9受到77点伤害

可可萝#EZBAOSOOV发起攻击, XIV9lcdAvS9MRKVQPL回避了攻击

wangifc5NuJx52y1cMSaD发起攻击, 前尘如梦UYGMHRNX受到111点伤害

xMq4EGwwCTieuZT发起攻击, 石之自由jV3zf35受到62点伤害

 石之自由jV3zf35被击倒了

MeltelabRC3P3Go7使用加速术, MeltelabRC3P3Go7进入疾走状态

无惨不等式#YMGTFCOPE发起攻击, 七七#EUEMIGPI受到1点伤害

权计WN13vmJnn发起攻击, 前尘如梦UYGMHRNX受到86点伤害

 前尘如梦UYGMHRNX被击倒了

10l-DYWg发起攻击, 权计WN13vmJnn受到101点伤害

 权计WN13vmJnn被击倒了

[oWmjI_$'4Z#[GK,,BX2发起攻击, OxnCFhsBdeN受到33点伤害

 [oWmjI_$'4Z#[GK,,BX2毒性发作, [oWmjI_$'4Z#[GK,,BX2受到17点伤害

 [oWmjI_$'4Z#[GK,,BX2被击倒了

PHKBUUPNHGMI使用治愈魔法, PHKBUUPNHGMI回复体力89点

态度jX2HoULfsFU9发起攻击, PHKBUUPNHGMI受到58点伤害

封魔宣夜8uW56ll发起攻击, 冥河WyO8MUZPPtKH受到16点伤害

(S("p{GE2up',7%^UGrP发起攻击, MeltelabRC3P3Go7受到42点伤害

 MeltelabRC3P3Go7被击倒了

 (S("p{GE2up',7%^UGrP召唤亡灵, MeltelabRC3P3Go7变成了丧尸

锋利ⅤEGZPVQMY发起攻击, 丧尸受到80点伤害

渊HGbwigVwI发起攻击, Imperio#4B4UZThv受到33点伤害

 Imperio#4B4UZThv被击倒了

虚空咦aubHluOMPElA发起攻击, 封魔宣夜8uW56ll受到60点伤害

Reku_Mochizuki#494460162188使用雷击术

 桃v66wy7tgu27xp受到45点伤害

 桃v66wy7tgu27xp被击倒了

哈莫雷特m6bi9z3ZWg9y发起攻击, Tik_Tok#IBxWzGZtr受到45点伤害

末otW7sfqOze发起攻击, 无惨不等式#YMGTFCOPE受到87点伤害

 无惨不等式#YMGTFCOPE被击倒了

缇亚卡#WOVLHAESD发起攻击, 仇决clFJZCMHS受到84点伤害

 仇决clFJZCMHS被击倒了

SDPC#AZLZJQUPN发起攻击, XIV9lcdAvS9MRKVQPL受到53点伤害

 XIV9lcdAvS9MRKVQPL被击倒了

跙坥咀诅阻珇伹伹怚发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到56点伤害

七七#EUEMIGPI发起攻击, (S("p{GE2up',7%^UGrP受到71点伤害

 七七#EUEMIGPI从铁壁中解除

Tik_Tok#IBxWzGZtr发起攻击, 虚空咦aubHluOMPElA受到81点伤害

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, 虚空咦aubHluOMPElA受到63点伤害

冥河WyO8MUZPPtKH发起狂暴攻击, Reku_Mochizuki#494460162188受到135点伤害

 Reku_Mochizuki#494460162188被击倒了

 冥河WyO8MUZPPtKH从狂暴中解除

可可萝#EZBAOSOOV发起攻击, 诅咒使伤害加倍, 缇亚卡#WOVLHAESD受到106点伤害

 缇亚卡#WOVLHAESD被击倒了

沉睡在悲伤的海洋中#056ARx3e发起攻击, 虚空咦aubHluOMPElA受到73点伤害

 虚空咦aubHluOMPElA被击倒了, 虚空咦aubHluOMPElA使用护身符抵挡了一次死亡, 虚空咦aubHluOMPElA回复体力2点

氯化钠8UJMGcZ使用生命之轮, 子子油渍柚不子油不是子柚渍不不渍柚油柚子的体力值与氯化钠8UJMGcZ互换

xMq4EGwwCTieuZT发起攻击, 丧尸受到104点伤害

 丧尸消失了

PHKBUUPNHGMI发起攻击, 可可萝#EZBAOSOOV受到135点伤害

 可可萝#EZBAOSOOV被击倒了

哈莫雷特m6bi9z3ZWg9y发起攻击, 封魔宣夜8uW56ll受到21点伤害

锋利ⅤEGZPVQMY发起攻击, 冥河WyO8MUZPPtKH受到77点伤害

 冥河WyO8MUZPPtKH被击倒了

(S("p{GE2up',7%^UGrP发动会心一击, 樱井光#CQMQFHIEV回避了攻击

wangifc5NuJx52y1cMSaD发动铁壁, wangifc5NuJx52y1cMSaD防御力大幅上升

樱井光#CQMQFHIEV发起攻击, 跙坥咀诅阻珇伹伹怚受到92点伤害

 跙坥咀诅阻珇伹伹怚被击倒了

 樱井光#CQMQFHIEV毒性发作, 樱井光#CQMQFHIEV受到11点伤害

 樱井光#CQMQFHIEV从中毒中解除

七七#EUEMIGPI发起攻击, wangifc5NuJx52y1cMSaD受到1点伤害

OxnCFhsBdeN投毒, 氯化钠8UJMGcZ受到34点伤害, 氯化钠8UJMGcZ中毒

末otW7sfqOze发起攻击, wangifc5NuJx52y1cMSaD受到1点伤害

封魔宣夜8uW56ll发起攻击, wangifc5NuJx52y1cMSaD受到1点伤害

渊HGbwigVwI发起攻击, 封魔宣夜8uW56ll受到51点伤害

 封魔宣夜8uW56ll被击倒了

DianmuYKFMWRPXIMCQ发起攻击, xMq4EGwwCTieuZT回避了攻击

xMq4EGwwCTieuZT发起攻击, 10l-DYWg受到44点伤害

态度jX2HoULfsFU9发起攻击, PHKBUUPNHGMI受到108点伤害

 PHKBUUPNHGMI被击倒了

SDPC#AZLZJQUPN发起攻击, 沉睡在悲伤的海洋中#056ARx3e受到94点伤害

 沉睡在悲伤的海洋中#056ARx3e被击倒了

10l-DYWg发起攻击, DianmuYKFMWRPXIMCQ受到74点伤害

 DianmuYKFMWRPXIMCQ被击倒了

Tik_Tok#IBxWzGZtr发起攻击, 氯化钠8UJMGcZ受到44点伤害

虚空咦aubHluOMPElA发起攻击, wangifc5NuJx52y1cMSaD受到1点伤害

(S("p{GE2up',7%^UGrP潜行到xMq4EGwwCTieuZT身后

末otW7sfqOze发起攻击, 渊HGbwigVwI受到83点伤害

 渊HGbwigVwI被击倒了

锋利ⅤEGZPVQMY发起攻击, 七七#EUEMIGPI受到41点伤害

10l-DYWg发起攻击, xMq4EGwwCTieuZT受到55点伤害

 xMq4EGwwCTieuZT被击倒了

七七#EUEMIGPI发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到72点伤害

 子子油渍柚不子油不是子柚渍不不渍柚油柚子被击倒了

OxnCFhsBdeN发起攻击, wangifc5NuJx52y1cMSaD受到1点伤害

态度jX2HoULfsFU9发起攻击, SDPC#AZLZJQUPN受到55点伤害

 SDPC#AZLZJQUPN被击倒了

哈莫雷特m6bi9z3ZWg9y发起攻击, 七七#EUEMIGPI受到54点伤害

 七七#EUEMIGPI被击倒了

wangifc5NuJx52y1cMSaD发起攻击, 虚空咦aubHluOMPElA受到50点伤害

 虚空咦aubHluOMPElA被击倒了

(S("p{GE2up',7%^UGrP发起攻击, 氯化钠8UJMGcZ受到59点伤害

 氯化钠8UJMGcZ被击倒了

 (S("p{GE2up',7%^UGrP召唤亡灵, 氯化钠8UJMGcZ变成了丧尸

樱井光#CQMQFHIEV发起攻击, 末otW7sfqOze受到44点伤害

 末otW7sfqOze被击倒了

10l-DYWg发起攻击, Tik_Tok#IBxWzGZtr受到58点伤害

 Tik_Tok#IBxWzGZtr被击倒了

态度jX2HoULfsFU9发起攻击, wangifc5NuJx52y1cMSaD受到1点伤害

OxnCFhsBdeN发起攻击, 丧尸受到88点伤害

丧尸发起攻击, 态度jX2HoULfsFU9受到56点伤害

 态度jX2HoULfsFU9被击倒了

(S("p{GE2up',7%^UGrP发起攻击, OxnCFhsBdeN受到36点伤害

 OxnCFhsBdeN被击倒了

樱井光#CQMQFHIEV发起攻击, (S("p{GE2up',7%^UGrP回避了攻击

锋利ⅤEGZPVQMY发起攻击, 丧尸回避了攻击

wangifc5NuJx52y1cMSaD发起攻击, 丧尸回避了攻击

 wangifc5NuJx52y1cMSaD从铁壁中解除

哈莫雷特m6bi9z3ZWg9y使用狂暴术, 樱井光#CQMQFHIEV受到63点伤害

 樱井光#CQMQFHIEV被击倒了

(S("p{GE2up',7%^UGrP发起攻击, wangifc5NuJx52y1cMSaD受到78点伤害

 wangifc5NuJx52y1cMSaD被击倒了

10l-DYWg发起攻击, 丧尸受到61点伤害

丧尸发起攻击, 10l-DYWg受到24点伤害

10l-DYWg发起攻击, 丧尸受到122点伤害

 丧尸消失了

(S("p{GE2up',7%^UGrP发起攻击, 锋利ⅤEGZPVQMY受到58点伤害

 锋利ⅤEGZPVQMY被击倒了

哈莫雷特m6bi9z3ZWg9y发起攻击, 10l-DYWg受到117点伤害

 10l-DYWg被击倒了

(S("p{GE2up',7%^UGrP发起攻击, 哈莫雷特m6bi9z3ZWg9y受到70点伤害

 哈莫雷特m6bi9z3ZWg9y被击倒了"###;
        let (raw_input, expected_lines) = parse_embedded_fight_case(
            FIGHT_CASE,
            "embedded fight case must contain a blank separator between input and trace",
            "embedded fight trace is empty",
        );
        let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
        let (actual_lines, guard) = collect_replay_lines(&mut runner, 50_000, true);
        assert!(guard < 50_000, "fight.md combat did not finish in expected rounds");
        if actual_lines != expected_lines {
            let min_len = actual_lines.len().min(expected_lines.len());
            let mismatch_idx = actual_lines
                .iter()
                .zip(expected_lines.iter())
                .position(|(lhs, rhs)| lhs != rhs)
                .unwrap_or(min_len);
            let ctx_start = mismatch_idx.saturating_sub(5);
            let ctx_end = (mismatch_idx + 5).min(min_len);
            eprintln!("fight_large mismatch context [{ctx_start}..{ctx_end}):");
            for idx in ctx_start..ctx_end {
                eprintln!(
                    "  idx={idx}: actual={:?} | expected={:?}",
                    actual_lines.get(idx),
                    expected_lines.get(idx)
                );
            }
            panic!(
                "fight_large mismatch at idx={mismatch_idx}, actual_len={}, expected_len={}, actual={:?}, expected={:?}",
                actual_lines.len(),
                expected_lines.len(),
                actual_lines.get(mismatch_idx),
                expected_lines.get(mismatch_idx)
            );
        }
    }

    #[test]
    fn charm_state_redirects_target_group() {
        let raw_input = "a\nc\n\nb";
        let runner = runners::Runner::new_from_namerena_raw(raw_input.to_string()).unwrap();
        let actor = runner.world.groups[0][0];
        let ally = runner.world.groups[0][1];
        let enemy = runner.world.groups[1][0];
        runner
            .storage
            .just_get_player_mut(actor)
            .expect("cannot get actor")
            .set_state(crate::player::skill::charm::CharmState {
                group_id: enemy,
                target: Some(actor),
                on_post_action: None,
                step: 2,
            });

        let targets = runners::select_targets(actor, &runner.world, &runner.storage);
        assert!(targets.enemy_alive.contains(&ally));
        assert!(!targets.enemy_alive.contains(&enemy));
    }

    #[test]
    fn runtime_spawn_queue_syncs_into_world_group() {
        let raw_input = "owner\n\nenemy";
        let mut runner = runners::Runner::new_from_namerena_raw(raw_input.to_string()).unwrap();
        let owner = runner.world.groups[0][0];
        let mut minion =
            crate::player::Player::new_from_namerena_raw("owner?minion".to_string(), runner.storage.clone()).unwrap();
        minion.set_state(crate::player::skill::act::minion::MinionRuntimeState {
            owner: Some(owner),
            kind: crate::player::skill::act::minion::MinionKind::Clone,
        });
        let minion_id = minion.as_ptr();
        runner.storage.queue_spawn(owner, minion);

        let mut updates = crate::engine::update::RunUpdates::new();
        runner.round_tick(&mut updates);
        assert!(runner.world.groups[0].contains(&minion_id));
    }

    #[test]
    fn runtime_remove_queue_syncs_world_and_storage() {
        let raw_input = "owner\n\nenemy";
        let mut runner = runners::Runner::new_from_namerena_raw(raw_input.to_string()).unwrap();
        let enemy = runner.world.groups[1][0];
        runner.storage.queue_remove_player(enemy);

        let mut updates = crate::engine::update::RunUpdates::new();
        runner.round_tick(&mut updates);
        assert!(!runner.world.groups[1].contains(&enemy));
        assert!(runner.storage.get_player(&enemy).is_none());
    }
}

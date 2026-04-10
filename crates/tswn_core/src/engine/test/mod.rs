use super::*;

mod runner;
use self::runner::{collect_replay_lines, winner_names};

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

mod split_namerena_groups {
    use super::*;

    #[test]
    fn basic_split() {
        let raw_input = "a\nb\nc".to_string();
        let groups = runners::Runner::split_namerena_into_groups(raw_input);
        assert_eq!(groups, (plrs!("a", "b", "c"), plr!()));

        let raw_input = "a\nb\nc\n".to_string();
        let groups = runners::Runner::split_namerena_into_groups(raw_input);
        assert_eq!(groups, (plrs!("a", "b", "c"), plr!()));

        let raw_input = "a\nb\nc\n\n".to_string();
        let groups = runners::Runner::split_namerena_into_groups(raw_input);
        assert_eq!(groups, (plrs!("a", "b", "c"), plr!()));
    }

    #[test]
    fn split_teams() {
        let raw_input = "a\nb\n\nc\nd".to_string();
        let groups = runners::Runner::split_namerena_into_groups(raw_input);
        assert_eq!(groups, (vec![plr!["a", "b"], plr!["c", "d"]], plr!()));
    }

    #[test]
    fn more_than_2_newline() {
        for x in 2..10 {
            let new_lines = "\n".repeat(x);
            let raw_input = format!("a\nb{new_lines}c\nd");
            let groups = runners::Runner::split_namerena_into_groups(raw_input);
            assert_eq!(groups, (vec![plr!["a", "b"], plr!["c", "d"]], plr!()));
        }

        for x in 2..10 {
            let new_lines = "\n".repeat(x);
            let raw_input = format!("a\nb{new_lines}c\nd{new_lines}e");
            let groups = runners::Runner::split_namerena_into_groups(raw_input);
            assert_eq!(groups, (vec![plr!["a", "b"], plr!["c", "d"], plr!["e"]], plr!()));
        }
    }

    #[test]
    fn lot_of_teams() {
        let raw_input = "a\nb\nc\nd\ne\nf".to_string();
        let groups = runners::Runner::split_namerena_into_groups(raw_input);
        assert_eq!(groups, (plrs!("a", "b", "c", "d", "e", "f"), plr!()));
    }

    #[test]
    fn trim_js_line_end_before_grouping() {
        let raw_input = "a\u{3000}\n\u{3000}\n\u{3000}b\u{3000}".to_string();
        let groups = runners::Runner::split_namerena_into_groups(raw_input);
        assert_eq!(groups, (vec![plr!["a"], plr!["\u{3000}b"]], plr!()));
    }

    #[test]
    fn normal_seed() {
        let raw_input = "seed: a@!\nb\nc".to_string();
        let groups = runners::Runner::split_namerena_into_groups(raw_input);
        assert_eq!(groups, (plrs!("seed: a@!", "b", "c"), plr!["seed: a@!"]));
    }

    #[test]
    fn need_fix_seed1() {
        let raw_input = "aaaa\nbbbb\n\nseed: a@!".to_string();
        let groups = runners::Runner::split_namerena_into_groups(raw_input);
        assert_eq!(groups, (vec![plr!("aaaa", "bbbb", "seed: a@!")], plr!["seed: a@!"]))
    }

    #[test]
    fn need_fix_seed2() {
        let raw_input = "seed: a@!\n\naaaa\nbbbb".to_string();
        let groups = runners::Runner::split_namerena_into_groups(raw_input);
        assert_eq!(groups, (vec![plr!("seed: a@!", "aaaa", "bbbb")], plr!["seed: a@!"]))
    }
}

mod prepared_raw_consistency {
    use super::*;

    fn winner_names_sorted(runner: &runners::Runner) -> Vec<String> {
        let mut names = winner_names(runner);
        names.sort();
        names
    }

    #[test]
    fn no_seed_runner_and_prepared_runner_match() {
        let raw_input = "喘际瞬爆@昀澤\n\n蕾蒂·怀特洛可-65HEZHB264LFPFQ@Squall".to_string();

        let mut raw_runner = runners::Runner::new_from_namerena_raw(raw_input.clone()).expect("raw runner should build");
        let (groups, seed) = runners::Runner::split_namerena_into_groups(raw_input);
        assert!(seed.is_empty(), "expected no seed in raw input");

        let prepared = runners::Runner::prepare_groups(&groups).expect("prepared runner should build");
        let mut prepared_runner =
            runners::Runner::new_from_prepared_with_seed(&prepared, &[]).expect("prepared runner without seed should build");

        let (raw_lines, raw_rounds, raw_score) = collect_replay_lines(&mut raw_runner, 100_000, true);
        let (prepared_lines, prepared_rounds, prepared_score) = collect_replay_lines(&mut prepared_runner, 100_000, true);

        assert_eq!(
            winner_names_sorted(&raw_runner),
            winner_names_sorted(&prepared_runner),
            "winner names differ between raw and prepared without seed"
        );
        assert_eq!(
            raw_score, prepared_score,
            "battle score differs between raw and prepared without seed"
        );
        assert_eq!(
            raw_rounds, prepared_rounds,
            "round count differs between raw and prepared without seed"
        );
        assert_eq!(
            raw_lines, prepared_lines,
            "replay trace differs between raw and prepared without seed"
        );
    }
}

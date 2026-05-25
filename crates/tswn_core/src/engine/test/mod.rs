//! # 引擎测试模块 (engine::test)
//!
//! 本模块收纳 `engine` 相关测试，主要覆盖两类场景：
//!
//! - [`runner`]：按真实对局回放比对战斗输出、胜者与累计分数
//! - 内联测试：校验输入分组、种子修正等基础行为
//!
//! 这里的测试更偏向“对外行为一致性”验证，用来确保 Rust 引擎与既有名竞规则保持一致。

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

    #[test]
    fn benchmark_marker_default_score_shape_matches_js() {
        let raw_input = "!test!\n\naaaa\nbbbb".to_string();
        let groups = runners::Runner::split_namerena_into_groups(raw_input);
        assert_eq!(groups, (vec![plr!("!test!"), plr!("aaaa", "bbbb")], plr!()));
    }

    #[test]
    fn benchmark_marker_bang_score_shape_matches_js() {
        let raw_input = "!test!\n!\n\naaaa\nbbbb".to_string();
        let groups = runners::Runner::split_namerena_into_groups(raw_input);
        assert_eq!(groups, (vec![plr!("!test!", "!"), plr!("aaaa", "bbbb")], plr!()));
    }

    #[test]
    fn bang_team_player_stays_in_roster_not_seed() {
        let raw_input = "xxxx@!\n\nnormal".to_string();
        let groups = runners::Runner::split_namerena_into_groups(raw_input);
        assert_eq!(groups, (vec![plr!("xxxx@!"), plr!("normal")], plr!()));
    }

    #[test]
    fn bang_team_player_builds_as_testex() {
        let runner = runners::Runner::new_from_namerena_raw("xxxx@!\n\nnormal".to_string()).unwrap();
        let testex = runner
            .world
            .all_plrs()
            .into_iter()
            .find_map(|id| {
                let player = runner.storage.get_player(&id)?;
                (player.id_name() == "xxxx").then_some(player.player_type())
            })
            .expect("xxxx@! player should exist");
        assert_eq!(testex, crate::player::PlayerType::TestEx);
    }

    #[test]
    fn bang_testex_same_team_does_not_upgrade() {
        let raw_input = "aaaaaa\n33554632@!\n\n33554633@!\n33554634@!".to_string();
        let (groups, seed) = runners::Runner::split_namerena_into_groups(raw_input);
        let runner =
            runners::Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, crate::player::eval_name::WIN_RATE_EVAL_RQ)
                .unwrap();
        let magic = runner
            .world
            .all_plrs()
            .into_iter()
            .find_map(|id| {
                let player = runner.storage.get_player(&id)?;
                (player.id_name() == "33554634").then_some(player.get_status().magic)
            })
            .expect("33554634@! player should exist");
        assert_eq!(magic, 46);
    }

    #[test]
    fn bang_score_round_235_clone_raw_base_matches_md5_winner() {
        let raw_input = "aaaaaa\n33555133@!\n\n33555134@!\n33555135@!".to_string();
        let (groups, seed) = runners::Runner::split_namerena_into_groups(raw_input);
        let mut runner =
            runners::Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, crate::player::eval_name::WIN_RATE_EVAL_RQ)
                .unwrap();
        runner.run_to_completion();

        let mut winners = winner_names(&runner);
        winners.sort();
        assert_eq!(winners, vec!["33555133", "33555133?0", "aaaaaa", "aaaaaa?0", "aaaaaa?1"]);
    }

    #[test]
    fn score_round_4950_reused_summon_clears_runtime_states_matches_md5_winner() {
        let raw_input = "aaaa@aaaaa\n33569278@\u{0002}\n\n33569279@\u{0002}\n33569280@\u{0002}".to_string();
        let (groups, seed) = runners::Runner::split_namerena_into_groups(raw_input);
        let mut runner =
            runners::Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, crate::player::eval_name::WIN_RATE_EVAL_RQ)
                .unwrap();
        runner.run_to_completion();

        let mut winners = winner_names(&runner);
        winners.sort();
        assert_eq!(winners, vec!["33569278", "aaaa"]);
    }

    #[test]
    fn bang_score_round_8662_broken_iron_clears_immediately_matches_md5_winner() {
        let raw_input = "aaaa@aaaaa\n33580414@!\n\n33580415@!\n33580416@!".to_string();
        let (groups, seed) = runners::Runner::split_namerena_into_groups(raw_input);
        let mut runner =
            runners::Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, crate::player::eval_name::WIN_RATE_EVAL_RQ)
                .unwrap();
        runner.run_to_completion();

        let mut winners = winner_names(&runner);
        winners.sort();
        assert_eq!(winners, vec!["33580415", "33580416", "33580416?0"]);
    }

    #[test]
    fn bang_score_round_6024_dead_owner_minion_not_protect_candidate_matches_md5_winner() {
        let raw_input = "[Face: 212]@!\n33572500@!\n\n33572501@!\n33572502@!".to_string();
        let (groups, seed) = runners::Runner::split_namerena_into_groups(raw_input);
        let mut runner =
            runners::Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, crate::player::eval_name::WIN_RATE_EVAL_RQ)
                .unwrap();
        runner.run_to_completion();

        let mut winners = winner_names(&runner);
        winners.sort();
        assert_eq!(winners, vec!["33572500", "[Face: 212]", "[Face: 212]?0"]);
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

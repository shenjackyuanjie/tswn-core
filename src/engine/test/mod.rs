use super::*;

mod runner;

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

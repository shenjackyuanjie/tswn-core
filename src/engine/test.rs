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
    use crate::engine::update::UpdateType;

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
        let mut events = Vec::new();

        let mut guard = 0usize;
        while !runner.have_winner() && guard < 256 {
            let updates = runner.main_round();
            for update in updates.updates {
                if matches!(update.update_type, UpdateType::NextLine) {
                    continue;
                }
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
                msg = msg.replace("[2]", &param);
                events.push(msg);
            }
            guard += 1;
        }

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

        let winner = runner
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
            .collect::<Vec<String>>();
        assert_eq!(winner, vec!["help".to_string()]);
    }

    #[test]
    fn fight_md_should_match_trace_step_by_step() {
        let fight_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fight.md");
        let fight_text = std::fs::read_to_string(&fight_path).expect("cannot read fight.md");
        let fight_text = fight_text.replace("\r\n", "\n").replace('\r', "\n");
        let (raw_input, expected_part) = fight_text
            .split_once("\n\n\n")
            .expect("fight.md must contain a blank separator between input and trace");

        let expected_lines = expected_part
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| line.to_string())
            .collect::<Vec<String>>();
        assert!(!expected_lines.is_empty(), "fight.md trace is empty");

        let mut runner = runners::Runner::new_from_namerena_raw(raw_input.trim_end().to_string()).unwrap();
        let mut actual_lines = Vec::new();
        let mut guard = 0usize;
        while !runner.have_winner() && guard < 50_000 {
            let updates = runner.main_round();
            let mut parts = Vec::new();
            for update in updates.updates {
                if matches!(update.update_type, UpdateType::NextLine) {
                    continue;
                }
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
                msg = msg.replace("[2]", &param);
                parts.push(msg);
            }
            if !parts.is_empty() {
                actual_lines.push(parts.join(", "));
            }
            guard += 1;
        }
        assert!(guard < 50_000, "fight.md combat did not finish in expected rounds");
        assert_eq!(actual_lines, expected_lines);
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

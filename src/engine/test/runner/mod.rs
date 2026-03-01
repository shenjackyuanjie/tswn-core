
    use super::*;
    use crate::engine::update::{RunUpdate, UpdateType};

    mod fight_large;
    mod large_01_10;
    mod large_11_17;
    mod large_18_22;
    mod large_23_30;
    mod simple;
    mod small;

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
        // player 从 world groups 中移除，但保留在 storage 中（对齐 JS/Dart 设计：对象仍可被引用以查找名字）
        assert!(!runner.world.groups[1].contains(&enemy));
        assert!(runner.storage.get_player(&enemy).is_some());
    }

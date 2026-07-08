use tswn_core::engine::engine_core::EngineCore;
use tswn_core::engine::tick;
use tswn_core::engine::update::{RunUpdate, RunUpdates, UpdateType};
use tswn_core::player::skill::act::minion::{MinionKind, MinionRuntimeState};
use tswn_core::player::{Player, PlayerType};
use tswn_core::Runner;

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
            $($x.to_string()),+,
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

fn format_update_message(runner: &Runner, update: &RunUpdate) -> String {
    let caster = runner
        .storage
        .get_player(&update.caster)
        .map(|plr| plr.display_name())
        .unwrap_or_else(|| format!("#{}", update.caster));
    let target = runner
        .storage
        .get_player(&update.target)
        .map(|plr| plr.display_name())
        .unwrap_or_else(|| format!("#{}", update.target));
    let mut msg = update.message.to_string();
    msg = msg.replace("[0]", &caster);
    msg = msg.replace("[1]", &target);
    let param = if let Some(p) = update.param {
        p.to_string()
    } else if update.targets.is_empty() {
        update.score.to_string()
    } else {
        update
            .targets
            .iter()
            .map(|id| {
                runner
                    .storage
                    .get_player(id)
                    .map(|plr| plr.display_name())
                    .unwrap_or_else(|| format!("#{id}"))
            })
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

fn collect_replay_lines(runner: &mut Runner, max_rounds: usize, normalize: bool) -> (Vec<String>, usize, u64) {
    let mut lines = Vec::new();
    let mut guard = 0usize;
    let mut total_score = 0u64;
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
            if update.score > 0 {
                total_score += update.score as u64;
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
    (lines, guard, total_score)
}

fn winner_names(runner: &Runner) -> Vec<String> {
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

#[test]
fn split_basic_inputs() {
    let groups = Runner::split_namerena_into_groups("a\nb\nc".to_string());
    assert_eq!(groups, (plrs!("a", "b", "c"), plr!()));

    let groups = Runner::split_namerena_into_groups("a\nb\nc\n".to_string());
    assert_eq!(groups, (plrs!("a", "b", "c"), plr!()));

    let groups = Runner::split_namerena_into_groups("a\nb\nc\n\n".to_string());
    assert_eq!(groups, (plrs!("a", "b", "c"), plr!()));
}

#[test]
fn split_teams() {
    let groups = Runner::split_namerena_into_groups("a\nb\n\nc\nd".to_string());
    assert_eq!(groups, (vec![plr!["a", "b"], plr!["c", "d"]], plr!()));
}

#[test]
fn split_collapses_more_than_two_newlines() {
    for x in 2..10 {
        let new_lines = "\n".repeat(x);
        let raw_input = format!("a\nb{new_lines}c\nd");
        let groups = Runner::split_namerena_into_groups(raw_input);
        assert_eq!(groups, (vec![plr!["a", "b"], plr!["c", "d"]], plr!()));
    }

    for x in 2..10 {
        let new_lines = "\n".repeat(x);
        let raw_input = format!("a\nb{new_lines}c\nd{new_lines}e");
        let groups = Runner::split_namerena_into_groups(raw_input);
        assert_eq!(groups, (vec![plr!["a", "b"], plr!["c", "d"], plr!["e"]], plr!()));
    }
}

#[test]
fn split_lot_of_teams() {
    let groups = Runner::split_namerena_into_groups("a\nb\nc\nd\ne\nf".to_string());
    assert_eq!(groups, (plrs!("a", "b", "c", "d", "e", "f"), plr!()));
}

#[test]
fn split_trims_js_line_end_before_grouping() {
    let groups = Runner::split_namerena_into_groups("a\u{3000}\n\u{3000}\n\u{3000}b\u{3000}".to_string());
    assert_eq!(groups, (vec![plr!["a"], plr!["\u{3000}b"]], plr!()));
}

#[test]
fn split_keeps_seed_lines() {
    let groups = Runner::split_namerena_into_groups("seed: a@!\nb\nc".to_string());
    assert_eq!(groups, (plrs!("seed: a@!", "b", "c"), plr!["seed: a@!"]));

    let groups = Runner::split_namerena_into_groups("aaaa\nbbbb\n\nseed: a@!".to_string());
    assert_eq!(groups, (vec![plr!("aaaa", "bbbb", "seed: a@!")], plr!["seed: a@!"]));

    let groups = Runner::split_namerena_into_groups("seed: a@!\n\naaaa\nbbbb".to_string());
    assert_eq!(groups, (vec![plr!("seed: a@!", "aaaa", "bbbb")], plr!["seed: a@!"]));
}

#[test]
fn split_benchmark_markers_match_js_shape() {
    let groups = Runner::split_namerena_into_groups("!test!\n\naaaa\nbbbb".to_string());
    assert_eq!(groups, (vec![plr!("!test!"), plr!("aaaa", "bbbb")], plr!()));

    let groups = Runner::split_namerena_into_groups("!test!\n!\n\naaaa\nbbbb".to_string());
    assert_eq!(groups, (vec![plr!("!test!", "!"), plr!("aaaa", "bbbb")], plr!()));
}

#[test]
fn bang_team_player_stays_in_roster_not_seed() {
    let groups = Runner::split_namerena_into_groups("xxxx@!\n\nnormal".to_string());
    assert_eq!(groups, (vec![plr!("xxxx@!"), plr!("normal")], plr!()));
}

#[test]
fn bang_team_player_builds_as_testex() {
    let runner = Runner::new_from_namerena_raw("xxxx@!\n\nnormal".to_string()).unwrap();
    let testex = runner
        .world
        .all_plrs()
        .into_iter()
        .find_map(|id| {
            let player = runner.storage.get_player(&id)?;
            (player.id_name() == "xxxx").then_some(player.player_type())
        })
        .expect("xxxx@! player should exist");
    assert_eq!(testex, PlayerType::TestEx);
}

#[test]
fn bang_testex_same_team_does_not_upgrade() {
    let raw_input = "aaaaaa\n33554632@!\n\n33554633@!\n33554634@!".to_string();
    let (groups, seed) = Runner::split_namerena_into_groups(raw_input);
    let runner =
        Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, tswn_core::player::eval_name::WIN_RATE_EVAL_RQ)
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
    let (groups, seed) = Runner::split_namerena_into_groups(raw_input);
    let mut runner =
        Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, tswn_core::player::eval_name::WIN_RATE_EVAL_RQ)
            .unwrap();
    runner.run_to_completion();

    let mut winners = winner_names(&runner);
    winners.sort();
    assert_eq!(winners, vec!["33555133", "33555133?0", "aaaaaa", "aaaaaa?0", "aaaaaa?1"]);
}

#[test]
fn score_round_4950_reused_summon_clears_runtime_states_matches_md5_winner() {
    let raw_input = "aaaa@aaaaa\n33569278@\u{0002}\n\n33569279@\u{0002}\n33569280@\u{0002}".to_string();
    let (groups, seed) = Runner::split_namerena_into_groups(raw_input);
    let mut runner =
        Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, tswn_core::player::eval_name::WIN_RATE_EVAL_RQ)
            .unwrap();
    runner.run_to_completion();

    let mut winners = winner_names(&runner);
    winners.sort();
    assert_eq!(winners, vec!["33569278", "aaaa"]);
}

#[test]
fn bang_score_round_8662_broken_iron_clears_immediately_matches_md5_winner() {
    let raw_input = "aaaa@aaaaa\n33580414@!\n\n33580415@!\n33580416@!".to_string();
    let (groups, seed) = Runner::split_namerena_into_groups(raw_input);
    let mut runner =
        Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, tswn_core::player::eval_name::WIN_RATE_EVAL_RQ)
            .unwrap();
    runner.run_to_completion();

    let mut winners = winner_names(&runner);
    winners.sort();
    assert_eq!(winners, vec!["33580415", "33580416", "33580416?0"]);
}

#[test]
fn bang_score_round_6024_dead_owner_minion_not_protect_candidate_matches_md5_winner() {
    let raw_input = "[Face: 212]@!\n33572500@!\n\n33572501@!\n33572502@!".to_string();
    let (groups, seed) = Runner::split_namerena_into_groups(raw_input);
    let mut runner =
        Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, tswn_core::player::eval_name::WIN_RATE_EVAL_RQ)
            .unwrap();
    runner.run_to_completion();

    let mut winners = winner_names(&runner);
    winners.sort();
    assert_eq!(winners, vec!["33572500", "[Face: 212]", "[Face: 212]?0"]);
}

#[test]
fn no_seed_runner_and_prepared_runner_match() {
    let raw_input = "喘际瞬爆@昀澤\n\n蕾蒂·怀特洛可-65HEZHB264LFPFQ@Squall".to_string();

    let mut raw_runner = Runner::new_from_namerena_raw(raw_input.clone()).expect("raw runner should build");
    let (groups, seed) = Runner::split_namerena_into_groups(raw_input);
    assert!(seed.is_empty(), "expected no seed in raw input");

    let prepared = Runner::prepare_groups(&groups).expect("prepared runner should build");
    let mut prepared_runner = Runner::new_from_prepared_with_seed(&prepared, &[]).expect("prepared runner should build");

    let (raw_lines, raw_rounds, raw_score) = collect_replay_lines(&mut raw_runner, 100_000, true);
    let (prepared_lines, prepared_rounds, prepared_score) = collect_replay_lines(&mut prepared_runner, 100_000, true);

    let mut raw_winners = winner_names(&raw_runner);
    raw_winners.sort();
    let mut prepared_winners = winner_names(&prepared_runner);
    prepared_winners.sort();
    assert_eq!(raw_winners, prepared_winners, "winner names differ between raw and prepared without seed");
    assert_eq!(raw_score, prepared_score, "battle score differs between raw and prepared without seed");
    assert_eq!(raw_rounds, prepared_rounds, "round count differs between raw and prepared without seed");
    assert_eq!(raw_lines, prepared_lines, "replay trace differs between raw and prepared without seed");
}

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

fn assert_prepare_vs_raw_case(mode: PreparedParityMode, case_idx: usize, library: &[String]) {
    let total_players = mode.total_players();
    let mut players = library[..total_players].to_vec();
    let shift = case_idx % players.len();
    players.rotate_left(shift);
    let seed = format!("seed:{}@!", tswn_core::engine::PROFILE_START as usize + case_idx);
    let raw = mode.build_input(&players, &seed);
    let (groups, parsed_seed) = Runner::split_namerena_into_groups(raw.clone());

    let mut raw_runner = Runner::new_from_namerena_raw(raw).unwrap();
    let prepared = Runner::prepare_groups_with_eval_rq(&groups, tswn_core::player::eval_name::DEFAULT_EVAL_RQ).unwrap();
    let mut prepared_runner = Runner::new_from_prepared_with_seed(&prepared, &parsed_seed).unwrap();

    assert_eq!(
        raw_runner.input_groups,
        prepared_runner.input_groups,
        "input_groups mismatch for mode={} case_idx={case_idx}",
        mode.label()
    );

    let (raw_lines, raw_guard, raw_score) = collect_replay_lines(&mut raw_runner, 10_000, true);
    let (prepared_lines, prepared_guard, prepared_score) = collect_replay_lines(&mut prepared_runner, 10_000, true);

    assert!(raw_guard < 10_000, "raw runner did not finish for mode={} case_idx={case_idx}", mode.label());
    assert!(
        prepared_guard < 10_000,
        "prepared runner did not finish for mode={} case_idx={case_idx}",
        mode.label()
    );
    assert_eq!(raw_score, prepared_score, "battle score mismatch for mode={} case_idx={case_idx}", mode.label());
    assert_eq!(winner_names(&raw_runner), winner_names(&prepared_runner), "winner mismatch");
    assert_eq!(raw_lines, prepared_lines, "full replay trace mismatch for mode={} case_idx={case_idx}", mode.label());
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

#[test]
fn sort_int_test() {
    let runner = Runner::new_from_namerena_raw("aaa\nbbb\nseed: aaaa@!".to_string()).unwrap();
    let ints = [16_391_432, 11_292_362];
    assert!(!runner.have_winner());

    for (i, plr) in runner
        .world
        .groups
        .iter()
        .flatten()
        .filter(|plr| runner.storage.get_player(plr).expect("player should exist").is_seed_plr())
        .enumerate()
    {
        let plr = runner.storage.get_player(plr).expect("player should exist");
        assert_eq!(plr.sort_int as u32, ints[i]);
    }
}

#[test]
fn sort_int_test2() {
    let runner = Runner::new_from_namerena_raw("aaa\nbbb".to_string()).unwrap();
    let ints = [7_525_315, 8_712_372];
    assert!(!runner.have_winner());

    for (i, plr) in runner.world.groups.iter().flatten().enumerate() {
        let plr = runner.storage.get_player(plr).expect("player should exist");
        assert_eq!(plr.sort_int as u32, ints[i]);
    }
}

#[test]
fn input_order_should_not_change_initial_state() {
    let runner_ab = Runner::new_from_namerena_raw("aaaaa\nhelp".to_string()).unwrap();
    let runner_ba = Runner::new_from_namerena_raw("help\naaaaa".to_string()).unwrap();

    let mut state_ab = runner_ab
        .world
        .groups
        .iter()
        .flatten()
        .map(|id| runner_ab.storage.get_player(id).expect("player should exist"))
        .map(|plr| (plr.id_name(), plr.get_sort_int(), plr.move_point()))
        .collect::<Vec<(String, i32, i32)>>();
    state_ab.sort_by(|a, b| a.0.cmp(&b.0));

    let mut state_ba = runner_ba
        .world
        .groups
        .iter()
        .flatten()
        .map(|id| runner_ba.storage.get_player(id).expect("player should exist"))
        .map(|plr| (plr.id_name(), plr.get_sort_int(), plr.move_point()))
        .collect::<Vec<(String, i32, i32)>>();
    state_ba.sort_by(|a, b| a.0.cmp(&b.0));

    assert_eq!(state_ab, state_ba);
}

#[test]
fn charm_state_redirects_target_group() {
    let runner = Runner::new_from_namerena_raw("a\nc\n\nb".to_string()).unwrap();
    let actor = runner.world.groups[0][0];
    let ally = runner.world.groups[0][1];
    let enemy = runner.world.groups[1][0];
    runner
        .storage
        .just_get_player_mut(actor)
        .expect("cannot get actor")
        .set_state(tswn_core::player::skill::charm::CharmState {
            group_id: enemy,
            effective_team_idx: None,
            source_team_idx: None,
            target: Some(actor),
            on_post_action: None,
            step: 2,
        });

    let targets = tick::select_targets(actor, &runner.world, &runner.storage);
    assert!(targets.enemy_alive.contains(&ally));
    assert!(!targets.enemy_alive.contains(&enemy));
}

#[test]
fn charm_state_prefers_effective_team_idx_for_target_group() {
    let runner = Runner::new_from_namerena_raw("a\nc\n\nb\nd\n\ne\nf".to_string()).unwrap();
    let actor = runner.world.groups[0][0];
    let actor_ally = runner.world.groups[0][1];
    let enemy = runner.world.groups[1][0];
    let third = runner.world.groups[2][0];
    let third_ally = runner.world.groups[2][1];

    runner.storage.just_get_player_mut(actor).expect("cannot get actor").set_state(
        tswn_core::player::skill::act::charm::CharmState {
            group_id: enemy,
            effective_team_idx: Some(2),
            source_team_idx: Some(2),
            target: Some(actor),
            on_post_action: None,
            step: 2,
        },
    );

    let targets = tick::select_targets(actor, &runner.world, &runner.storage);
    assert!(!targets.ally_alive.contains(&actor));
    assert!(!targets.ally_alive.contains(&actor_ally));
    assert!(targets.ally_alive.contains(&third));
    assert!(targets.ally_alive.contains(&third_ally));
}

#[test]
fn runtime_spawn_queue_syncs_into_world_group() {
    let mut runner = Runner::new_from_namerena_raw("owner\n\nenemy".to_string()).unwrap();
    let owner = runner.world.groups[0][0];
    let mut minion = Player::new_from_namerena_raw("owner?minion".to_string(), runner.storage.clone()).unwrap();
    minion.set_state(MinionRuntimeState {
        owner: Some(owner),
        kind: MinionKind::Clone,
        share_damage_owner: None,
    });
    let minion_id = minion.as_ptr();
    runner.storage.queue_spawn(owner, minion);

    let mut updates = RunUpdates::new();
    runner.round_tick(&mut updates);
    assert!(runner.world.groups[0].contains(&minion_id));
}

#[test]
fn runtime_remove_queue_syncs_world_and_storage() {
    let mut runner = Runner::new_from_namerena_raw("owner\n\nenemy".to_string()).unwrap();
    let enemy = runner.world.groups[1][0];
    runner.storage.just_get_player_mut(enemy).unwrap().set_hp_raw(0);
    runner.storage.record_death(enemy);
    runner.storage.queue_remove_player(enemy);

    let mut updates = RunUpdates::new();
    runner.round_tick(&mut updates);
    assert!(runner.world.groups[1].contains(&enemy));
    assert!(runner.world.team_roster(1).unwrap().contains(&enemy));
    assert!(!runner.world.team_alive(1).unwrap().contains(&enemy));
    assert!(!runner.world.alives_flat(&runner.storage).contains(&enemy));
    assert!(runner.storage.group_containing(enemy).unwrap().contains(&enemy));
    assert!(runner.storage.alive_group_containing(enemy).is_none());
    assert!(!runner.storage.all_alive_ids().contains(&enemy));
    assert_eq!(runner.storage.alive_group_count(), 1);
    assert!(runner.storage.get_player(&enemy).is_some());
}

#[test]
fn runtime_remove_queue_does_not_remove_still_alive_player() {
    let mut runner = Runner::new_from_namerena_raw("owner\n\nenemy".to_string()).unwrap();
    let enemy = runner.world.groups[1][0];
    runner.storage.queue_remove_player(enemy);

    let mut updates = RunUpdates::new();
    runner.round_tick(&mut updates);

    assert!(runner.world.groups[1].contains(&enemy));
    assert!(runner.world.team_roster(1).unwrap().contains(&enemy));
    assert!(runner.world.team_alive(1).unwrap().contains(&enemy));
    assert!(runner.world.alives_flat(&runner.storage).contains(&enemy));
    assert!(runner.storage.group_containing(enemy).unwrap().contains(&enemy));
    assert!(runner.storage.alive_group_containing(enemy).unwrap().contains(&enemy));
    assert!(runner.storage.all_alive_ids().contains(&enemy));
    assert_eq!(runner.storage.alive_group_count(), 2);
    assert!(runner.storage.get_player(&enemy).is_some());
}

#[test]
fn select_targets_ignores_dead_enemy_shadow_left_in_world_alive_view() {
    let mut runner = Runner::new_from_namerena_raw("actor\n\nenemy".to_string()).unwrap();
    let actor = runner.world.groups[0][0];
    let enemy = runner.world.groups[1][0];

    let mut shadow = Player::new_from_namerena_raw("enemy_shadow".to_string(), runner.storage.clone()).unwrap();
    shadow.revive_with_hp(1);
    shadow.set_state(MinionRuntimeState {
        owner: Some(enemy),
        kind: MinionKind::Shadow,
        share_damage_owner: None,
    });
    let shadow_id = runner.storage.just_insert_player(shadow);

    runner.world.add_new_player(shadow_id, enemy);
    runner.storage.sync_groups(&runner.world.groups);
    runner.storage.sync_alive_groups_owned(runner.world.alives_by_group(&runner.storage));

    assert!(runner.world.team_alive(1).unwrap().contains(&shadow_id));
    assert!(runner.storage.get_player(&shadow_id).unwrap().alive());

    runner.storage.just_get_player_mut(shadow_id).expect("cannot get shadow").set_hp_raw(0);

    assert!(runner.world.team_alive(1).unwrap().contains(&shadow_id));
    assert!(!runner.storage.get_player(&shadow_id).unwrap().alive());

    let targets = tick::select_targets(actor, &runner.world, &runner.storage);
    assert!(targets.enemy_alive.contains(&enemy));
    assert!(!targets.enemy_alive.contains(&shadow_id));
    assert!(!targets.all_alive.contains(&shadow_id));
}

#[test]
fn world_remove_player_keeps_roster_but_prunes_alive_view() {
    let runner = Runner::new_from_namerena_raw("owner\n\nenemy".to_string()).unwrap();
    let enemy = runner.world.groups[1][0];
    let mut world = runner.world.clone();

    world.remove_player(enemy);

    assert!(world.groups[1].contains(&enemy));
    assert!(world.team_roster(1).unwrap().contains(&enemy));
    assert!(!world.team_alive(1).unwrap().contains(&enemy));
    assert!(world.all_plrs().contains(&enemy));
    assert!(!world.alives_flat(&runner.storage).contains(&enemy));
}

#[test]
fn world_remove_from_roster_prunes_roster_and_alive_view() {
    let runner = Runner::new_from_namerena_raw("owner\n\nenemy".to_string()).unwrap();
    let enemy = runner.world.groups[1][0];
    let mut world = runner.world.clone();

    world.remove_from_roster(enemy);

    assert!(!world.groups[1].contains(&enemy));
    assert!(!world.team_roster(1).unwrap().contains(&enemy));
    assert!(!world.team_alive(1).unwrap().contains(&enemy));
    assert!(!world.all_plrs().contains(&enemy));
    assert!(!world.alives_flat(&runner.storage).contains(&enemy));
}

#[test]
fn sync_runtime_entities_revives_before_death_removal_preserves_flat_alive_order() {
    let mut runner = Runner::new_from_namerena_raw("anchor\nrevived\n\nenemy".to_string()).unwrap();
    let anchor = runner.world.groups[0][0];
    let revived = runner.world.groups[0][1];
    let enemy = runner.world.groups[1][0];

    runner.storage.just_get_player_mut(revived).unwrap().set_hp_raw(0);
    runner.world.remove_player(revived);
    assert_eq!(runner.world.alives_flat(&runner.storage), vec![anchor, enemy]);

    runner.storage.just_get_player_mut(revived).unwrap().revive_with_hp(1);
    runner.storage.queue_revival(revived);
    runner.storage.just_get_player_mut(anchor).unwrap().set_hp_raw(0);
    runner.storage.record_death(anchor);

    EngineCore::default().sync_runtime_entities(&mut runner.world, &runner.storage);
    assert_eq!(runner.world.team_alive(0).unwrap(), &[revived]);
    assert_eq!(runner.world.alives_flat(&runner.storage), vec![revived, enemy]);
}

#[test]
fn sync_runtime_entities_applies_revivals_before_spawns_in_round_order() {
    let mut runner = Runner::new_from_namerena_raw("anchor\nrevived\n\nenemy".to_string()).unwrap();
    let anchor = runner.world.groups[0][0];
    let revived = runner.world.groups[0][1];
    let enemy = runner.world.groups[1][0];

    runner.storage.just_get_player_mut(revived).unwrap().set_hp_raw(0);
    runner.world.remove_player(revived);

    let mut minion = Player::new_from_namerena_raw("enemy?spawn".to_string(), runner.storage.clone()).unwrap();
    minion.set_state(MinionRuntimeState {
        owner: Some(enemy),
        kind: MinionKind::Clone,
        share_damage_owner: None,
    });

    runner.storage.just_get_player_mut(revived).unwrap().revive_with_hp(1);
    runner.storage.queue_revival(revived);
    runner.storage.queue_spawn(enemy, minion);

    EngineCore::default().sync_runtime_entities(&mut runner.world, &runner.storage);

    let spawned = runner
        .world
        .players
        .iter()
        .copied()
        .find(|id| *id != anchor && *id != enemy && *id != revived)
        .expect("spawned minion should be present");

    assert_eq!(runner.world.players, vec![anchor, enemy, revived, spawned]);
    assert_eq!(runner.world.team_alive(0).unwrap(), &[anchor, revived]);
    assert_eq!(runner.world.team_alive(1).unwrap(), &[enemy, spawned]);
    assert_eq!(runner.world.alives_flat(&runner.storage), vec![anchor, revived, enemy, spawned]);
}

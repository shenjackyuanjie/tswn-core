use tswn_core::Runner;

fn build_input(target: &str, modifier: &str, round: usize) -> String {
    let base = 33_554_431 + round * 3;
    format!(
        "{target}\n{base}@{modifier}\n\n{}@{modifier}\n{}@{modifier}",
        base + 1,
        base + 2
    )
}

fn main() {
    let modifier = std::env::args().nth(1).unwrap_or_else(|| "\u{0002}".to_string());
    let rounds = std::env::args()
        .nth(2)
        .and_then(|arg| arg.parse::<usize>().ok())
        .unwrap_or(1000);
    let list = std::env::args().any(|arg| arg == "--list");

    let mut team0_wins = 0usize;
    let mut target_any_wins = 0usize;
    let mut target_first_wins = 0usize;
    let mut raw_data = Vec::new();

    for round in 0..rounds {
        let input = build_input("aaaaaa", &modifier, round);
        let mut runner = Runner::new_from_namerena_raw(input).expect("runner should build");
        let target_ids = runner
            .all_plrs()
            .into_iter()
            .filter(|id| {
                runner
                    .storage
                    .get_player(id)
                    .map(|player| player.id_name() == "aaaaaa")
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        let team0_ids = runner.input_groups.first().cloned().unwrap_or_default();

        runner.run_to_completion();

        if let Some(winners) = runner.world.winner.as_ref() {
            if winners.iter().any(|id| team0_ids.contains(id)) {
                team0_wins += 1;
            }
            if winners.iter().any(|id| target_ids.contains(id)) {
                target_any_wins += 1;
            }
            if winners.first().is_some_and(|id| target_ids.contains(id)) {
                target_first_wins += 1;
            }
            if list {
                let winner_names = winners
                    .iter()
                    .filter_map(|id| runner.storage.get_player(id))
                    .map(|player| player.id_key_name())
                    .collect::<Vec<_>>()
                    .join("|");
                let team0_hit = winners.iter().any(|id| team0_ids.contains(id));
                let target_hit = winners.iter().any(|id| target_ids.contains(id));
                println!("{},{},{}", round + 1, team0_hit as u8, winner_names);
                debug_assert_eq!(team0_hit, target_hit);
            }
        }

        if (round + 1) % 100 == 0 {
            raw_data.push((round + 1, team0_wins, target_any_wins, target_first_wins));
        }
    }

    println!("team0_wins={team0_wins}");
    println!("target_any_wins={target_any_wins}");
    println!("target_first_wins={target_first_wins}");
    for (round, team0, target_any, target_first) in raw_data {
        println!("{round},{team0},{target_any},{target_first}");
    }
}

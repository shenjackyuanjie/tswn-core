use std::fmt::Write as _;

use tswn_core::{Runner, engine::PROFILE_START, engine::update::UpdateType, player::eval_name::WIN_RATE_EVAL_RQ};

fn build_score_match_input(target: &str, modifier: &str, round: usize, out: &mut String) {
    out.clear();
    let profile_base = PROFILE_START as usize + round * 3;
    out.push_str(target);
    out.push('\n');
    let _ = write!(out, "{}@{modifier}", profile_base);
    out.push_str("\n\n");
    let _ = write!(out, "{}@{modifier}\n{}@{modifier}", profile_base + 1, profile_base + 2);
}

fn format_update(runner: &Runner, update: &tswn_core::engine::update::RunUpdate) -> String {
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
            .filter_map(|id| runner.storage.get_player(id).map(|plr| plr.display_name()))
            .collect::<Vec<_>>()
            .join(",")
    };
    msg.replace("[2]", &param)
}

fn main() {
    let modifier = std::env::args().nth(1).unwrap_or_else(|| "\u{0002}".to_string());
    let count = std::env::args()
        .nth(2)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1000);
    let detail_round = std::env::args().nth(3).and_then(|s| s.parse::<usize>().ok());
    let start_round = std::env::args()
        .nth(4)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1);

    let mut input = String::new();
    let mut wins = 0usize;
    print!("[");
    for offset in 0..count {
        let round_number = start_round + offset;
        build_score_match_input("aaaaaa", &modifier, round_number - 1, &mut input);
        let (groups, seed) = Runner::split_namerena_into_groups(input.clone());
        let mut runner = Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, WIN_RATE_EVAL_RQ).expect("runner should build");
        let team0_targets: Vec<usize> = runner
            .input_groups
            .first()
            .map(|group| group.iter().take(1).copied().collect())
            .unwrap_or_default();
        if detail_round == Some(round_number) {
            eprintln!("input={input:?}");
            for id in runner.world.all_plrs() {
                let player = runner.storage.get_player(&id).expect("player should exist");
                let st = player.get_status();
                eprintln!(
                    "player={} hp={} atk={} def={} spd={} agi={} mag={} mdf={} wis={} mp={} move={} factor={}",
                    player.id_name(),
                    st.hp,
                    st.attack,
                    st.defense,
                    st.speed,
                    st.agility,
                    st.magic,
                    st.resistance,
                    st.wisdom,
                    st.magic_point,
                    player.move_point(),
                    player.get_name_factor()
                );
            }
        }
        if detail_round == Some(round_number) {
            let mut guard = 0usize;
            while !runner.have_winner() && guard < 100_000 {
                let updates = runner.main_round();
                for update in updates.updates {
                    if matches!(update.update_type, UpdateType::NextLine) {
                        continue;
                    }
                    eprintln!("update={}", format_update(&runner, &update));
                }
                guard += 1;
            }
        } else {
            runner.run_to_completion();
        }
        if detail_round == Some(round_number) {
            let winners = runner
                .world
                .winner
                .as_ref()
                .map(|ids| {
                    ids.iter()
                        .filter_map(|id| runner.storage.get_player(id).map(|p| p.id_name()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            eprintln!("winner={winners:?}");
        }
        if let Some(ref winners) = runner.world.winner
            && winners.first().is_some_and(|winner| team0_targets.contains(winner))
        {
            wins += 1;
        }
        if offset > 0 {
            print!(",");
        }
        print!("{{\"round\":{},\"score\":{}}}", round_number, wins);
    }
    println!("]");
}

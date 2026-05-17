use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tswn_core::engine::update::{RunUpdate, UpdateType};
use tswn_core::player::eval_name::WIN_RATE_EVAL_RQ;
use tswn_core::{Runner, engine, win_rate::groups_win_rate};

use crate::BENCH_PARALLEL_THRESHOLD;

pub fn run(raw: String, out_raw: bool) {
    let mut runner = match Runner::new_from_namerena_raw(raw) {
        Ok(runner) => runner,
        Err(err) => {
            eprintln!("构建对局失败: {err}");
            std::process::exit(1);
        }
    };
    let input_player_ids = collect_input_player_ids(&runner);

    if out_raw {
        print_fight_raw(&mut runner, &input_player_ids);
        return;
    }

    print_all_players(&runner);

    let mut round = 1usize;
    let mut idle_rounds = 0usize;
    let mut total_score = 0u64;
    let mut score_by_caster: HashMap<usize, u64> = HashMap::new();

    while !runner.have_winner() && round <= 100_000 {
        let updates = runner.main_round();
        if updates.updates.is_empty() {
            idle_rounds += 1;
            if idle_rounds > 16 {
                break;
            }
            continue;
        }
        idle_rounds = 0;

        println!("=== 回合 {round} ===");
        for update in updates.updates {
            match update.update_type {
                UpdateType::NextLine => println!(),
                _ => {
                    if update.score > 0 {
                        total_score += update.score as u64;
                        *score_by_caster.entry(update.caster).or_insert(0) += update.score as u64;
                    }
                    println!("{}", fmt_update(&runner, &update));
                }
            }
        }
        round += 1;
    }

    println!("\n=== 对局结果 ===");
    if let Some(winners) = runner.world.winner.clone() {
        println!("赢家:");
        for winner in winners {
            if let Some(plr) = runner.storage.get_player(&winner) {
                let battle_score = score_by_caster.get(&winner).copied().unwrap_or(0);
                println!(
                    "- {} (id={}, all_sum={}, battle_score={}, hp={})",
                    plr.display_name(),
                    winner,
                    plr.get_status().all_sum,
                    battle_score,
                    plr.get_status().hp
                );
            }
        }
    } else {
        println!("未分出胜负（达到安全轮次或连续空更新）。");
    }
    println!("总战斗分: {total_score}");
    if let Some(win_idx_line) = fmt_winner_input_indices(&runner, &input_player_ids) {
        println!("{win_idx_line}");
    }
}

pub fn run_diff(raw: String) {
    let mut runner = match Runner::new_from_namerena_raw(raw) {
        Ok(runner) => runner,
        Err(err) => {
            eprintln!("构建对局失败: {err}");
            std::process::exit(1);
        }
    };

    let (lines, _guard, _total_score) = collect_diff_lines(&mut runner, 20_000, true);
    if !lines.is_empty() {
        println!("{}", lines.join("\n"));
    }
}

pub fn run_raw(raw: String, n: usize, threads: Option<usize>) {
    let trimmed = raw.trim().to_string();
    if trimmed.is_empty() {
        eprintln!("raw: 输入为空或无有效玩家");
        return;
    }

    if !starts_with_raw_bench_header(&trimmed) {
        let mut runner = match Runner::new_from_namerena_raw(trimmed) {
            Ok(runner) => runner,
            Err(err) => {
                eprintln!("构建对局失败: {err}");
                std::process::exit(1);
            }
        };
        let input_player_ids = collect_input_player_ids(&runner);
        print_fight_raw(&mut runner, &input_player_ids);
        return;
    }

    let body = strip_raw_bench_header(&trimmed).trim().to_string();
    if body.is_empty() {
        eprintln!("raw: !test! 之后未提供有效输入");
        return;
    }

    let (groups, _) = Runner::split_namerena_into_groups(body.clone());
    let group_count = groups.iter().filter(|g| !g.is_empty()).count();
    match group_count {
        0 => eprintln!("raw: !test! 之后未提供有效输入"),
        1 => run_raw_score(body, n, threads),
        2 => run_raw_winrate(body, n, threads),
        _ => eprintln!("raw: !test! 模式只支持 1 组（评分）或 2 组（胜率）输入"),
    }
}

fn run_raw_score(raw: String, n: usize, threads: Option<usize>) {
    let (groups, _) = Runner::split_namerena_into_groups(raw.clone());
    let target_group = groups.into_iter().next().unwrap_or_default();
    let target_count = target_group.len();
    if target_count == 0 {
        eprintln!("评分: 无目标玩家");
        return;
    }

    println!("=== 原始 namerena 评分测试 ({n} 场) ===");
    println!("目标: {}", target_group.join(", "));
    println!("info: {target_count}");

    print!("[普通评分] ");
    let normal = run_raw_score_inner(&target_group, "\u{0002}", n, threads);
    let ns = normal.0 as f64 * 10_000.0 / normal.1.max(1) as f64;
    println!("普通评分: {:.0} / 10000  ({}/{})", ns, normal.0, normal.1);

    print!("[!评分]    ");
    let bang = run_raw_score_inner(&target_group, "!", n, threads);
    let bs = bang.0 as f64 * 10_000.0 / bang.1.max(1) as f64;
    println!("!评分:     {:.0} / 10000  ({}/{})", bs, bang.0, bang.1);
}

fn js_score_targets_per_round(target_group: &[String]) -> usize {
    if target_group.len() == 2 && target_group[0] == target_group[1] {
        1
    } else {
        target_group.len()
    }
}

fn js_score_profiles_per_round(target_group: &[String]) -> usize {
    if target_group.len() == 2 && target_group[0] == target_group[1] {
        1
    } else if target_group.len() == 1 {
        3
    } else {
        target_group.len()
    }
}

fn build_js_score_match_input(target_group: &[String], modifier: &str, round: usize, bench_input: &mut String) {
    bench_input.clear();

    let tracked_targets = js_score_targets_per_round(target_group);
    let profile_count = js_score_profiles_per_round(target_group);
    let profile_base = engine::PROFILE_START as usize + round * profile_count;

    if target_group.len() == 1 {
        bench_input.push_str(&target_group[0]);
        bench_input.push('\n');
        let _ = std::fmt::Write::write_fmt(bench_input, format_args!("{}@{modifier}", profile_base));
        bench_input.push_str("\n\n");
        let _ = std::fmt::Write::write_fmt(
            bench_input,
            format_args!("{}@{modifier}\n{}@{modifier}", profile_base + 1, profile_base + 2),
        );
        return;
    }

    for (idx, name) in target_group.iter().take(tracked_targets).enumerate() {
        if idx > 0 {
            bench_input.push('\n');
        }
        bench_input.push_str(name);
    }
    bench_input.push_str("\n\n");
    for offset in 0..profile_count {
        if offset > 0 {
            bench_input.push('\n');
        }
        let _ = std::fmt::Write::write_fmt(bench_input, format_args!("{}@{modifier}", profile_base + offset));
    }
}

fn run_raw_score_inner(target_group: &[String], modifier: &str, n: usize, threads: Option<usize>) -> (usize, usize) {
    let workers = resolve_raw_workers(threads, n);

    if workers <= 1 || n < BENCH_PARALLEL_THRESHOLD {
        return run_raw_score_range(target_group, modifier, 0, n, true);
    }

    let next = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let target_group = target_group.to_vec();
        let modifier = modifier.to_string();
        let next = Arc::clone(&next);
        handles.push(std::thread::spawn(move || {
            run_raw_score_worker(&target_group, modifier.as_str(), next.as_ref(), n)
        }));
    }

    let mut wins = 0usize;
    let mut total = 0usize;
    for handle in handles {
        let (part_wins, part_total) = handle.join().expect("raw score worker thread panicked");
        wins += part_wins;
        total += part_total;
    }
    (wins, total)
}

fn run_raw_score_range(target_group: &[String], modifier: &str, start: usize, end: usize, show_progress: bool) -> (usize, usize) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut progress_printed = false;
    let tracked_targets = js_score_targets_per_round(target_group);
    let mut bench_input = String::with_capacity(target_group.iter().map(|name| name.len() + 1).sum::<usize>() + 96);

    for i in start..end {
        build_js_score_match_input(target_group, modifier, i, &mut bench_input);

        let (groups, seed) = Runner::split_namerena_into_groups(bench_input.clone());
        let mut runner = match Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, WIN_RATE_EVAL_RQ) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let team0_targets: Vec<usize> = runner
            .input_groups
            .first()
            .map(|group| group.iter().take(tracked_targets).copied().collect())
            .unwrap_or_default();

        runner.run_to_completion();
        total += 1;
        if let Some(ref winners) = runner.world.winner
            && winners.first().is_some_and(|winner| team0_targets.contains(winner))
        {
            wins += 1;
        }
        if show_progress && (i + 1) % 100 == 0 {
            print!("\r  进度: {}/{}  ", i + 1, end);
            progress_printed = true;
        }
    }
    if progress_printed {
        println!();
    }
    (wins, total)
}

fn run_raw_score_worker(target_group: &[String], modifier: &str, next: &AtomicUsize, end: usize) -> (usize, usize) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let tracked_targets = js_score_targets_per_round(target_group);
    let mut bench_input = String::with_capacity(target_group.iter().map(|name| name.len() + 1).sum::<usize>() + 96);

    loop {
        let i = next.fetch_add(1, Ordering::Relaxed);
        if i >= end {
            break;
        }

        build_js_score_match_input(target_group, modifier, i, &mut bench_input);

        let (groups, seed) = Runner::split_namerena_into_groups(bench_input.clone());
        let mut runner = match Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, WIN_RATE_EVAL_RQ) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let team0_targets: Vec<usize> = runner
            .input_groups
            .first()
            .map(|group| group.iter().take(tracked_targets).copied().collect())
            .unwrap_or_default();

        runner.run_to_completion();
        total += 1;
        if let Some(ref winners) = runner.world.winner
            && winners.first().is_some_and(|winner| team0_targets.contains(winner))
        {
            wins += 1;
        }
    }

    (wins, total)
}

fn resolve_raw_workers(threads: Option<usize>, total: usize) -> usize {
    threads
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|x| x.get().saturating_mul(5).div_ceil(4))
                .unwrap_or(1)
        })
        .min(total.max(1))
}

fn starts_with_raw_bench_header(raw: &str) -> bool {
    let raw = raw.trim_start_matches('\u{feff}');
    raw.strip_prefix("!test!")
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
}

fn strip_raw_bench_header(raw: &str) -> &str {
    let raw = raw.trim_start_matches('\u{feff}');
    raw.strip_prefix("!test!").unwrap_or(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_bench_header_detects_plain_header() {
        assert!(starts_with_raw_bench_header("!test!\n\nmario"));
    }

    #[test]
    fn raw_bench_header_detects_leading_blank_lines() {
        let trimmed = "\n\n!test!\n\nmario".trim().to_string();
        assert!(starts_with_raw_bench_header(&trimmed));
        assert_eq!(strip_raw_bench_header(&trimmed).trim(), "mario");
    }

    #[test]
    fn raw_bench_header_detects_bom_prefixed_header() {
        assert!(starts_with_raw_bench_header("\u{feff}!test!\n\nmario"));
    }

    #[test]
    fn raw_bench_header_rejects_non_header_inputs() {
        assert!(!starts_with_raw_bench_header("mario\n\nluigi"));
        assert!(!starts_with_raw_bench_header("!test!mario"));
    }

    #[test]
    fn strip_raw_bench_header_keeps_bench_body() {
        assert_eq!(strip_raw_bench_header("!test!\n\nmario\n\nluigi").trim(), "mario\n\nluigi");
    }

    #[test]
    fn trimmed_leading_blank_line_input_routes_to_bench_group_count() {
        let trimmed = "\n!test!\n\nmario\n\nluigi".trim().to_string();
        assert!(starts_with_raw_bench_header(&trimmed));

        let body = strip_raw_bench_header(&trimmed).trim().to_string();
        let (groups, _) = Runner::split_namerena_into_groups(body);
        let group_count = groups.iter().filter(|g| !g.is_empty()).count();

        assert_eq!(group_count, 2);
    }

    #[test]
    fn raw_score_single_target_builds_js_match_shape() {
        let single = ["aaaaa".to_string()];
        let mut bench_input = String::new();
        build_js_score_match_input(&single, "!", 0, &mut bench_input);
        assert_eq!(js_score_targets_per_round(&single), 1);
        assert_eq!(js_score_profiles_per_round(&single), 3);
        assert_eq!(bench_input, "aaaaa\n33554431@!\n\n33554432@!\n33554433@!");
    }

    #[test]
    fn winner_input_indices_follow_original_input_order() {
        let raw = "Italian_Love #5Agn8kVYl@Shabby_fish\n我会回来的 #yTneTj00J@Shabby_fish\n\nH6PeQOTNUlx@tyakasha\nOrbital #sfPTzSpZz@tyakasha\nseed:33554434@!";
        let mut runner = Runner::new_from_namerena_raw(raw.to_string()).expect("runner should build");
        let input_player_ids = collect_input_player_ids(&runner);

        runner.run_to_completion();

        let win_idx = fmt_winner_input_indices(&runner, &input_player_ids).expect("winner indices should exist");
        let indices = win_idx
            .strip_prefix("win_idx=")
            .expect("win_idx prefix should exist")
            .split(',')
            .filter(|part| !part.is_empty())
            .map(|part| part.parse::<usize>().expect("winner index should be an integer"))
            .collect::<Vec<_>>();

        assert!(!indices.is_empty());
        assert!(indices.iter().all(|idx| *idx < 2), "expected team0 winners, got {indices:?}");
    }
}

fn run_raw_winrate(raw: String, n: usize, threads: Option<usize>) {
    println!("=== 原始 namerena 胜率测试 ({n} 场) ===");
    let summary = run_raw_winrate_inner(&raw, n, threads);
    let rate = summary.0 as f64 * 100.0 / summary.1.max(1) as f64;
    println!("胜率: {:.2}%  ({}/{})", rate, summary.0, summary.1);
}

fn run_raw_winrate_inner(raw: &str, n: usize, threads: Option<usize>) -> (usize, usize) {
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    let thread = threads.and_then(|x| u32::try_from(x).ok()).unwrap_or(0);
    match groups_win_rate(&groups, n, tswn_core::player::eval_name::WIN_RATE_EVAL_RQ, thread) {
        Ok(summary) => (summary.wins, summary.total),
        Err(err) => {
            eprintln!("构建胜率模板失败: {err}");
            (0, 0)
        }
    }
}

fn collect_input_player_ids(runner: &Runner) -> Vec<usize> {
    runner.input_groups.iter().flat_map(|group| group.iter().copied()).collect()
}

fn fmt_winner_input_indices(runner: &Runner, input_player_ids: &[usize]) -> Option<String> {
    let winners = runner.world.winner.as_ref()?;
    let indices = winners
        .iter()
        .filter_map(|winner| input_player_ids.iter().position(|id| id == winner))
        .map(|idx| idx.to_string())
        .collect::<Vec<String>>();
    if indices.is_empty() {
        None
    } else {
        Some(format!("win_idx={}", indices.join(",")))
    }
}

fn print_all_players(runner: &Runner) {
    println!("=== 玩家状态 ===");
    let player_ids = runner.storage.all_player_ids();
    for id in player_ids {
        if let Some(plr) = runner.storage.get_player(&id) {
            let status = plr.get_status();
            println!(
                "- {} (id={}): HP={}/{}, move_point:{} ATK={}, DEF={}, SPD={}, AGI={}, MAG={}, MP={}, MDF={}, ITL={}, all_sum={} 系数: {}",
                plr.display_name(),
                id,
                status.hp,
                status.max_hp,
                status.move_point,
                status.attack,
                status.defense,
                status.speed,
                status.agility,
                status.magic,
                status.magic_point,
                status.resistance,
                status.wisdom,
                status.all_sum,
                plr.get_name_factor()
            );
        }
    }
    println!();
}

#[derive(Default)]
struct TraceNameState {
    assigned: HashMap<usize, String>,
    next_index: HashMap<usize, usize>,
    summon_name: HashMap<usize, String>,
}

fn root_trace_owner_id(storage: &engine::storage::Storage, start_id: usize) -> usize {
    use tswn_core::player::skill::act::minion::MinionRuntimeState;

    let mut current = start_id;
    loop {
        let Some(plr) = storage.get_player_or_pending(&current) else {
            return current;
        };
        let Some(minion) = plr.get_state::<MinionRuntimeState>() else {
            return current;
        };
        let Some(owner) = minion.owner else {
            return current;
        };
        current = owner;
    }
}

fn format_trace_minion_name(owner: &tswn_core::player::Player, index: usize) -> String {
    let base = format!("{}?{}", owner.id_name(), index);
    let team = owner.clan_name();
    if !team.is_empty() && team != owner.id_name() {
        format!("{base}@{team}")
    } else {
        base
    }
}

fn alloc_trace_minion_name(trace_names: &mut TraceNameState, root_owner_id: usize, owner: &tswn_core::player::Player) -> String {
    let index = trace_names.next_index.entry(root_owner_id).or_insert(0);
    let name = format_trace_minion_name(owner, *index);
    *index += 1;
    name
}

fn plr_name(runner: &Runner, id: usize) -> String {
    runner
        .storage
        .get_player_or_pending(&id)
        .map(|plr| plr.id_key_name())
        .unwrap_or_else(|| format!("#{id}"))
}

fn fmt_update_with_mode(runner: &Runner, update: &RunUpdate, append_score: bool) -> String {
    let caster = plr_name(runner, update.caster);
    let target = plr_name(runner, update.target);
    let targets = if let Some(p) = update.param {
        p.to_string()
    } else if update.targets.is_empty() {
        update.score.to_string()
    } else {
        update.targets.iter().map(|id| plr_name(runner, *id)).collect::<Vec<String>>().join(",")
    };

    let mut msg = update.message.to_string();
    msg = msg.replace("[0]", &caster);
    msg = msg.replace("[1]", &target);
    msg = msg.replace("[2]", &targets);
    if append_score && update.score > 0 {
        format!("{msg}  (+{})", update.score)
    } else {
        msg
    }
}

fn fmt_update(runner: &Runner, update: &RunUpdate) -> String { fmt_update_with_mode(runner, update, true) }

fn fmt_update_diff(runner: &Runner, update: &RunUpdate) -> String {
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

fn plr_name_raw(runner: &Runner, id: usize, trace_names: &mut TraceNameState) -> String {
    if let Some(name) = trace_names.assigned.get(&id) {
        return name.clone();
    }

    let Some(plr) = runner.storage.get_player_or_pending(&id) else {
        return format!("#{id}");
    };

    use tswn_core::player::PlayerType;
    use tswn_core::player::skill::act::minion::MinionRuntimeState;

    let name = if plr.player_type() == PlayerType::Boss {
        plr.display_name()
    } else if let Some(minion) = plr.get_state::<MinionRuntimeState>() {
        if let Some(owner_id) = minion.owner {
            let root_owner_id = root_trace_owner_id(&runner.storage, owner_id);
            if let Some(owner) = runner.storage.get_player_or_pending(&root_owner_id) {
                use tswn_core::player::skill::act::minion::MinionKind;
                if minion.kind == MinionKind::Summon {
                    if let Some(name) = trace_names.summon_name.get(&owner_id) {
                        name.clone()
                    } else {
                        let name = alloc_trace_minion_name(trace_names, root_owner_id, owner);
                        trace_names.summon_name.insert(owner_id, name.clone());
                        name
                    }
                } else {
                    alloc_trace_minion_name(trace_names, root_owner_id, owner)
                }
            } else {
                plr.id_key_name()
            }
        } else {
            plr.id_key_name()
        }
    } else {
        plr.id_key_name()
    };

    trace_names.assigned.insert(id, name.clone());
    name
}

fn fmt_update_raw_with_state(runner: &Runner, update: &RunUpdate, trace_names: &mut TraceNameState) -> String {
    let caster = plr_name_raw(runner, update.caster, trace_names);
    let mut target = plr_name_raw(runner, update.target, trace_names);
    let targets = if let Some(p) = update.param {
        p.to_string()
    } else if update.targets.is_empty() {
        update.score.to_string()
    } else {
        update
            .targets
            .iter()
            .map(|id| plr_name_raw(runner, *id, trace_names))
            .collect::<Vec<String>>()
            .join(",")
    };

    if update.message == "召唤出幻影" {
        use tswn_core::player::skill::act::minion::{MinionKind, MinionRuntimeState};

        let root_owner_id = root_trace_owner_id(&runner.storage, update.caster);
        let pending_id = runner
            .storage
            .all_player_ids()
            .into_iter()
            .chain(runner.storage.pending_spawn_ids_for_owner(update.caster))
            .find(|id| {
                !trace_names.assigned.contains_key(id)
                    && runner
                        .storage
                        .get_player_or_pending(id)
                        .and_then(|plr| plr.get_state::<MinionRuntimeState>())
                        .map(|state| {
                            state.kind == MinionKind::Shadow
                                && root_trace_owner_id(&runner.storage, state.owner.unwrap_or(*id)) == root_owner_id
                        })
                        .unwrap_or(false)
            });
        if let Some(pending_id) = pending_id {
            target = plr_name_raw(runner, pending_id, trace_names);
        }
        return format!("召唤出{target}");
    }

    let mut msg = update.message.to_string();
    msg = msg.replace("[0]", &caster);
    msg = msg.replace("[1]", &target);
    msg.replace("[2]", &targets)
}

fn sanitize_output_line(line: &str) -> String {
    let filtered = line
        .chars()
        .filter(|ch| !ch.is_control() && !matches!(*ch, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}'))
        .collect::<String>();

    let mut normalized = String::with_capacity(filtered.len());
    let mut prev_space = false;
    for ch in filtered.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                normalized.push(' ');
                prev_space = true;
            }
        } else {
            normalized.push(ch);
            prev_space = false;
        }
    }
    normalized.trim().to_string()
}

fn normalize_trace_line(line: String) -> String {
    let mut normalized = line
        .replace("[s_counter]", "")
        .replace("[s_dmg160]", "")
        .replace("[s_dmg120]", "")
        .replace("[s_dmg0]", "")
        .replace(' ', "")
        .replace('！', "!")
        .replace('？', "?")
        .replace('，', ",")
        .replace('：', ":")
        .replace('；', ";")
        .replace('（', "(")
        .replace('）', ")")
        .replace('²', "2");

    for (from, to) in [
        ("[回避]", "回避"),
        ("[反击]", "反击"),
        ("[吸血攻击]", "吸血攻击"),
        ("[聚气]", "聚气"),
        ("[潜行]", "潜行"),
        ("[背刺]", "背刺"),
        ("[狂暴攻击]", "狂暴攻击"),
        ("[狂暴术]", "狂暴术"),
        ("[狂暴]", "狂暴"),
        ("[蓄力]", "蓄力"),
        ("[隐匿]", "隐匿"),
        ("[魅惑]", "魅惑"),
        ("[防御]", "防御"),
        ("[吞噬]", "吞噬"),
        ("[分身]", "分身"),
        ("[会心一击]", "会心一击"),
        ("[伤害反弹]", "伤害反弹"),
        ("[净化]", "净化"),
        ("[护身符]", "护身符"),
        ("[诅咒]", "诅咒"),
        ("[守护]", "守护"),
        ("[生命之轮]", "生命之轮"),
        ("[垂死]", "垂死"),
        ("[火球术]", "火球术"),
        ("[瘟疫]", "瘟疫"),
        ("[加速术]", "加速术"),
        ("[疾走]", "疾走"),
        ("[治愈魔法]", "治愈魔法"),
        ("[迟缓]", "迟缓"),
        ("[中毒]", "中毒"),
        ("[冰冻术]", "冰冻术"),
        ("[冰冻]", "冰冻"),
        ("[铁壁]", "铁壁"),
        ("[投毒]", "投毒"),
        ("[毒性发作]", "毒性发作"),
        ("[附体]", "附体"),
        ("[地裂术]", "地裂术"),
        ("[连击]", "连击"),
        ("[苏生术]", "苏生术"),
        ("[复活]", "复活"),
        ("[幻术]", "幻术"),
        ("[减速术]", "减速术"),
        ("[雷击术]", "雷击术"),
        ("[血祭]", "血祭"),
        ("[召唤亡灵]", "召唤亡灵"),
        ("[自爆]", "自爆"),
    ] {
        normalized = normalized.replace(from, to);
    }

    sanitize_output_line(&normalized)
}

fn normalize_diff_trace_line(line: String) -> String {
    line.replace("[s_counter]", "")
        .replace("[s_dmg160]", "")
        .replace("[s_dmg120]", "")
        .replace("[s_dmg0]", "")
        .replace(['[', ']'], "")
        .replace(' ', "")
        .trim()
        .to_string()
}

fn collect_diff_lines(runner: &mut Runner, max_rounds: usize, normalize: bool) -> (Vec<String>, usize, u64) {
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
            let mut msg = fmt_update_diff(runner, &update);
            if normalize {
                msg = normalize_diff_trace_line(msg);
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

fn is_action_line(line: &str) -> bool {
    line.contains("发起攻击")
        || (line.contains("使用") && !line.contains("护身符抵挡了一次死亡"))
        || line.contains("做出垂死抗争")
        || line.contains("连击")
        || line.contains("从疾走中解除")
}

fn emit_current_turn(output_lines: &mut Vec<String>, pending_action_line: &mut String, pending_misc_lines: &mut Vec<String>) {
    if !pending_action_line.is_empty() {
        output_lines.push(std::mem::take(pending_action_line));
        output_lines.push(String::new());
        pending_misc_lines.clear();
        return;
    }
    if !pending_misc_lines.is_empty() {
        output_lines.push(pending_misc_lines.join(", "));
        output_lines.push(String::new());
        pending_misc_lines.clear();
    }
}

fn print_fight_raw(runner: &mut Runner, input_player_ids: &[usize]) {
    let mut output_lines: Vec<String> = Vec::new();
    let mut pending_action_line = String::new();
    let mut pending_misc_lines: Vec<String> = Vec::new();
    let mut trace_names = TraceNameState::default();
    let debug_raw_seq = std::env::var_os("TSWN_DEBUG_RAW_SEQ").is_some();
    let mut raw_seq_idx = 0usize;

    let mut round = 1usize;
    let mut idle_rounds = 0usize;
    while !runner.have_winner() && round <= 100_000 {
        let updates = runner.main_round();
        if updates.updates.is_empty() {
            idle_rounds += 1;
            if idle_rounds > 16 {
                break;
            }
            continue;
        }
        idle_rounds = 0;

        for update in updates.updates {
            if matches!(update.update_type, UpdateType::NextLine) {
                if debug_raw_seq {
                    eprintln!("[raw_seq/{raw_seq_idx}] <NextLine>");
                    raw_seq_idx += 1;
                }
                emit_current_turn(&mut output_lines, &mut pending_action_line, &mut pending_misc_lines);
                continue;
            }

            let line = normalize_trace_line(fmt_update_raw_with_state(runner, &update, &mut trace_names));
            if line.is_empty() {
                continue;
            }

            if debug_raw_seq {
                eprintln!("[raw_seq/{raw_seq_idx}] {line}");
                raw_seq_idx += 1;
            }

            if is_action_line(&line) {
                emit_current_turn(&mut output_lines, &mut pending_action_line, &mut pending_misc_lines);
                pending_action_line = line;
                continue;
            }

            if pending_action_line.is_empty() {
                pending_misc_lines.push(line);
            } else {
                pending_action_line.push_str(", ");
                pending_action_line.push_str(&line);
            }
        }
        round += 1;
    }

    emit_current_turn(&mut output_lines, &mut pending_action_line, &mut pending_misc_lines);
    while matches!(output_lines.last(), Some(line) if line.is_empty()) {
        output_lines.pop();
    }

    if !output_lines.is_empty() {
        println!("{}", output_lines.join("\n"));
    }
    if let Some(win_idx_line) = fmt_winner_input_indices(runner, input_player_ids) {
        println!("{win_idx_line}");
    }
}

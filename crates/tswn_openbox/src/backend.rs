use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tswn_core::engine::storage::Storage;
use tswn_core::player::{Player, eval_name::WIN_RATE_EVAL_RQ, overlay::PlayerOverlay};
use tswn_core::win_rate::{WinRateTiming, prepared_win_rate, resolve_win_rate_workers};
use tswn_core::{PreparedRunner, Runner};

const BENCH_PARALLEL_THRESHOLD: usize = 100;

#[derive(Debug, Clone)]
pub enum ProgressEvent {
    Log(String),
    Progress { done: usize, total: usize },
    Done(Result<String, String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Log,
    Jsonl,
    Pure,
}

#[derive(Debug, Clone)]
pub struct CommonBenchOptions {
    pub count: usize,
    pub threads: Option<usize>,
    pub keep_rq: bool,
    pub verbose: bool,
    pub min_screen: Option<f64>,
    pub min_file: Option<f64>,
    pub wr_precision: usize,
}

#[derive(Debug, Clone)]
pub struct BatchRateInput {
    pub target_text: String,
    pub player_text: String,
    pub player_double_plus: bool,
    pub output_mode: OutputMode,
    pub output_file: PathBuf,
    pub options: CommonBenchOptions,
}

#[derive(Debug, Clone)]
pub struct PairInput {
    pub target_text: String,
    pub player_text: String,
    pub teammate_text: String,
    pub head: usize,
    pub output_mode: OutputMode,
    pub output_file: PathBuf,
    pub options: CommonBenchOptions,
}

#[derive(Debug, Clone)]
struct BenchSummary {
    wins: usize,
    total: usize,
    timing: WinRateTiming,
}

impl BenchSummary {
    fn win_rate_percent(&self) -> f64 { self.wins as f64 * 100.0 / self.total.max(1) as f64 }
}

#[derive(Debug, Clone)]
struct BatchRateSummary {
    avg: f64,
    aggregate_rate: f64,
    wins: usize,
    total: usize,
    timing: WinRateTiming,
    elapsed: Duration,
    valid_matchups: usize,
    skipped_matchups: usize,
}

impl BatchRateSummary {
    fn throughput(&self) -> f64 {
        let secs = self.elapsed.as_secs_f64();
        if secs > 0.0 { self.total as f64 / secs } else { 0.0 }
    }
}

pub fn run_to_diy(raw: &str, old: bool, details: bool) -> Result<String, String> {
    let names = parse_line_list(raw);
    if names.is_empty() {
        return Err("请输入至少一个名字。".to_string());
    }

    let storage = Storage::new_arc();
    let mut out = String::new();
    for name in &names {
        let mut player =
            Player::new_from_namerena_raw(name.clone(), storage.clone()).map_err(|err| format!("构建玩家失败: {name}: {err}"))?;
        player.build();
        let export = if old { player.to_diy_compact() } else { player.to_ol_json() };
        let _ = writeln!(out, "{export}");

        if details && names.len() == 1 {
            let status = player.get_status();
            let _ = writeln!(out);
            let _ = writeln!(out, "=== 原始信息 ===");
            let _ = writeln!(out, "名字: {}", player.id_name());
            let _ = writeln!(out, "队伍: {}", player.clan_name());
            let _ = writeln!(
                out,
                "八围: atk={} def={} spd={} agi={} mag={} res={} wis={} maxhp={}",
                status.attack,
                status.defense,
                status.speed,
                status.agility,
                status.magic,
                status.resistance,
                status.wisdom,
                status.max_hp
            );
            let _ = writeln!(out, "name_factor: {:.6}", player.get_name_factor());
        }
    }
    Ok(out)
}

pub fn run_namer_pf(raw: &str, n: usize, threads: Option<usize>, send: impl Fn(ProgressEvent)) {
    let groups = parse_namer_pf_groups(raw);
    if groups.is_empty() {
        send(ProgressEvent::Done(Err("namer-pf: 输入为空或无有效玩家。".to_string())));
        return;
    }

    let mut out = String::from("pp|pd|qp|qd|sum\n");
    let total = groups.len();
    for (index, group) in groups.iter().enumerate() {
        let pp = namer_pf_score(group, "\u{0002}", false, n, threads);
        let pd = namer_pf_score(group, "\u{0002}", true, n, threads);
        let qp = namer_pf_score(group, "!", false, n, threads);
        let qd = namer_pf_score(group, "!", true, n, threads);
        let sum = pp + pd + qp + qd;
        let _ = writeln!(out, "{pp}|{pd}|{qp}|{qd}|{sum}");
        send(ProgressEvent::Progress { done: index + 1, total });
    }
    send(ProgressEvent::Done(Ok(out)));
}

pub fn run_batch_rate(input: BatchRateInput, send: impl Fn(ProgressEvent)) {
    let target_groups = parse_plus_separated_groups(&input.target_text);
    let (player_groups, player_labels) = parse_player_groups_with_labels(&input.player_text, input.player_double_plus);
    if target_groups.is_empty() {
        send(ProgressEvent::Done(Err("batch-rate: 靶子列表为空。".to_string())));
        return;
    }
    if player_groups.is_empty() {
        send(ProgressEvent::Done(Err("batch-rate: 选手列表为空。".to_string())));
        return;
    }

    let mut output = match create_output_file(&input.output_file) {
        Ok(file) => file,
        Err(err) => {
            send(ProgressEvent::Done(Err(err)));
            return;
        }
    };

    let n = input.options.count.max(1);
    let eval_rq = eval_rq(input.options.keep_rq);
    let precision = input.options.wr_precision.min(9);
    let total = player_groups.len() * target_groups.len();
    let mut done = 0usize;

    for (index, (player, label)) in player_groups.iter().zip(player_labels.iter()).enumerate() {
        let mut verbose = String::new();
        let summary = bench_batch_rate_for_group(
            player,
            &target_groups,
            n,
            input.options.threads,
            eval_rq,
            input.options.verbose,
            &mut verbose,
            |_, _| {
                done += 1;
                send(ProgressEvent::Progress { done, total });
            },
        );

        if input.options.min_file.is_none_or(|limit| summary.avg >= limit) {
            let line = format_batch_file_record(input.output_mode, label, &summary, precision);
            if let Err(err) = writeln!(output, "{line}") {
                send(ProgressEvent::Done(Err(format!("写入输出文件失败: {err}"))));
                return;
            }
        }

        if input.options.min_screen.is_none_or(|limit| summary.avg >= limit) {
            let log = format_batch_screen_log(
                index + 1,
                player_groups.len(),
                label,
                &summary,
                precision,
                input.options.verbose,
                &verbose,
            );
            send(ProgressEvent::Log(log));
        }
    }

    send(ProgressEvent::Done(Ok(format!(
        "完成，结果已写入: {}",
        input.output_file.display()
    ))));
}

pub fn run_pair(input: PairInput, send: impl Fn(ProgressEvent)) {
    let target_groups = parse_plus_separated_groups(&input.target_text);
    let players = parse_line_list(&input.player_text);
    let teammates = parse_line_list(&input.teammate_text);
    if target_groups.is_empty() {
        send(ProgressEvent::Done(Err("pair: 靶子列表为空。".to_string())));
        return;
    }
    if players.is_empty() {
        send(ProgressEvent::Done(Err("pair: 选手列表为空。".to_string())));
        return;
    }
    if teammates.is_empty() {
        send(ProgressEvent::Done(Err("pair: 队友列表为空。".to_string())));
        return;
    }

    let mut output = match create_output_file(&input.output_file) {
        Ok(file) => file,
        Err(err) => {
            send(ProgressEvent::Done(Err(err)));
            return;
        }
    };

    let n = input.options.count.max(1);
    let head = input.head.max(1);
    let eval_rq = eval_rq(input.options.keep_rq);
    let precision = input.options.wr_precision.min(9);
    let total = players.len() * teammates.len() * target_groups.len();
    let mut done = 0usize;

    for (index, player) in players.iter().enumerate() {
        let started = Instant::now();
        let converted_player = match player_to_ol(player) {
            Ok(value) => value,
            Err(err) => {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
        };
        let mut pair_rates = Vec::with_capacity(teammates.len());
        let mut total_wins = 0usize;
        let mut total_battles = 0usize;
        let mut total_valid_matchups = 0usize;
        let mut total_skipped_matchups = 0usize;
        let mut total_timing = WinRateTiming::default();
        let mut verbose = String::new();

        for teammate in &teammates {
            let pair_group = format!("{converted_player}\n{teammate}");
            if input.options.verbose {
                let _ = writeln!(verbose, "teammate: {teammate}");
            }
            let summary = bench_batch_rate_for_group(
                &pair_group,
                &target_groups,
                n,
                input.options.threads,
                eval_rq,
                input.options.verbose,
                &mut verbose,
                |_, _| {
                    done += 1;
                    send(ProgressEvent::Progress { done, total });
                },
            );
            if summary.valid_matchups > 0 {
                pair_rates.push((summary.avg, teammate.clone()));
            }
            total_wins += summary.wins;
            total_battles += summary.total;
            total_valid_matchups += summary.valid_matchups;
            total_skipped_matchups += summary.skipped_matchups;
            total_timing.merge(summary.timing);
        }

        pair_rates.sort_by(|a, b| b.0.total_cmp(&a.0));
        let selected_count = head.min(pair_rates.len());
        let final_score = pair_rates.iter().take(selected_count).map(|(rate, _)| *rate).sum::<f64>();
        let elapsed = started.elapsed();

        if input.options.min_file.is_none_or(|limit| final_score >= limit) {
            let line = format_pair_file_record(
                input.output_mode,
                player,
                final_score,
                selected_count,
                head,
                &pair_rates,
                total_wins,
                total_battles,
                total_valid_matchups,
                total_skipped_matchups,
                elapsed,
                precision,
            );
            if let Err(err) = writeln!(output, "{line}") {
                send(ProgressEvent::Done(Err(format!("写入输出文件失败: {err}"))));
                return;
            }
        }

        if input.options.min_screen.is_none_or(|limit| final_score >= limit) {
            let log = format_pair_screen_log(
                index + 1,
                players.len(),
                player,
                final_score,
                selected_count,
                head,
                &pair_rates,
                total_wins,
                total_battles,
                total_valid_matchups,
                total_skipped_matchups,
                elapsed,
                precision,
                input.options.verbose,
                &verbose,
            );
            send(ProgressEvent::Log(log));
        }

        let _ = total_timing;
    }

    send(ProgressEvent::Done(Ok(format!(
        "完成，结果已写入: {}",
        input.output_file.display()
    ))));
}

fn create_output_file(path: &PathBuf) -> Result<File, String> {
    if path.as_os_str().is_empty() {
        return Err("请先选择输出文件。".to_string());
    }
    if path.exists() && path.is_dir() {
        return Err(format!("输出路径不能是目录: {}", path.display()));
    }
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty())
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|err| format!("创建输出目录失败: {}: {err}", parent.display()))?;
    }
    File::create(path).map_err(|err| format!("打开输出文件失败: {}: {err}", path.display()))
}

fn eval_rq(keep_rq: bool) -> f64 {
    if keep_rq {
        tswn_core::player::eval_name::DEFAULT_EVAL_RQ
    } else {
        WIN_RATE_EVAL_RQ
    }
}

fn bench_winrate_summary(raw: &str, n: usize, threads: Option<usize>, eval_rq: f64) -> Result<BenchSummary, String> {
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    let prepared = Runner::prepare_groups_with_eval_rq(&groups, eval_rq).map_err(|err| format!("{err}"))?;
    bench_prepared_summary(&prepared, n, threads, eval_rq)
}

fn bench_prepared_summary(
    prepared: &PreparedRunner,
    n: usize,
    threads: Option<usize>,
    eval_rq: f64,
) -> Result<BenchSummary, String> {
    let thread = threads.and_then(|x| u32::try_from(x).ok()).unwrap_or(0);
    let summary = prepared_win_rate(prepared, n, eval_rq, thread).map_err(|err| format!("{err}"))?;
    Ok(BenchSummary {
        wins: summary.wins,
        total: summary.total,
        timing: summary.timing,
    })
}

fn bench_batch_rate_for_group(
    player: &str,
    target_groups: &[String],
    n: usize,
    threads: Option<usize>,
    eval_rq: f64,
    verbose: bool,
    verbose_buf: &mut String,
    mut tick_target: impl FnMut(usize, &str),
) -> BatchRateSummary {
    let started = Instant::now();
    let mut accumulated_rate = 0.0;
    let mut accumulated_wins = 0usize;
    let mut accumulated_total = 0usize;
    let mut accumulated_timing = WinRateTiming::default();
    let mut valid_matchups = 0usize;
    let mut skipped_matchups = 0usize;

    for (index, target) in target_groups.iter().enumerate() {
        if let Some(duplicate) = first_duplicate_name_in_matchup(&[player, target.as_str()]) {
            skipped_matchups += 1;
            if verbose {
                let _ = writeln!(
                    verbose_buf,
                    "  [{}/{}] vs {} => SKIP duplicate name: {}",
                    index + 1,
                    target_groups.len(),
                    display_group(target),
                    duplicate
                );
            }
            tick_target(index, target);
            continue;
        }

        let raw = format!("{player}\n\n{target}");
        match bench_winrate_summary(&raw, n, threads, eval_rq) {
            Ok(summary) => {
                if verbose {
                    let _ = writeln!(
                        verbose_buf,
                        "  [{}/{}] vs {} => {:.2}% ({}/{})",
                        index + 1,
                        target_groups.len(),
                        display_group(target),
                        summary.win_rate_percent(),
                        summary.wins,
                        summary.total
                    );
                }
                accumulated_rate += summary.win_rate_percent();
                accumulated_wins += summary.wins;
                accumulated_total += summary.total;
                accumulated_timing.merge(summary.timing);
                valid_matchups += 1;
            }
            Err(err) => {
                skipped_matchups += 1;
                if verbose {
                    let _ = writeln!(
                        verbose_buf,
                        "  [{}/{}] vs {} => ERROR: {err}",
                        index + 1,
                        target_groups.len(),
                        display_group(target)
                    );
                }
            }
        }
        tick_target(index, target);
    }

    let avg = if valid_matchups > 0 {
        accumulated_rate / valid_matchups as f64
    } else {
        0.0
    };
    let aggregate_rate = accumulated_wins as f64 * 100.0 / accumulated_total.max(1) as f64;
    BatchRateSummary {
        avg,
        aggregate_rate,
        wins: accumulated_wins,
        total: accumulated_total,
        timing: accumulated_timing,
        elapsed: started.elapsed(),
        valid_matchups,
        skipped_matchups,
    }
}

fn namer_pf_score(base_group: &[String], modifier: &str, duplicate: bool, n: usize, threads: Option<usize>) -> u64 {
    let mut target_group = base_group.to_vec();
    if duplicate {
        target_group.extend(base_group.iter().cloned());
    }
    let summary = run_bench_score_inner(&target_group, modifier, n, threads);
    (summary.wins as f64 * 10_000.0 / summary.total.max(1) as f64).round() as u64
}

fn run_bench_score_inner(target_group: &[String], modifier: &str, n: usize, threads: Option<usize>) -> BenchSummary {
    let workers = resolve_win_rate_workers(threads.and_then(|x| u32::try_from(x).ok()).unwrap_or(0), n);
    let (wins, total, timing) = if workers <= 1 || n < BENCH_PARALLEL_THRESHOLD {
        run_bench_score_range(target_group, modifier, 0, n)
    } else {
        let next = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            let target_group = target_group.to_vec();
            let modifier = modifier.to_string();
            let next = Arc::clone(&next);
            handles.push(std::thread::spawn(move || {
                run_bench_score_worker(&target_group, &modifier, &next, n)
            }));
        }
        let mut wins = 0usize;
        let mut total = 0usize;
        let mut timing = WinRateTiming::default();
        for handle in handles {
            let (part_wins, part_total, part_timing) = handle.join().expect("score worker thread panicked");
            wins += part_wins;
            total += part_total;
            timing.merge(part_timing);
        }
        (wins, total, timing)
    };
    BenchSummary { wins, total, timing }
}

fn run_bench_score_range(target_group: &[String], modifier: &str, start: usize, end: usize) -> (usize, usize, WinRateTiming) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut timing = WinRateTiming::default();
    let mut bench_input = String::with_capacity(target_group.iter().map(|name| name.len() + 1).sum::<usize>() + 96);

    for i in start..end {
        build_js_score_match_input(target_group, modifier, i, &mut bench_input);
        let t_init = Instant::now();
        let (groups, seed) = Runner::split_namerena_into_groups(bench_input.clone());
        let mut runner = match Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, WIN_RATE_EVAL_RQ) {
            Ok(runner) => runner,
            Err(_) => continue,
        };
        let target_team: Vec<usize> = runner.input_groups.first().map(|group| group.to_vec()).unwrap_or_default();
        timing.init_nanos += t_init.elapsed().as_nanos();
        let t_fight = Instant::now();
        runner.run_to_completion();
        timing.fight_nanos += t_fight.elapsed().as_nanos();
        total += 1;
        if let Some(winners) = runner.world.winner.as_ref()
            && winners.first().is_some_and(|winner| target_team.contains(winner))
        {
            wins += 1;
        }
    }
    (wins, total, timing)
}

fn run_bench_score_worker(
    target_group: &[String],
    modifier: &str,
    next: &AtomicUsize,
    end: usize,
) -> (usize, usize, WinRateTiming) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut timing = WinRateTiming::default();
    let mut bench_input = String::with_capacity(target_group.iter().map(|name| name.len() + 1).sum::<usize>() + 96);

    loop {
        let i = next.fetch_add(1, Ordering::Relaxed);
        if i >= end {
            break;
        }
        build_js_score_match_input(target_group, modifier, i, &mut bench_input);
        let t_init = Instant::now();
        let (groups, seed) = Runner::split_namerena_into_groups(bench_input.clone());
        let mut runner = match Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, WIN_RATE_EVAL_RQ) {
            Ok(runner) => runner,
            Err(_) => continue,
        };
        let target_team: Vec<usize> = runner.input_groups.first().map(|group| group.to_vec()).unwrap_or_default();
        timing.init_nanos += t_init.elapsed().as_nanos();
        let t_fight = Instant::now();
        runner.run_to_completion();
        timing.fight_nanos += t_fight.elapsed().as_nanos();
        total += 1;
        if let Some(winners) = runner.world.winner.as_ref()
            && winners.first().is_some_and(|winner| target_team.contains(winner))
        {
            wins += 1;
        }
    }
    (wins, total, timing)
}

fn build_js_score_match_input(target_group: &[String], modifier: &str, round: usize, out: &mut String) {
    out.clear();
    let tracked_targets = js_score_targets_per_round(target_group);
    let profile_count = js_score_profiles_per_round(target_group);
    let profile_base = tswn_core::engine::PROFILE_START as usize + round * profile_count;

    if target_group.len() == 1 {
        out.push_str(&target_group[0]);
        out.push('\n');
        let _ = write!(out, "{}@{modifier}", profile_base);
        out.push_str("\n\n");
        let _ = write!(out, "{}@{modifier}\n{}@{modifier}", profile_base + 1, profile_base + 2);
        return;
    }

    for (index, name) in target_group.iter().take(tracked_targets).enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(name);
    }
    out.push_str("\n\n");
    for offset in 0..profile_count {
        if offset > 0 {
            out.push('\n');
        }
        let _ = write!(out, "{}@{modifier}", profile_base + offset);
    }
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

fn parse_line_list(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_plus_separated_groups(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.split('+').map(str::trim).collect::<Vec<_>>().join("\n"))
        .collect()
}

fn parse_player_groups_with_labels(content: &str, double_plus: bool) -> (Vec<String>, Vec<String>) {
    let mut groups = Vec::new();
    let mut labels = Vec::new();
    for line in content.lines().map(str::trim).filter(|line| !line.is_empty()) {
        labels.push(line.to_string());
        let separator = if double_plus { "++" } else { "+" };
        groups.push(line.split(separator).map(str::trim).collect::<Vec<_>>().join("\n"));
    }
    (groups, labels)
}

fn parse_namer_pf_groups(raw: &str) -> Vec<Vec<String>> {
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_namer_pf_group_line)
        .filter(|group| !group.is_empty())
        .collect()
}

fn parse_namer_pf_group_line(line: &str) -> Vec<String> {
    let mut group: Vec<String> = Vec::new();
    for segment in split_plus_outside_quotes(line) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        if PlayerOverlay::parse_inline(segment).is_some()
            && let Some(previous) = group.last_mut()
        {
            previous.push('+');
            previous.push_str(segment);
            continue;
        }
        group.push(segment.to_string());
    }
    group
}

fn split_plus_outside_quotes(raw: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;
    for ch in raw.chars() {
        if in_string {
            current.push(ch);
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
        } else if ch == '+' {
            segments.push(std::mem::take(&mut current));
        } else {
            current.push(ch);
            if ch == '"' {
                in_string = true;
            }
        }
    }
    segments.push(current);
    segments
}

fn first_duplicate_name_in_matchup(groups: &[&str]) -> Option<String> {
    let mut seen = HashSet::new();
    for group in groups {
        for name in group.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let id_name = Player::raw_namerena_to_idname(name);
            if !seen.insert(id_name.clone()) {
                return Some(id_name);
            }
        }
    }
    None
}

fn player_to_ol(raw: &str) -> Result<String, String> {
    if raw.contains("+diy[") || raw.contains("+ol:") {
        return Ok(raw.to_string());
    }
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw(raw.to_string(), storage)
        .map_err(|err| format!("转换 player-list 名字为 +ol 失败: {raw}: {err}"))?;
    player.build();
    Ok(player.to_ol_json())
}

fn format_batch_file_record(mode: OutputMode, label: &str, summary: &BatchRateSummary, precision: usize) -> String {
    match mode {
        OutputMode::Log => format!("{} {label}", format_rate(summary.avg, precision)),
        OutputMode::Pure => label.to_string(),
        OutputMode::Jsonl => format!(
            "{{\"label\":\"{}\",\"avg_win_rate\":{},\"aggregate_win_rate\":{},\"wins\":{},\"total\":{},\"valid_matchups\":{},\"skipped_matchups\":{},\"elapsed_s\":{:.3},\"us_per_battle\":{:.1},\"battles_per_s\":{:.0}}}",
            escape_json_string(label),
            format_rate(summary.avg, precision),
            format_rate(summary.aggregate_rate, precision),
            summary.wins,
            summary.total,
            summary.valid_matchups,
            summary.skipped_matchups,
            summary.elapsed.as_secs_f64(),
            summary.elapsed.as_micros() as f64 / summary.total.max(1) as f64,
            summary.throughput()
        ),
    }
}

fn format_batch_screen_log(
    index: usize,
    total_players: usize,
    label: &str,
    summary: &BatchRateSummary,
    precision: usize,
    verbose: bool,
    verbose_text: &str,
) -> String {
    let mut out = String::new();
    if verbose {
        let _ = writeln!(out, "{verbose_text}");
    }
    let _ = writeln!(
        out,
        "[{index}/{total_players}] {label}\t平均胜率: {}%\t汇总: {}% ({}/{})\t有效: {}\t跳过重复: {}\t用时: {:.3}s",
        format_rate(summary.avg, precision),
        format_rate(summary.aggregate_rate, precision),
        summary.wins,
        summary.total,
        summary.valid_matchups,
        summary.skipped_matchups,
        summary.elapsed.as_secs_f64()
    );
    out
}

#[allow(clippy::too_many_arguments)]
fn format_pair_file_record(
    mode: OutputMode,
    label: &str,
    final_score: f64,
    selected_count: usize,
    head: usize,
    pair_rates: &[(f64, String)],
    total_wins: usize,
    total_battles: usize,
    valid_matchups: usize,
    skipped_matchups: usize,
    elapsed: Duration,
    precision: usize,
) -> String {
    match mode {
        OutputMode::Log => format!("{} {label}", format_rate(final_score, precision)),
        OutputMode::Pure => label.to_string(),
        OutputMode::Jsonl => {
            let top_pairs = pair_rates
                .iter()
                .take(selected_count)
                .map(|(rate, teammate)| {
                    format!(
                        "{{\"teammate\":\"{}\",\"batch_rate\":{}}}",
                        escape_json_string(teammate),
                        format_rate(*rate, precision)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            let aggregate = total_wins as f64 * 100.0 / total_battles.max(1) as f64;
            let throughput = if elapsed.as_secs_f64() > 0.0 {
                total_battles as f64 / elapsed.as_secs_f64()
            } else {
                0.0
            };
            format!(
                "{{\"label\":\"{}\",\"score\":{},\"head\":{},\"selected\":{},\"top_pairs\":[{}],\"aggregate_win_rate\":{},\"wins\":{},\"total\":{},\"valid_matchups\":{},\"skipped_matchups\":{},\"elapsed_s\":{:.3},\"battles_per_s\":{:.0}}}",
                escape_json_string(label),
                format_rate(final_score, precision),
                head,
                selected_count,
                top_pairs,
                format_rate(aggregate, precision),
                total_wins,
                total_battles,
                valid_matchups,
                skipped_matchups,
                elapsed.as_secs_f64(),
                throughput
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn format_pair_screen_log(
    index: usize,
    total_players: usize,
    label: &str,
    final_score: f64,
    selected_count: usize,
    head: usize,
    pair_rates: &[(f64, String)],
    total_wins: usize,
    total_battles: usize,
    valid_matchups: usize,
    skipped_matchups: usize,
    elapsed: Duration,
    precision: usize,
    verbose: bool,
    verbose_text: &str,
) -> String {
    let mut out = String::new();
    if verbose {
        let _ = writeln!(out, "{verbose_text}");
    }
    let _ = writeln!(
        out,
        "[{index}/{total_players}] {label}\t最终分数: {}\ttop: {selected_count}/{head}",
        format_rate(final_score, precision)
    );
    for (rank, (rate, teammate)) in pair_rates.iter().take(selected_count).enumerate() {
        let _ = writeln!(out, "  #{} {}% {}", rank + 1, format_rate(*rate, precision), teammate);
    }
    let aggregate = total_wins as f64 * 100.0 / total_battles.max(1) as f64;
    let _ = writeln!(
        out,
        "  汇总胜率: {}% ({}/{})  有效靶子: {}  跳过重复: {}  用时: {:.3}s",
        format_rate(aggregate, precision),
        total_wins,
        total_battles,
        valid_matchups,
        skipped_matchups,
        elapsed.as_secs_f64()
    );
    out
}

fn display_group(raw: &str) -> String {
    raw.lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>().join(", ")
}

fn format_rate(value: f64, precision: usize) -> String {
    let value = if value.abs() < 0.5_f64 * 10_f64.powi(-(precision as i32)) {
        0.0
    } else {
        value
    };
    format!("{value:.precision$}")
}

fn escape_json_string(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

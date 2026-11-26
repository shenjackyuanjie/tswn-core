use std::collections::HashSet;

use crate::win_rate::WinRateTiming;

use super::{BatchSummary, CliApiResult, PairRateEntry, PairRateResult};

/// 对基础组（duplicate 时复制为双人同名组）运行评分并折算为百分制分数。
pub fn namer_pf_score(
    base_group: &[String],
    modifier: &str,
    duplicate: bool,
    n: usize,
    thread: u32,
    eval_rq: f64,
) -> CliApiResult<f64> {
    let mut target_group = base_group.to_vec();
    if duplicate {
        target_group.extend(base_group.iter().cloned());
    }

    crate::runtime::runtime_score(&target_group, modifier, n, eval_rq, thread)
        .map(|summary| summary.score_10000())
        .map_err(super::runtime_batch_error)
}

/// 对单个选手遍历全部靶子组累计批量胜率汇总；带权时按因子加权，跳过无效对阵。
pub fn batch_rate_for_group(
    player: &str,
    target_groups: &[String],
    target_factors: Option<&[f64]>,
    n: usize,
    thread: u32,
    eval_rq: f64,
) -> CliApiResult<BatchSummary> {
    let mut accumulated_rate = 0.0;
    let mut accumulated_wins = 0usize;
    let mut accumulated_total = 0usize;
    let mut accumulated_timing = WinRateTiming::default();
    let mut valid_matchups = 0usize;
    let mut skipped_matchups = 0usize;
    let mut accumulated_factor = 0.0;
    let factored = target_factors.is_some();

    for (index, target) in target_groups.iter().enumerate() {
        let factor = target_factors.and_then(|factors| factors.get(index)).copied().unwrap_or(1.0);
        if factored && groups_have_same_players(player, target) {
            accumulated_rate += 50.0 * factor;
            accumulated_factor += factor;
            accumulated_wins += 1;
            accumulated_total += 2;
            valid_matchups += 1;
            continue;
        }
        if !factored && first_duplicate_name_in_matchup(&[player, target.as_str()]).is_some() {
            skipped_matchups += 1;
            continue;
        }

        let raw = format!("{player}\n\n{target}");
        let summary = super::win_rate_summary(&raw, n, Some(eval_rq), thread)?;
        accumulated_rate += summary.win_rate * factor;
        accumulated_factor += factor;
        accumulated_wins += summary.wins;
        accumulated_total += summary.total;
        accumulated_timing.merge(WinRateTiming {
            init_nanos: summary.init_nanos,
            fight_nanos: summary.fight_nanos,
        });
        valid_matchups += 1;
    }

    let avg = if accumulated_factor > 0.0 {
        accumulated_rate / accumulated_factor
    } else {
        0.0
    };
    let aggregate_rate = accumulated_wins as f64 * 100.0 / accumulated_total.max(1) as f64;
    Ok(BatchSummary {
        avg,
        aggregate_rate,
        wins: accumulated_wins,
        total: accumulated_total,
        timing: accumulated_timing,
        valid_matchups,
        skipped_matchups,
    })
}

/// 对单个选手与各队友组合累计 pair 胜率，并按乘权分数取前 head 名求和。
pub fn pair_rate_for_player(
    player: &str,
    target_groups: &[String],
    target_factors: Option<&[f64]>,
    teammates: &[String],
    head: usize,
    n: usize,
    thread: u32,
    eval_rq: f64,
) -> CliApiResult<PairRateResult> {
    let converted_player = group_to_ol(player)?;
    let mut pair_rates = Vec::with_capacity(teammates.len());
    let mut total_wins = 0usize;
    let mut total_battles = 0usize;
    let mut total_valid_matchups = 0usize;
    let mut total_skipped_matchups = 0usize;
    let mut total_timing = WinRateTiming::default();

    for teammate in teammates {
        // 队友同样冻结成单独构建：只冻结选手会让同公会队友单方面吃到组队加成。
        let converted_teammate = group_to_ol(teammate)?;
        let pair_group = format!("{converted_player}\n{converted_teammate}");
        let summary = batch_rate_for_group(&pair_group, target_groups, target_factors, n, thread, eval_rq)?;
        if summary.valid_matchups > 0 {
            pair_rates.push(PairRateEntry {
                name: teammate.clone(),
                rate: summary.avg,
            });
        }
        total_wins += summary.wins;
        total_battles += summary.total;
        total_valid_matchups += summary.valid_matchups;
        total_skipped_matchups += summary.skipped_matchups;
        total_timing.merge(summary.timing);
    }

    pair_rates.sort_by(|a, b| b.rate.total_cmp(&a.rate));
    let selected = head.min(pair_rates.len());
    let final_score = pair_rates.iter().take(selected).map(|pair| pair.rate).sum::<f64>();
    let aggregate_win_rate = total_wins as f64 * 100.0 / total_battles.max(1) as f64;

    Ok(PairRateResult {
        label: player.to_string(),
        final_score,
        head,
        selected,
        top_pairs: pair_rates.into_iter().take(selected).collect(),
        aggregate_win_rate,
        wins: total_wins,
        total: total_battles,
        valid_matchups: total_valid_matchups,
        skipped_matchups: total_skipped_matchups,
        init_nanos: total_timing.init_nanos,
        fight_nanos: total_timing.fight_nanos,
    })
}

/// 返回对阵中第一个重复出现的归一化玩家名。
fn first_duplicate_name_in_matchup(groups: &[&str]) -> Option<String> {
    let mut seen = HashSet::new();
    for group in groups {
        for name in group.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let id_name = crate::namerena::raw_namerena_to_id_name(name);
            if !seen.insert(id_name.clone()) {
                return Some(id_name);
            }
        }
    }
    None
}

/// 判断两组文本的玩家集合是否相同：归一化后排序比较。
fn groups_have_same_players(left: &str, right: &str) -> bool {
    let mut left = group_player_ids(left);
    let mut right = group_player_ids(right);
    left.sort_unstable();
    right.sort_unstable();
    left == right
}

/// 把组文本转换为归一化玩家 id 列表。
fn group_player_ids(group: &str) -> Vec<String> {
    group
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(crate::namerena::raw_namerena_to_id_name)
        .collect()
}

/// 选手与队友共用：一组输入里的每个成员单独导出成 `+ol`。
///
/// 冻结后的属性与技能来自该成员单独构队的结果，同公会成员之间的组队加成不会再作用
/// 到它身上（`apply_team_upgrades` 只改 `name_base`，overlay 的 `attrs` / `skills` 会直接
/// 覆盖），因此 pair 两侧都保持各自单独构建的强度。已经是 `+diy` / `+ol` 的输入原样保留。
fn member_to_ol(raw: &str) -> CliApiResult<String> {
    if raw.contains("+diy[") || raw.contains("+ol:") {
        return Ok(raw.to_string());
    }
    super::parse::export_player(raw, false, false)
}

/// 把组文本整体转换为 ol 格式多行文本。
fn group_to_ol(group: &str) -> CliApiResult<String> {
    group
        .lines()
        .map(member_to_ol)
        .collect::<CliApiResult<Vec<_>>>()
        .map(|members| members.join("\n"))
}

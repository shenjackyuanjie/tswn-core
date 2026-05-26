//! `fight` / `diff` 的用户入口。
//!
//! 这一层只做两件事：
//! - 负责把用户输入变成 `Runner`；
//! - 负责把对局推进结果按“普通可读输出”或“diff 输出”打印出来。
//!
//! 和 raw benchmark 的分流逻辑、trace 字符串归一化逻辑相比，这里的职责更接近
//! “命令驱动器”，因此单独拆出来，避免后续维护时在大量格式化细节里找入口函数。

use std::collections::HashMap;

use tswn_core::engine::update::UpdateType;
use tswn_core::error::runner::RunnerResult;
use tswn_core::Runner;

use super::trace::{collect_diff_lines, fmt_update, print_fight_raw};

/// 运行普通对战。
///
/// 这里保留两种输出模式：
/// - `out_raw=false` 时打印人类可读的完整回合日志；
/// - `out_raw=true` 时把输出切给 raw trace 格式化器，避免两条路径彼此污染。
pub fn run(raw: String, out_raw: bool) {
    let mut runner = match new_runner_from_raw_for_cli(raw) {
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

/// 运行普通对战并按 runner diff 格式输出。
pub fn run_diff(raw: String) {
    let mut runner = match new_runner_from_raw_for_cli(raw) {
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

/// 为 CLI 构建 `Runner`。
///
/// 这段逻辑单独抽出来，是为了让 `fight` / `diff` / `raw` 三条入口共享同一套
/// “调试环境变量是否强制切到 win-rate rq” 的策略，不要各自维护一份细节分支。
pub(super) fn new_runner_from_raw_for_cli(raw: String) -> RunnerResult<Runner> {
    #[cfg(not(feature = "no_debug"))]
    {
        // 评分路径会把 JS 的全局 `rq` 污染为 6。这个环境变量只给单局复盘用，
        // 方便 raw/fight/diff 用同一套名字强度口径复现评分分叉；`no_debug` 会整段编译掉。
        if std::env::var_os("TSWN_DEBUG_FORCE_WIN_RATE_RQ").is_some() {
            let (groups, seed) = Runner::split_namerena_into_groups(raw.clone());
            return Runner::new_from_groups_with_seed_and_eval_rq(
                &groups,
                &seed,
                tswn_core::player::eval_name::WIN_RATE_EVAL_RQ,
            );
        }
    }

    Runner::new_from_namerena_raw(raw)
}

/// 记录原始输入中的玩家 ID 顺序。
///
/// raw trace 与普通输出最终都会把胜者映射回“输入中的位置”，因此这里保留一个统一 helper。
pub(super) fn collect_input_player_ids(runner: &Runner) -> Vec<usize> {
    runner.input_groups.iter().flat_map(|group| group.iter().copied()).collect()
}

/// 把胜者 ID 转换回输入顺序中的索引，输出 `win_idx=...`。
pub(super) fn fmt_winner_input_indices(runner: &Runner, input_player_ids: &[usize]) -> Option<String> {
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

/// 打印开战前所有玩家的状态快照。
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

#[cfg(test)]
mod tests {
    use super::*;

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
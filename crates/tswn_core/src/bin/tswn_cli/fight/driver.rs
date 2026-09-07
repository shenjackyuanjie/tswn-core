//! `fight` / `diff` 的主 Runtime 入口。

/// 运行普通对战。
pub fn run(raw: String) { super::runtime::run_runtime_fight(raw); }

/// 运行普通对战并按 runner diff 格式输出。
pub fn run_diff(raw: String) { super::runtime::run_runtime_diff(raw); }

/// Runtime 的初始实体索引与原始输入顺序一致；运行期生成的实体不会进入 `win_idx`。
pub(super) fn fmt_runtime_winner_input_indices(
    runner: &tswn_core::runtime::RuntimeRunner,
    input_player_count: usize,
) -> Option<String> {
    let winner_team = runner.runtime().world.winner_team()?;
    let indices = runner
        .runtime()
        .world
        .team_roster(winner_team)?
        .iter()
        .filter_map(|entity| ((entity.0 as usize) < input_player_count).then_some(entity.0.to_string()))
        .collect::<Vec<_>>();
    if indices.is_empty() {
        None
    } else {
        Some(format!("win_idx={}", indices.join(",")))
    }
}

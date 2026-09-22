//! tswn-wasm — tswn-core 的 WebAssembly 绑定。
//!
//! 通过 `wasm-bindgen` 将战斗引擎的核心功能导出为 JavaScript 可调用的 API，
//! 包括单次战斗回放、胜率统计及玩家图标生成。
//!
//! 导出边界统一用 `tsify::Ts<T>` 收发领域类型：参数收 `Option<Ts<T>>`，返回
//! `Ts<T>` / `Vec<Ts<T>>`，并在函数体内经 [`convert`] 转成普通 Rust 类型。这样
//! （反）序列化失败能按稳定错误码抛出，不会像旧的 ABI 边界转换那样 `throw_str`
//! 并泄漏内存；生成的 TypeScript 声明与之前一致。

mod battle;
mod convert;
mod error;
mod fight;
mod model;
mod render;
mod win_rate;

use std::sync::Once;
use tsify::Ts;

pub use battle::BattleSession;
use convert::{ts_in, ts_out, ts_out_vec};
use error::WasmResult;
pub use fight::FightSession;
use model::{
    BattleReplayOptions, CliBatchRateResult, CliGroupWinRateResult, CliIconInfo, CliNamerPfResult, CliPairRateResult,
    CliScoreResult, CliWinRateResult, FightOptions, FightReplay, FightSummary, GroupWinRateResult, RuntimeNormalizedRunView,
    WinRateOptions, WinRateResult,
};
use wasm_bindgen::prelude::*;
pub use win_rate::WinRateSession;

static PANIC_HOOK: Once = Once::new();

pub fn install_panic_hook() { PANIC_HOOK.call_once(console_error_panic_hook::set_once); }

#[wasm_bindgen(start)]
pub fn wasm_start() { install_panic_hook(); }

#[wasm_bindgen]
pub fn version() -> String { env!("CARGO_PKG_VERSION").to_string() }

#[wasm_bindgen]
pub fn core_version() -> String { tswn_core::version().to_string() }

#[wasm_bindgen]
pub fn default_eval_rq() -> f64 { tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ }

#[wasm_bindgen]
pub fn win_rate_eval_rq() -> f64 { tswn_core::namerena::eval_name::WIN_RATE_EVAL_RQ }

#[wasm_bindgen]
pub fn name_to_png_base64(name: String) -> String {
    install_panic_hook();
    tswn_core::namerena::icon_render::render_icon_b64_from_name(&name)
}

#[wasm_bindgen]
pub fn name_to_png_bytes(name: String) -> Vec<u8> {
    install_panic_hook();
    tswn_core::namerena::icon_render::render_icon_png_from_name(&name)
}

#[wasm_bindgen]
pub fn name_to_icon_rgba(name: String) -> Vec<u8> {
    install_panic_hook();
    tswn_core::namerena::icon_render::render_icon_vec_from_name(&name)
}

#[wasm_bindgen]
pub fn fight(raw_input: String, options: Option<Ts<FightOptions>>) -> WasmResult<Ts<FightReplay>> {
    install_panic_hook();
    let options = ts_in(options)?;
    fight::fight_impl(raw_input, options).and_then(|replay| ts_out(&replay))
}

#[wasm_bindgen]
pub fn fight_summary(raw_input: String, options: Option<Ts<FightOptions>>) -> WasmResult<Ts<FightSummary>> {
    install_panic_hook();
    let options = ts_in(options)?;
    fight::fight_summary_impl(raw_input, options).and_then(|summary| ts_out(&summary))
}

#[wasm_bindgen]
pub fn win_rate_sync(
    raw_input: String,
    total_rounds: usize,
    options: Option<Ts<WinRateOptions>>,
) -> WasmResult<Ts<WinRateResult>> {
    install_panic_hook();
    let options = ts_in(options)?;
    win_rate::run_win_rate_sync(raw_input, total_rounds, options).and_then(|result| ts_out(&result))
}

#[wasm_bindgen]
pub fn group_win_rate(
    target: String,
    against: Vec<String>,
    total_rounds: usize,
    options: Option<Ts<WinRateOptions>>,
) -> WasmResult<Vec<Ts<GroupWinRateResult>>> {
    install_panic_hook();
    let options = ts_in(options)?;

    let mut results = Vec::with_capacity(against.len());
    for opponent in against {
        let raw_input = format!("{target}\n\n{opponent}");
        let result = win_rate::run_win_rate_sync(raw_input, total_rounds, options.clone())?;
        results.push(GroupWinRateResult { opponent, result });
    }

    ts_out_vec(&results)
}

#[wasm_bindgen]
pub fn win_rate_summary(
    raw_input: String,
    total_rounds: usize,
    eval_rq: Option<f64>,
    thread: Option<u32>,
) -> WasmResult<Ts<CliWinRateResult>> {
    install_panic_hook();
    tswn_core::cli_api::win_rate_summary(&raw_input, total_rounds, eval_rq, thread.unwrap_or(0))
        .map(Into::into)
        .map_err(error::cli_api_error)
        .and_then(|result| ts_out(&result))
}

#[wasm_bindgen]
pub fn team_win_rate_summary(
    team1: String,
    team2: String,
    total_rounds: usize,
    eval_rq: Option<f64>,
    thread: Option<u32>,
) -> WasmResult<Ts<CliWinRateResult>> {
    install_panic_hook();
    tswn_core::cli_api::team_win_rate_summary(&team1, &team2, total_rounds, eval_rq, thread.unwrap_or(0))
        .map(Into::into)
        .map_err(error::cli_api_error)
        .and_then(|result| ts_out(&result))
}

#[wasm_bindgen]
pub fn group_win_rate_summary(
    target: String,
    against: Vec<String>,
    total_rounds: usize,
    eval_rq: Option<f64>,
    thread: Option<u32>,
) -> WasmResult<Vec<Ts<CliGroupWinRateResult>>> {
    install_panic_hook();
    tswn_core::cli_api::group_win_rate_summary(&target, &against, total_rounds, eval_rq, thread.unwrap_or(0))
        .map(|results| results.into_iter().map(Into::into).collect::<Vec<_>>())
        .map_err(error::cli_api_error)
        .and_then(|results| ts_out_vec(&results))
}

#[wasm_bindgen]
pub fn score(
    raw_input: String,
    total_rounds: usize,
    mode: Option<String>,
    eval_rq: Option<f64>,
    thread: Option<u32>,
) -> WasmResult<Ts<CliScoreResult>> {
    install_panic_hook();
    let mode = mode.unwrap_or_else(|| "normal".to_string());
    tswn_core::cli_api::score(&raw_input, total_rounds, &mode, eval_rq, thread.unwrap_or(0))
        .map(Into::into)
        .map_err(error::cli_api_error)
        .and_then(|result| ts_out(&result))
}

#[wasm_bindgen]
pub fn namer_pf(
    raw_input: String,
    total_rounds: usize,
    modes: Option<Vec<String>>,
    keep_rq: Option<bool>,
    thread: Option<u32>,
) -> WasmResult<Vec<Ts<CliNamerPfResult>>> {
    install_panic_hook();
    tswn_core::cli_api::namer_pf(&raw_input, total_rounds, modes, keep_rq.unwrap_or(false), thread.unwrap_or(0))
        .map(|results| results.into_iter().map(Into::into).collect::<Vec<_>>())
        .map_err(error::cli_api_error)
        .and_then(|results| ts_out_vec(&results))
}

#[wasm_bindgen]
pub fn batch_rate(
    target_groups: Vec<String>,
    player_groups: Vec<String>,
    total_rounds: usize,
    player_labels: Option<Vec<String>>,
    keep_rq: Option<bool>,
    thread: Option<u32>,
) -> WasmResult<Vec<Ts<CliBatchRateResult>>> {
    install_panic_hook();
    tswn_core::cli_api::batch_rate(
        &target_groups,
        &player_groups,
        total_rounds,
        player_labels,
        keep_rq.unwrap_or(false),
        thread.unwrap_or(0),
    )
    .map(|results| results.into_iter().map(Into::into).collect::<Vec<_>>())
    .map_err(error::cli_api_error)
    .and_then(|results| ts_out_vec(&results))
}

#[wasm_bindgen]
pub fn batch_rate_factored(
    target_groups: Vec<String>,
    target_factors: Vec<f64>,
    player_groups: Vec<String>,
    total_rounds: usize,
    player_labels: Option<Vec<String>>,
    keep_rq: Option<bool>,
    thread: Option<u32>,
) -> WasmResult<Vec<Ts<CliBatchRateResult>>> {
    install_panic_hook();
    tswn_core::cli_api::batch_rate_factored(
        &target_groups,
        &target_factors,
        &player_groups,
        total_rounds,
        player_labels,
        keep_rq.unwrap_or(false),
        thread.unwrap_or(0),
    )
    .map(|results| results.into_iter().map(Into::into).collect::<Vec<_>>())
    .map_err(error::cli_api_error)
    .and_then(|results| ts_out_vec(&results))
}

#[wasm_bindgen]
pub fn pair_rate(
    target_groups: Vec<String>,
    players: Vec<String>,
    teammates: Vec<String>,
    head: usize,
    total_rounds: usize,
    keep_rq: Option<bool>,
    thread: Option<u32>,
) -> WasmResult<Vec<Ts<CliPairRateResult>>> {
    install_panic_hook();
    tswn_core::cli_api::pair_rate(
        &target_groups,
        &players,
        &teammates,
        head,
        total_rounds,
        keep_rq.unwrap_or(false),
        thread.unwrap_or(0),
    )
    .map(|results| results.into_iter().map(Into::into).collect::<Vec<_>>())
    .map_err(error::cli_api_error)
    .and_then(|results| ts_out_vec(&results))
}

#[wasm_bindgen]
pub fn pair_rate_factored(
    target_groups: Vec<String>,
    target_factors: Vec<f64>,
    players: Vec<String>,
    teammates: Vec<String>,
    head: usize,
    total_rounds: usize,
    keep_rq: Option<bool>,
    thread: Option<u32>,
) -> WasmResult<Vec<Ts<CliPairRateResult>>> {
    install_panic_hook();
    tswn_core::cli_api::pair_rate_factored(
        &target_groups,
        &target_factors,
        &players,
        &teammates,
        head,
        total_rounds,
        keep_rq.unwrap_or(false),
        thread.unwrap_or(0),
    )
    .map(|results| results.into_iter().map(Into::into).collect::<Vec<_>>())
    .map_err(error::cli_api_error)
    .and_then(|results| ts_out_vec(&results))
}

#[wasm_bindgen]
pub fn default_custom_runtime_normalized_run(raw_input: String, max_rounds: usize) -> WasmResult<Ts<RuntimeNormalizedRunView>> {
    install_panic_hook();
    default_custom_runtime_normalized_run_impl(raw_input, max_rounds).and_then(|run| ts_out(&run))
}

fn default_custom_runtime_normalized_run_impl(raw_input: String, max_rounds: usize) -> WasmResult<RuntimeNormalizedRunView> {
    tswn_core::cli_api::default_custom_runtime_normalized_run(&raw_input, max_rounds)
        .map(tswn_core::cli_api::JsonRuntimeNormalizedRun::from)
        .map(Into::into)
        .map_err(error::cli_api_error)
}

/// 运行完整战斗，并返回共用、可供 UI 使用的回放 JSON 形状。
#[wasm_bindgen(unchecked_return_type = "BattleReplay")]
pub fn battle_replay(raw_input: String, options: Option<Ts<BattleReplayOptions>>) -> WasmResult<JsValue> {
    install_panic_hook();
    let options = ts_in(options)?;
    let replay = tswn_core::cli_api::battle_replay(&raw_input, options.to_core()).map_err(error::cli_api_error)?;
    battle::dto_to_js(&replay)
}

#[wasm_bindgen]
pub fn to_diy(name: String, old: Option<bool>, minions: Option<bool>) -> WasmResult<String> {
    install_panic_hook();
    tswn_core::cli_api::to_diy(&name, old.unwrap_or(false), minions.unwrap_or(false)).map_err(error::cli_api_error)
}

#[wasm_bindgen]
pub fn to_diy_batch(names: Vec<String>, old: Option<bool>, minions: Option<bool>) -> WasmResult<Vec<String>> {
    install_panic_hook();
    tswn_core::cli_api::to_diy_batch(&names, old.unwrap_or(false), minions.unwrap_or(false)).map_err(error::cli_api_error)
}

#[wasm_bindgen]
pub fn icon_info(name: String) -> WasmResult<Ts<CliIconInfo>> {
    install_panic_hook();
    let info = tswn_core::cli_api::icon_info(&name).into();
    ts_out(&info)
}

#[wasm_bindgen]
pub fn parse_group_lines(content: String, double_plus: Option<bool>) -> Vec<String> {
    install_panic_hook();
    tswn_core::cli_api::parse_group_lines(&content, double_plus.unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::UpdateTypeView;

    #[test]
    fn default_custom_runtime_normalized_run_exposes_wasm_view_golden_shape() {
        let run = default_custom_runtime_normalized_run_impl("left@red\n\nright@blue\n".to_string(), 1)
            .expect("default custom runtime normalized run should execute");

        assert_eq!(run.rounds.len(), 1);
        assert_eq!(run.winner_team, None);
        assert!(run.guard_exhausted);
        assert_eq!(run.total_score, 77);

        let round = &run.rounds[0];
        assert_eq!(round.winner_team, None);
        assert_eq!(round.round, 1);
        assert_eq!(round.total_score, 77);
        assert_eq!(round.rng_i, 74);
        assert_eq!(round.rng_j, 92);
        assert_eq!(round.entity_ids, vec![1, 2]);
        assert_eq!(round.teams, vec![0, 1]);
        assert_eq!(round.hp, vec![262, 288]);
        assert_eq!(round.magic_point, vec![23, 16]);
        assert_eq!(round.defense, vec![6, 56]);
        assert_eq!(round.resistance, vec![52, 25]);
        assert_eq!(round.alive, vec![true, true]);
        assert_eq!(round.round_order, vec![0, 1]);
        assert_eq!(round.flat_alive, vec![0, 1]);
        assert_eq!(round.team_alive, vec![vec![0], vec![1]]);
        assert_eq!(round.alive_group_count, 2);

        assert_eq!(round.actions.len(), 1);
        let action = &round.actions[0];
        assert_eq!(action.round, 1);
        assert_eq!(action.actor, 1);
        assert_eq!(action.target, 0);
        assert_eq!(action.amount, 36);

        assert_eq!(round.frames.len(), 3);
        let frame = &round.frames[0];
        assert_eq!(frame.message, "[0]发起攻击");
        assert_eq!(frame.caster, 1);
        assert_eq!(frame.target, 0);
        assert!(frame.targets.is_empty());
        assert_eq!(frame.param, None);
        assert_eq!(frame.score, 0);
        assert_eq!(frame.delay0, 1000);
        assert_eq!(frame.delay1, 100);
        assert_eq!(frame.update_type, UpdateTypeView::None);

        let frame = &round.frames[1];
        assert_eq!(frame.message, "[1]受到[2]点伤害");
        assert_eq!(frame.caster, 1);
        assert_eq!(frame.target, 0);
        assert!(frame.targets.is_empty());
        assert_eq!(frame.param, None);
        assert_eq!(frame.score, 77);
        assert_eq!(frame.delay0, 1154);
        assert_eq!(frame.delay1, 100);
        assert_eq!(frame.update_type, UpdateTypeView::None);

        let frame = &round.frames[2];
        assert_eq!(frame.message, "\n");
        assert_eq!(frame.caster, 0);
        assert_eq!(frame.target, 0);
        assert!(frame.targets.is_empty());
        assert_eq!(frame.param, None);
        assert_eq!(frame.score, 0);
        assert_eq!(frame.delay0, 0);
        assert_eq!(frame.delay1, 0);
        assert_eq!(frame.update_type, UpdateTypeView::NextLine);
    }

    #[test]
    fn default_custom_runtime_normalized_run_rejects_zero_max_rounds() {
        let err = tswn_core::cli_api::default_custom_runtime_normalized_run("left@red\n\nright@blue\n", 0)
            .expect_err("default custom runtime normalized run should reject zero max rounds");
        let err = crate::error::cli_api_tswn_error(err);

        assert_eq!(err.code, "INVALID_INPUT");
        assert_eq!(err.message, "runtime max_rounds must be positive");
    }
}

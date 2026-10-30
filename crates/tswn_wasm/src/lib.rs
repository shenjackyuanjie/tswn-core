//! JavaScript / WebAssembly 接入层与浏览器示例的计算接口。
//!
//! 游戏规则来自 core；本 crate 负责 wasm-bindgen 边界、对象生命周期、DTO 转换及
//! TypeScript 声明。页面主题、播放游标和 DOM 渲染位于 `examples/` 展示层。
//!
//! # 推荐接口
//!
//! | 接口 | 用途 | 执行方式 |
//! | --- | --- | --- |
//! | [`BattleSession`] | 规范的逐帧对局 | 按需 `next_frame`，返回 core DTO |
//! | [`battle_replay`] | 完整回放 | 同步收集整场 |
//! | [`WinRateSession`] | 大批胜率与进度 | 主动分批 step，调用方负责让出事件循环 |
//! | [`win_rate_sync`] | 小批同步胜率 | 调用完成前不让出事件循环 |
//! | [`FightSession`] / [`fight()`] | 兼容旧页面的字段投影 | 保留已有页面使用的接口形状 |
//! | [`name_to_png_bytes`] / [`to_diy`] | 图标及输入格式导出 | 同步 helper |
//!
//! ```javascript
//! import init, { BattleSession } from "./tswn_wasm.js";
//! await init();
//! const session = new BattleSession("left@red\n\nright@blue");
//! try {
//!   console.log(session.initial_states());
//!   for (let frame; (frame = session.next_frame()) !== null;) {
//!     console.log(frame.frame_index);
//!   }
//!   console.log(session.result());
//! } finally {
//!   session.free(); // wasm-bindgen 对象的显式释放
//! }
//! ```
//!
//! 将对象调用放在创建与 `free()` 之间。异常与正常终止分开：错误含稳定 code，Runtime 失败
//! 后会话保持失败标记而不产生正常 result；调用方应停止推进并在 finally 释放。
//! 返回的普通 JS 对象、数组和像素缓冲为独立数据，不需要随会话一并释放；
//! 复制会话变量只复制同一包装对象的引用，free 后不能通过任何别名再次调用。
//! 业务错误通常以 `{code, message}` 普通对象抛出，并非 `Error` 实例；错误对象
//! 序列化异常时退化为 code 字符串。panic hook 仅向控制台报告，不捕获 panic
//! 或将其转换成业务错误，发生 WASM trap 后不应继续推进该对象。
//! 原生 Cargo doc 展示 Rust 适配实现，实际 JS 返回形状以生成的 `.d.ts` 和规范 DTO 为准。
//!
//! # 转换、线程与构建
//!
//! `tsify::Ts<T>` 经内部转换函数显式检查输入/输出；规范 Battle DTO 使用
//! JSON-compatible 的 serde-wasm-bindgen 序列化，按相应类型校验转换。
//! WinRateSession / win_rate_sync 在当前线程同步计算，仅接受 thread=0/1；
//! 不会自行创建 Web Worker。分批 step 之间需由宿主让出事件循环，或把整个计算放进 Worker。
//! 绑定版本与核心版本分别由 [`version`] / [`core_version`] 返回。
//! 打包、网页运行、Node/Bun 验证和资源路径见
//! [WASM 使用说明](https://github.com/shenjackyuanjie/tswn-core/tree/main/crates/tswn_wasm)。

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

/// 仅安装一次控制台 panic hook；用于诊断，不提供 catch 或业务错误转换。
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
/// 返回带 `data:image/png;base64,` 前缀的图标文本，可直接作为图片 URL。
pub fn name_to_png_base64(name: String) -> String {
    install_panic_hook();
    tswn_core::namerena::icon_render::render_icon_b64_from_name(&name)
}

#[wasm_bindgen]
/// 返回独立的 PNG 文件字节，JS 侧为字节数组；不含 Base64/data URL 前缀。
pub fn name_to_png_bytes(name: String) -> Vec<u8> {
    install_panic_hook();
    tswn_core::namerena::icon_render::render_icon_png_from_name(&name)
}

#[wasm_bindgen]
/// 返回按行排列的 16×16 RGBA 像素，共 1024 字节，可用于 ImageData。
pub fn name_to_icon_rgba(name: String) -> Vec<u8> {
    install_panic_hook();
    tswn_core::namerena::icon_render::render_icon_vec_from_name(&name)
}

#[wasm_bindgen]
/// 同步收集旧页面格式回放；options.capture_replay=false 时仍推进，但不保存 frames。
/// 需要明确截断原因与规范 DTO 时使用 battle_replay 或 BattleSession。
pub fn fight(raw_input: String, options: Option<Ts<FightOptions>>) -> WasmResult<Ts<FightReplay>> {
    install_panic_hook();
    let options = ts_in(options)?;
    fight::fight_impl(raw_input, options).and_then(|replay| ts_out(&replay))
}

#[wasm_bindgen]
/// 同步运行旧页面兼容对局并返回最终摘要；整个调用完成前不让出事件循环。
pub fn fight_summary(raw_input: String, options: Option<Ts<FightOptions>>) -> WasmResult<Ts<FightSummary>> {
    install_panic_hook();
    let options = ts_in(options)?;
    fight::fight_summary_impl(raw_input, options).and_then(|summary| ts_out(&summary))
}

#[wasm_bindgen]
/// 同步执行全部试验，返回第一组的百分比胜率；输入按空行分队。
/// total_rounds=0 返回已完成的零试验结果；thread 只接受 0/1，大批任务优先分步执行。
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
/// 按 against 顺序同步比较 target 与每个对手，每项文本表示一个多行队伍。
/// 任一对局失败则整次调用抛错，不返回已完成的数组前缀。
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
/// 以 core CLI 口径计算第一组的计数、百分比胜率与纳秒耗时；省略 eval_rq 使用胜率默认值。
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
/// 比较两个多行队伍并返回 CLI 胜率摘要；所有计算同步完成后才返回。
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
/// 以 mode 指定单项评分，省略时用 normal；返回分数、胜场/总场、失败统计和耗时。
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
/// 按行解析名字组合并计算选定模式；keep_rq 为真使用普通构建口径。
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
/// 以正且有限的 target_factors 加权目标组；权重数组必须与目标组等长。
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
/// 对带靶子权重的搭档组合评分；head 控制最终汇总的头部搭档数量。
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

/// 同步收集规范回放为普通 JS 对象，并不是 JSON 字符串。
///
/// max_rounds 为正数回合预算；正常截断仍返回结果，Runtime 失败抛出带 code 的值。
/// 完整帧和快照保存在内存中，长对局可用 BattleSession 按需取帧。
#[wasm_bindgen(unchecked_return_type = "BattleReplay")]
pub fn battle_replay(raw_input: String, options: Option<Ts<BattleReplayOptions>>) -> WasmResult<JsValue> {
    install_panic_hook();
    let options = ts_in(options)?;
    let replay = tswn_core::cli_api::battle_replay(&raw_input, options.to_core()).map_err(error::cli_api_error)?;
    battle::dto_to_js(&replay)
}

/// 与 battle_replay 相同的回放，但允许卡牌前端为某个实体按顺序强制技能。
///
/// forced_skills 是技能 id 队列：每次轮到 actor_id 行动时消费一个；
/// 队列耗尽后恢复 Runtime 原本的概率选择。
#[wasm_bindgen(unchecked_return_type = "BattleReplay")]
pub fn battle_replay_forced(
    raw_input: String,
    actions: Vec<u32>,
    options: Option<Ts<BattleReplayOptions>>,
) -> WasmResult<JsValue> {
    install_panic_hook();
    let options = ts_in(options)?;
    // 三元组：actor, skill, mp（0 表示这一手没有指定消耗，由引擎现场掷）
    let pairs = actions
        .chunks_exact(3)
        .map(|chunk| {
            let cost = chunk[2] & 0xff;
            (chunk[0], chunk[1], (cost > 0).then_some(cost))
        })
        .collect::<Vec<_>>();
    let potions = actions
        .chunks_exact(3)
        .filter_map(|chunk| {
            let code = chunk[2] >> 8;
            (code > 0).then_some((chunk[0], code))
        })
        .collect::<Vec<_>>();
    tswn_core::runtime::forced_action::set_potions(potions);
    tswn_core::runtime::forced_action::set(pairs);
    let result = tswn_core::cli_api::battle_replay(&raw_input, options.to_core());
    tswn_core::runtime::forced_action::clear();
    battle::dto_to_js(&result.map_err(error::cli_api_error)?)
}

#[wasm_bindgen]
pub fn forced_hits() -> u32 { tswn_core::runtime::forced_action::hits() }

#[wasm_bindgen]
pub fn forced_taken() -> u32 { tswn_core::runtime::forced_action::taken() }

#[wasm_bindgen]
/// 将单名转换为 DIY 文本；old/minions 省略时为 false，冲突选项由 core 校验。
pub fn to_diy(name: String, old: Option<bool>, minions: Option<bool>) -> WasmResult<String> {
    install_panic_hook();
    tswn_core::cli_api::to_diy(&name, old.unwrap_or(false), minions.unwrap_or(false)).map_err(error::cli_api_error)
}

#[wasm_bindgen]
/// 按输入顺序返回 DIY 文本数组；任一名字失败则抛错，不返回部分结果。
pub fn to_diy_batch(names: Vec<String>, old: Option<bool>, minions: Option<bool>) -> WasmResult<Vec<String>> {
    install_panic_hook();
    tswn_core::cli_api::to_diy_batch(&names, old.unwrap_or(false), minions.unwrap_or(false)).map_err(error::cli_api_error)
}

#[wasm_bindgen]
/// 返回图标形状和颜色索引的普通 JS 对象，不含 PNG 像素数据。
pub fn icon_info(name: String) -> WasmResult<Ts<CliIconInfo>> {
    install_panic_hook();
    let info = tswn_core::cli_api::icon_info(&name).into();
    ts_out(&info)
}

#[wasm_bindgen]
/// 逐行解析名字组为多行队伍文本数组；double_plus 为真使用 ++，否则使用 +。
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

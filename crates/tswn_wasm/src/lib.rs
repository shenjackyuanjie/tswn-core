mod error;
mod fight;
mod model;
mod render;
mod win_rate;

use std::sync::Once;

use error::WasmResult;
pub use fight::FightSession;
use model::{FightOptions, FightReplay, FightSummary, GroupWinRateResult, WinRateOptions, WinRateResult};
use wasm_bindgen::prelude::*;
pub use win_rate::WinRateSession;

static PANIC_HOOK: Once = Once::new();

pub(crate) fn install_panic_hook() { PANIC_HOOK.call_once(console_error_panic_hook::set_once); }

#[wasm_bindgen(start)]
pub fn wasm_start() { install_panic_hook(); }

#[wasm_bindgen]
pub fn version() -> String { env!("CARGO_PKG_VERSION").to_string() }

#[wasm_bindgen]
pub fn core_version() -> String { tswn_core::version().to_string() }

#[wasm_bindgen]
pub fn default_eval_rq() -> f64 { tswn_core::player::eval_name::DEFAULT_EVAL_RQ }

#[wasm_bindgen]
pub fn win_rate_eval_rq() -> f64 { tswn_core::player::eval_name::WIN_RATE_EVAL_RQ }

#[wasm_bindgen]
pub fn name_to_png_base64(name: String) -> String {
    install_panic_hook();
    tswn_core::player::icon_render::render_icon_b64_from_name(&name)
}

#[wasm_bindgen]
pub fn name_to_png_bytes(name: String) -> Vec<u8> {
    install_panic_hook();
    tswn_core::player::icon_render::render_icon_png_from_name(&name)
}

#[wasm_bindgen]
pub fn name_to_icon_rgba(name: String) -> Vec<u8> {
    install_panic_hook();
    tswn_core::player::icon_render::render_icon_vec_from_name(&name)
}

#[wasm_bindgen]
pub fn fight(raw_input: String, options: Option<FightOptions>) -> WasmResult<FightReplay> {
    install_panic_hook();
    let options = options.unwrap_or_default();
    fight::fight_impl(raw_input, options)
}

#[wasm_bindgen]
pub fn fight_summary(raw_input: String, options: Option<FightOptions>) -> WasmResult<FightSummary> {
    install_panic_hook();
    let options = options.unwrap_or_default();
    fight::fight_summary_impl(raw_input, options)
}

#[wasm_bindgen]
pub fn win_rate_sync(raw_input: String, total_rounds: usize, options: Option<WinRateOptions>) -> WasmResult<WinRateResult> {
    install_panic_hook();
    let options = options.unwrap_or_default();
    let result = win_rate::run_win_rate_sync(raw_input, total_rounds, options)?;
    Ok(result)
}

#[wasm_bindgen]
pub fn group_win_rate(
    target: String,
    against: Vec<String>,
    total_rounds: usize,
    options: Option<WinRateOptions>,
) -> WasmResult<Vec<GroupWinRateResult>> {
    install_panic_hook();
    let options = options.unwrap_or_default();

    let mut results = Vec::with_capacity(against.len());
    for opponent in against {
        let raw_input = format!("{target}\n\n{opponent}");
        let result = win_rate::run_win_rate_sync(raw_input, total_rounds, options.clone())?;
        results.push(GroupWinRateResult { opponent, result });
    }

    Ok(results)
}

mod error;
mod fight;
mod model;
mod render;
mod win_rate;

use std::sync::Once;

use error::{WasmResult, invalid_options, parse_options, to_js_value};
pub use fight::FightSession;
use model::{FightOptions, GroupWinRateResult, WinRateOptions};
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
pub fn fight(raw_input: String, options: Option<JsValue>) -> WasmResult<JsValue> {
    install_panic_hook();
    let options: FightOptions = parse_options(options)?;
    fight::fight_impl(raw_input, options)
}

#[wasm_bindgen]
pub fn fight_summary(raw_input: String, options: Option<JsValue>) -> WasmResult<JsValue> {
    install_panic_hook();
    let options: FightOptions = parse_options(options)?;
    fight::fight_summary_impl(raw_input, options)
}

#[wasm_bindgen]
pub fn win_rate_sync(raw_input: String, total_rounds: usize, options: Option<JsValue>) -> WasmResult<JsValue> {
    install_panic_hook();
    let options: WinRateOptions = parse_options(options)?;
    let result = win_rate::run_win_rate_sync(raw_input, total_rounds, options)?;
    to_js_value(&result)
}

#[wasm_bindgen]
pub fn group_win_rate(target: String, against: JsValue, total_rounds: usize, options: Option<JsValue>) -> WasmResult<JsValue> {
    install_panic_hook();
    let options: WinRateOptions = parse_options(options)?;
    let against: Vec<String> =
        serde_wasm_bindgen::from_value(against).map_err(|err| invalid_options(format!("failed to parse against: {err}")))?;

    let mut results = Vec::with_capacity(against.len());
    for opponent in against {
        let raw_input = format!("{target}\n\n{opponent}");
        let result = win_rate::run_win_rate_sync(raw_input, total_rounds, options.clone())?;
        results.push(GroupWinRateResult { opponent, result });
    }

    to_js_value(&results)
}

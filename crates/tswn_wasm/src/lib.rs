mod error;
mod fight;
mod model;
mod render;
mod win_rate;

use std::sync::Once;

use error::{WasmResult, parse_options};
pub use fight::FightSession;
use model::{FightOptions, WinRateOptions};
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
pub fn name_to_png_base64(name: String) -> String {
    install_panic_hook();
    tswn_core::player::icon_render::render_icon_b64_from_name(&name)
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
    let mut session = WinRateSession::new(raw_input, total_rounds, serde_wasm_bindgen::to_value(&options).ok())?;
    let _ = session.step(Some(total_rounds.max(1)))?;
    session.result()
}

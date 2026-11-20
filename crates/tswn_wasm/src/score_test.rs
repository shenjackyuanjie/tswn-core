//! 有限评分会话的 WASM 包装，长循环由 Worker 驱动。
use crate::{
    battle::dto_to_js,
    error::{WasmResult, cli_api_error},
};
use tswn_core::runtime::{ScoreTestSession as CoreSession, search::SearchConfig};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct ScoreTestSession {
    inner: CoreSession,
}

#[wasm_bindgen]
impl ScoreTestSession {
    #[wasm_bindgen(constructor)]
    pub fn new(raw_input: String, search: bool) -> WasmResult<Self> {
        crate::install_panic_hook();
        Ok(Self {
            inner: CoreSession::new(&raw_input, search.then(SearchConfig::default)).map_err(cli_api_error)?,
        })
    }

    /// 推进一场。10000 场后不再推进，也不会重新开始下一轮。
    pub fn step(&mut self) -> WasmResult<()> { self.inner.step().map_err(cli_api_error) }

    pub fn progress(&self) -> WasmResult<JsValue> { dto_to_js(&self.inner.progress()) }
}

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
    /// 第三个参数选择强化 profile，第四个参数显式开启原版对手验证；省略时双方使用所选策略。
    pub fn new(raw_input: String, search: bool, enhanced: Option<bool>, legacy_opponents: Option<bool>) -> WasmResult<Self> {
        crate::install_panic_hook();
        let mut inner = CoreSession::new_with_mode(&raw_input, search.then(SearchConfig::default), enhanced.unwrap_or(false))
            .map_err(cli_api_error)?;
        inner.set_legacy_opponents(legacy_opponents.unwrap_or(false));
        Ok(Self { inner })
    }

    /// 推进一场。10000 场后不再推进，也不会重新开始下一轮。
    pub fn step(&mut self) -> WasmResult<()> { self.inner.step().map_err(cli_api_error) }
    pub fn progress(&self) -> WasmResult<JsValue> { dto_to_js(&self.inner.progress()) }
}

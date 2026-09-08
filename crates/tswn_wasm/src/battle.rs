//! 规范的面向用户流式 API；DTO 直接从 core 序列化。
use crate::{
    error::{WasmResult, cli_api_error, internal_error},
    model::BattleOptions,
};
use serde::Serialize;
use tswn_core::cli_api::battle::BattleSession as CoreBattleSession;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const BATTLE_TYPES: &str = include_str!("battle_types.d.ts");

pub(crate) fn dto_to_js<T: Serialize + ?Sized>(dto: &T) -> WasmResult<JsValue> {
    dto.serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|error| internal_error(error.to_string()))
}

#[wasm_bindgen]
pub struct BattleSession {
    inner: CoreBattleSession,
}

#[wasm_bindgen]
impl BattleSession {
    #[wasm_bindgen(constructor)]
    pub fn new(raw_input: String, options: Option<BattleOptions>) -> WasmResult<BattleSession> {
        crate::install_panic_hook();
        Ok(Self {
            inner: CoreBattleSession::new(&raw_input, options.unwrap_or_default().to_core()).map_err(cli_api_error)?,
        })
    }
    #[wasm_bindgen(unchecked_return_type = "BattlePlayerState[]")]
    pub fn initial_states(&self) -> WasmResult<JsValue> { dto_to_js(self.inner.initial_states()) }
    #[wasm_bindgen(unchecked_return_type = "BattlePlayerState[]")]
    pub fn current_states(&self) -> WasmResult<JsValue> { dto_to_js(self.inner.current_states()) }
    #[wasm_bindgen(unchecked_return_type = "BattleReplayFrame | null")]
    pub fn next_frame(&mut self) -> WasmResult<JsValue> { dto_to_js(&self.inner.next_frame().map_err(cli_api_error)?) }
    #[wasm_bindgen(unchecked_return_type = "BattleStatus")]
    pub fn status(&self) -> WasmResult<JsValue> { dto_to_js(&self.inner.status()) }
    #[wasm_bindgen(unchecked_return_type = "BattleStopReason | null")]
    pub fn stop_reason(&self) -> WasmResult<JsValue> { dto_to_js(&self.inner.stop_reason()) }
    pub fn is_done(&self) -> bool { self.inner.is_done() }
    pub fn is_failed(&self) -> bool { self.inner.is_failed() }
    pub fn is_finished(&self) -> bool { self.inner.is_finished() }
    pub fn is_truncated(&self) -> bool { self.inner.is_truncated() }
    pub fn rounds_advanced(&self) -> usize { self.inner.rounds_advanced() }
    pub fn frames_emitted(&self) -> usize { self.inner.frames_emitted() }
    #[wasm_bindgen(unchecked_return_type = "BattleResult | null")]
    pub fn result(&self) -> WasmResult<JsValue> { dto_to_js(&self.inner.result()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tswn_core::cli_api::BattleOptions;

    #[test]
    fn wasm_session_query_exposes_real_sticky_runtime_failure() {
        let mut session = BattleSession {
            inner: CoreBattleSession::new("alpha@red+bed2[3000]\n\nbeta@blue", BattleOptions::default()).unwrap(),
        };
        assert!(!session.is_failed());
        session.inner.invalidate_runtime_for_test();
        let mut previous_error = None;
        for _ in 0..2 {
            // 原生测试不能构造 JsValue；经真实 Runtime 失败后调用同一个导出的 boolean query。
            let error = session.inner.next_frame().unwrap_err();
            let error = crate::error::cli_api_tswn_error(error);
            assert_eq!(error.code, "RUNTIME_FAILED");
            let current = (error.code, error.message);
            if let Some(previous) = &previous_error {
                assert_eq!(&current, previous);
            }
            previous_error = Some(current);
            assert!(session.is_failed());
            assert!(!session.is_done());
            assert!(!session.is_truncated());
            assert!(session.inner.result().is_none());
            assert!(session.inner.stop_reason().is_none());
        }
    }
}

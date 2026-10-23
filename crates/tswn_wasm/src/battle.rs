//! 规范的同步流式战斗 API；DTO 直接从 core 序列化为普通 JS 对象。
//!
//! 会话独占运行时，JS 宿主负责调用 free；DTO 为独立副本，不借用会话内存。
//! next_frame 先推进再序列化，输出转换失败不会回滚已经产生的帧。
//! 领域错误抛出带 code/message 的普通值，panic hook 不构成异常恢复边界。
use crate::{
    convert::ts_in,
    error::{WasmResult, cli_api_error, internal_error},
    model::BattleOptions,
};
use serde::Serialize;
use tsify::Ts;
use tswn_core::cli_api::battle::BattleSession as CoreBattleSession;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const BATTLE_TYPES: &str = include_str!("battle_types.d.ts");

pub(crate) fn dto_to_js<T: Serialize + ?Sized>(dto: &T) -> WasmResult<JsValue> {
    dto.serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|error| internal_error(error.to_string()))
}

#[wasm_bindgen]
/// 持有 core 运行时的 wasm-bindgen 对局对象，按需同步取帧。
///
/// JS 宿主在 finally 中调用生成的 free()；释放前返回的普通 DTO 可继续读取。
/// Runtime 失败不等同于正常终止，is_done 为 false 时也应检查 is_failed。
pub struct BattleSession {
    inner: CoreBattleSession,
}

#[wasm_bindgen]
impl BattleSession {
    #[wasm_bindgen(constructor)]
    /// 解析空行分队的 raw 和可选 options；max_rounds 为正数回合预算。
    /// 未提供选项时使用 core 默认值；错误抛出 `{code, message}`，不是 Error 实例。
    pub fn new(raw_input: String, options: Option<Ts<BattleOptions>>) -> WasmResult<BattleSession> {
        crate::install_panic_hook();
        let options = ts_in(options)?;
        Ok(Self {
            inner: CoreBattleSession::new(&raw_input, options.to_core()).map_err(cli_api_error)?,
        })
    }
    #[wasm_bindgen(unchecked_return_type = "BattlePlayerState[]")]
    /// 返回创建时的规范玩家快照数组。
    pub fn initial_states(&self) -> WasmResult<JsValue> { dto_to_js(self.inner.initial_states()) }
    #[wasm_bindgen(unchecked_return_type = "BattlePlayerState[]")]
    /// 返回当前边界的规范玩家快照数组。
    pub fn current_states(&self) -> WasmResult<JsValue> { dto_to_js(self.inner.current_states()) }
    #[wasm_bindgen(unchecked_return_type = "BattleReplayFrame | null")]
    /// 同步推进至下一可见帧；正常结束或截断后返回 JS null，可重复调用。
    /// Runtime 失败会保留 sticky error 并重复抛错，应停止推进并释放会话。
    pub fn next_frame(&mut self) -> WasmResult<JsValue> { dto_to_js(&self.inner.next_frame().map_err(cli_api_error)?) }
    #[wasm_bindgen(unchecked_return_type = "BattleStatus")]
    pub fn status(&self) -> WasmResult<JsValue> { dto_to_js(&self.inner.status()) }
    #[wasm_bindgen(unchecked_return_type = "BattleStopReason | null")]
    pub fn stop_reason(&self) -> WasmResult<JsValue> { dto_to_js(&self.inner.stop_reason()) }
    /// 只表示正常结束或截断；失败应另查 is_failed，不属于正常结果。
    pub fn is_done(&self) -> bool { self.inner.is_done() }
    pub fn is_failed(&self) -> bool { self.inner.is_failed() }
    pub fn is_finished(&self) -> bool { self.inner.is_finished() }
    pub fn is_truncated(&self) -> bool { self.inner.is_truncated() }
    pub fn rounds_advanced(&self) -> usize { self.inner.rounds_advanced() }
    pub fn frames_emitted(&self) -> usize { self.inner.frames_emitted() }
    #[wasm_bindgen(unchecked_return_type = "BattleResult | null")]
    /// 读取正常 terminal 结果，运行中或失败时返回 JS null。
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

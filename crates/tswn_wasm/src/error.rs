//! WASM 层错误类型与辅助函数。
//!
//! 定义 `TswnError` 结构体（含 `code`/`message` 字段）及各类错误构造函数，
//! 统一将错误序列化为 `JsValue` 以便 JavaScript 层处理。

use serde::Serialize;
use wasm_bindgen::JsValue;

#[derive(Debug, Clone, Serialize)]
pub struct TswnError {
    pub code: &'static str,
    pub message: String,
}

pub type WasmResult<T> = Result<T, JsValue>;

pub fn invalid_input(message: impl Into<String>) -> JsValue { error_value("INVALID_INPUT", message) }

pub fn runner_init_failed(message: impl Into<String>) -> JsValue { error_value("RUNNER_INIT_FAILED", message) }

pub fn win_rate_invalid_groups() -> JsValue {
    error_value("WIN_RATE_INVALID_GROUPS", "win_rate requires at least two non-empty groups")
}

pub fn internal_error(message: impl Into<String>) -> JsValue { error_value("INTERNAL_ERROR", message) }

pub fn error_value(code: &'static str, message: impl Into<String>) -> JsValue {
    let error = TswnError {
        code,
        message: message.into(),
    };
    serde_wasm_bindgen::to_value(&error).unwrap_or_else(|_| JsValue::from_str(error.code))
}

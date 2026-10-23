//! WASM 层错误类型与辅助函数。
//!
//! 定义 `TswnError` 结构体（含 `code`/`message` 字段）及各类错误构造函数，
//! 统一将错误序列化为 `JsValue`，导出函数返回 Err 时由 wasm-bindgen 抛给宿主。
//! 抛出的通常是普通对象，不是 JavaScript Error；序列化失败仅保留 code 字符串。
//! 本模块不捕获 Rust panic，调用方不能把业务 Result 与 WASM trap 混为一谈。

use serde::Serialize;
use tswn_core::cli_api::{CliApiError, CliApiErrorCode};
use wasm_bindgen::JsValue;

#[derive(Debug, Clone, Serialize)]
/// 可序列化的业务错误载荷；code 用于程序判断，message 用于展示诊断。
pub struct TswnError {
    pub code: &'static str,
    pub message: String,
}

pub type WasmResult<T> = Result<T, JsValue>;

pub fn invalid_input(message: impl Into<String>) -> JsValue { error_value(CliApiErrorCode::InvalidInput.as_str(), message) }

pub fn invalid_argument(message: impl Into<String>) -> JsValue { error_value(CliApiErrorCode::InvalidArgument.as_str(), message) }

pub fn runner_init_failed(message: impl Into<String>) -> JsValue {
    error_value(CliApiErrorCode::RunnerInitFailed.as_str(), message)
}

pub fn win_rate_invalid_groups() -> JsValue {
    error_value("WIN_RATE_INVALID_GROUPS", "win_rate requires at least two non-empty groups")
}

pub fn internal_error(message: impl Into<String>) -> JsValue { error_value(CliApiErrorCode::InternalError.as_str(), message) }

pub fn unsupported_option(message: impl Into<String>) -> JsValue {
    error_value(CliApiErrorCode::UnsupportedOption.as_str(), message)
}

/// 构造可抛出的普通 JS 值；序列化失败时退化为稳定 code 字符串，不再次 panic。
pub fn error_value(code: &'static str, message: impl Into<String>) -> JsValue {
    let error = TswnError::new(code, message);
    serde_wasm_bindgen::to_value(&error).unwrap_or_else(|_| JsValue::from_str(error.code))
}

pub fn cli_api_error(err: CliApiError) -> JsValue {
    let error = cli_api_tswn_error(err);
    error_value(error.code, error.message)
}

pub fn cli_api_tswn_error(err: CliApiError) -> TswnError { TswnError::new(err.code().as_str(), err.to_string()) }

impl TswnError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

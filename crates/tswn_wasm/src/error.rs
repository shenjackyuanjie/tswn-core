use serde::Serialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::JsValue;

#[derive(Debug, Clone, Serialize)]
pub struct TswnError {
    pub code: &'static str,
    pub message: String,
}

pub type WasmResult<T> = Result<T, JsValue>;

pub fn invalid_input(message: impl Into<String>) -> JsValue { error_value("INVALID_INPUT", message) }

pub fn invalid_options(message: impl Into<String>) -> JsValue { error_value("INVALID_OPTIONS", message) }

pub fn runner_init_failed(message: impl Into<String>) -> JsValue { error_value("RUNNER_INIT_FAILED", message) }

pub fn win_rate_invalid_groups() -> JsValue {
    error_value(
        "WIN_RATE_INVALID_GROUPS",
        "win_rate requires at least two non-empty groups",
    )
}

pub fn internal_error(message: impl Into<String>) -> JsValue { error_value("INTERNAL_ERROR", message) }

pub fn error_value(code: &'static str, message: impl Into<String>) -> JsValue {
    let error = TswnError {
        code,
        message: message.into(),
    };
    serde_wasm_bindgen::to_value(&error).unwrap_or_else(|_| JsValue::from_str(error.code))
}

pub fn to_js_value<T>(value: &T) -> WasmResult<JsValue>
where
    T: Serialize,
{
    serde_wasm_bindgen::to_value(value)
        .map_err(|err| internal_error(format!("failed to serialize wasm value: {err}")))
}

pub fn parse_options<T>(value: Option<JsValue>) -> WasmResult<T>
where
    T: Default + DeserializeOwned,
{
    let Some(value) = value else {
        return Ok(T::default());
    };
    if value.is_null() || value.is_undefined() {
        return Ok(T::default());
    }
    serde_wasm_bindgen::from_value(value)
        .map_err(|err| invalid_options(format!("failed to parse options: {err}")))
}
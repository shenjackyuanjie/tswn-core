//! WASM 边界与 Rust 领域类型之间的 `tsify::Ts<T>` 转换辅助。
//!
//! `tsify` 的 `into_wasm_abi` / `from_wasm_abi` 属性已弃用：它们把（反）序列化
//! 下沉到 wasm-bindgen ABI 边界，失败时只能 `throw_str`，会跳过析构函数并泄漏内存
//! （见 <https://github.com/madonoharu/tsify/issues/65>）。
//! 导出函数现在接收 `Option<Ts<T>>` 参数、返回 `Ts<T>` / `Vec<Ts<T>>`，
//! 再经这里的辅助函数在函数体内转换为普通 Rust 类型，失败按稳定错误码抛出。

use serde::Serialize;
use tsify::Ts;
use wasm_bindgen::JsValue;

use crate::error::{internal_error, invalid_argument};

/// 把 JS 侧传入的可选 `Ts<T>` 转成 Rust 值，未提供时用默认值。
///
/// 反序列化失败说明 JS 传来的参数不符合契约，按 `INVALID_ARGUMENT` 抛出；
/// 相比旧的 ABI 边界转换，这里能正常析构，不会泄漏内存。
pub fn ts_in<T>(value: Option<Ts<T>>) -> Result<T, JsValue>
where
    T: tsify::Tsify + serde::de::DeserializeOwned + Default,
    <T as tsify::Tsify>::JsType: Clone,
{
    match value {
        Some(value) => value.to_rust().map_err(|error| invalid_argument(error.to_string())),
        None => Ok(T::default()),
    }
}

/// 把 Rust 值转换成可回传 JS 的 `Ts<T>`。
///
/// 序列化失败属于 Rust 侧数据问题，按 `INTERNAL_ERROR` 抛出。
pub fn ts_out<T>(value: &T) -> Result<Ts<T>, JsValue>
where
    T: tsify::Tsify + Serialize,
{
    Ts::from_rust(value).map_err(|error| internal_error(error.to_string()))
}

/// 批量把 Rust 值转换成可回传 JS 的 `Vec<Ts<T>>`。
pub fn ts_out_vec<T>(values: &[T]) -> Result<Vec<Ts<T>>, JsValue>
where
    T: tsify::Tsify + Serialize,
{
    values.iter().map(ts_out).collect()
}

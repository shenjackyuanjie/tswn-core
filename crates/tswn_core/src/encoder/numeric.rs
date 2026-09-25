//! 数值变换：固定计数尺度、逐字段 `N_f` 与 f32 舍入。
//!
//! 规格第 5 节把数值槽分成两类：**机制计数的绝对量**用固定线性尺度（分母取 p99，
//! 不随容量放大），**逐字段标量**用 train 拟合的 `N_f`。任一数值槽只归一类，
//! 禁止先线性缩放再套 `N_f`；`list_position`/`order_rank` 是结构关系变换，不在此列。
//!
//! 所有变换先在 f64 计算，再按 IEEE 最近偶数舍入到 f32；NaN/Inf 在进入变换前报
//! [`EncodeError::NonFiniteValue`]，不产生“看起来正常”的数值样本。

use serde::{Deserialize, Serialize};

use crate::encoder::error::EncodeError;

/// 归一化变换类型；manifest 必须逐槽声明，不能只给 `s_f`/`c_f`（外部评审 4.2）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TransformKind {
    /// 固定线性计数尺度：`x/divisor`，裁剪 `[0,1]`。
    #[serde(rename = "fixed_count")]
    FixedCount { divisor: f64 },
    /// 逐字段拟合的 `N_f`，裁剪 `[-4,4]`。
    #[serde(rename = "fitted")]
    Fitted,
    /// 结构关系变换（`list_position` / `order_rank`）；不参与标量拟合。
    #[serde(rename = "structural")]
    Structural,
    /// 不归一化（bool、分类、引用、bit）。
    #[serde(rename = "passthrough")]
    Passthrough,
}

/// `N_f(x)=clip(sign(x)·ln(1+|x|/s_f)/ln(1+c_f/s_f), -4, 4)`。
///
/// `s_f`/`c_f` 由调用方保证有限且 `≥ 1`（manifest 校验时检查），因此分母不会为 0；
/// `x=0` 与 `-0` 都映射到对应符号的 0，保留零的符号区别。
pub fn normalize_fitted(value: f64, s_f: f64, c_f: f64) -> f64 {
    let numerator = (1.0 + value.abs() / s_f).ln() * value.signum();
    let denominator = (1.0 + c_f / s_f).ln();
    (numerator / denominator).clamp(-4.0, 4.0)
}

/// 固定计数尺度：`x/divisor` 裁剪到 `[0,1]`；超过 p99 的值饱和到 1，饱和不是容量错误。
pub fn normalize_fixed_count(value: f64, divisor: f64) -> f64 { (value / divisor).clamp(0.0, 1.0) }

/// 按声明的变换归一化一个数值槽的值；调用方已保证 `value` 有限。
pub fn apply_transform(value: f64, kind: TransformKind, s_f: f64, c_f: f64) -> f64 {
    match kind {
        TransformKind::FixedCount { divisor } => normalize_fixed_count(value, divisor),
        TransformKind::Fitted => normalize_fitted(value, s_f, c_f),
        // 结构变换由 list 族自行计算；标量通道不含此类槽位。
        TransformKind::Structural | TransformKind::Passthrough => value,
    }
}

/// f64 → f32（Rust `as` 即 IEEE 最近偶数舍入）；输入与舍入结果都必须有限。
pub fn to_f32_checked(value: f64, path: &str) -> Result<f32, EncodeError> {
    let rounded = value as f32;
    if !value.is_finite() || !rounded.is_finite() {
        return Err(EncodeError::NonFiniteValue { path: path.to_owned() });
    }
    Ok(rounded)
}

/// i64 → f64；用于把机制整数送入数值通道，保留有符号语义（负属性、`round_pos=-1`）。
pub fn integer_to_f64(value: i64) -> f64 { value as f64 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitted_maps_scale_and_clip_points() {
        // s_f=c_f=1 时 x=1 恰好映射到 1，x=c_f 是裁剪点。
        assert_eq!(normalize_fitted(0.0, 1.0, 1.0), 0.0);
        assert_eq!(normalize_fitted(1.0, 1.0, 1.0), 1.0);
        assert_eq!(normalize_fitted(-1.0, 1.0, 1.0), -1.0);
        // 超过 c_f 后饱和到 +4/-4，不越界。
        assert_eq!(normalize_fitted(1e9, 1.0, 1.0), 4.0);
        assert_eq!(normalize_fitted(-1e9, 1.0, 1.0), -4.0);
    }

    #[test]
    fn fitted_keeps_zero_sign_and_small_scale_below_one() {
        assert_eq!(normalize_fitted(-0.0, 3.0, 9.0).to_bits(), (-0.0f64).to_bits());
        // 0 < x < s_f 时结果落在 (0,1)：对数压缩不产生跳变。
        let small = normalize_fitted(0.5, 4.0, 8.0);
        assert!(small > 0.0 && small < 1.0);
    }

    #[test]
    fn fixed_count_saturates_instead_of_erroring() {
        assert_eq!(normalize_fixed_count(0.0, 16.0), 0.0);
        assert_eq!(normalize_fixed_count(8.0, 16.0), 0.5);
        // 容量从 32 提到 64 不改变尺度：超过 p99 只饱和到 1。
        assert_eq!(normalize_fixed_count(10_000.0, 16.0), 1.0);
    }

    #[test]
    fn to_f32_rejects_non_finite() {
        assert!(to_f32_checked(f64::NAN, "x").is_err());
        assert!(to_f32_checked(f64::INFINITY, "x").is_err());
        assert_eq!(to_f32_checked(1.0, "x").unwrap(), 1.0f32);
    }

    #[test]
    fn to_f32_rejects_finite_values_that_overflow_after_rounding() {
        for value in [f64::MAX, -f64::MAX] {
            assert!(value.is_finite());
            assert!(matches!(
                to_f32_checked(value, "x"),
                Err(EncodeError::NonFiniteValue { ref path }) if path == "x"
            ));
        }
        assert_eq!(to_f32_checked(f64::from(f32::MAX), "x").unwrap(), f32::MAX);
        assert_eq!(to_f32_checked(-0.0, "x").unwrap().to_bits(), (-0.0f32).to_bits());
    }

    #[test]
    fn integer_conversion_keeps_sign() {
        assert_eq!(integer_to_f64(-1), -1.0);
        assert_eq!(integer_to_f64(i64::MIN / 2), i64::MIN as f64 / 2.0);
    }
}

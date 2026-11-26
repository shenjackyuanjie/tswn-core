//! 琪露诺的可变技能等级；复位时恢复到开局值。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CirnoRuntime {
    /// 绝对零度每次施放减十，降到零后不再触发。
    pub zero_level: i32,
}
impl Default for CirnoRuntime {
    fn default() -> Self { Self { zero_level: 90 } }
}

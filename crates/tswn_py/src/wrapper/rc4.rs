//! RC4 伪随机数生成器的 Python 封装。
//!
//! 将 [`RC4`] 的完整接口暴露给 Python，
//! 包括状态读写、字节变换、概率判断方法（c94/c75/c50 等）及范围随机方法（r256/r64 等）。
//! 这些接口服务于战斗复现，不提供密码学安全随机数。字节序列输入转为 Rust 自有副本，
//! 返回修改后的序列；Python 原列表不被原地改写，生成器状态则随调用推进。

use pyo3::{PyResult, exceptions::PyValueError, pyclass, pymethods};

use tswn_core::rc4::{RC4, VAL_LEN};

/// 独立持有 RC4 排列表与游标的可变 Python 对象。
///
/// 创建、update 和 round 拒绝空 key，返回 ValueError；round=None 使用对应 core 默认值。
/// i/j 和排列表写入接口用于恢复状态，应仅使用有效快照，任意写入会破坏可复现性。
#[pyclass]
#[pyo3(name = "RC4")]
pub struct PyRC4 {
    pub inner: RC4,
}

impl PyRC4 {
    fn ensure_non_empty_keys(keys: &[u8]) -> PyResult<()> {
        if keys.is_empty() {
            Err(PyValueError::new_err("keys must not be empty"))
        } else {
            Ok(())
        }
    }
}

#[pymethods]
impl PyRC4 {
    /// 用密钥与轮数构造 RC4；密钥不能为空。
    #[new]
    pub fn new(keys: Vec<u8>, round: Option<usize>) -> PyResult<Self> {
        Self::ensure_non_empty_keys(&keys)?;
        Ok(Self {
            inner: RC4::new(&keys, round.unwrap_or(1)),
        })
    }

    /// 状态数组长度（VAL_LEN）。
    #[staticmethod]
    pub fn val_len() -> usize { VAL_LEN }

    /// 读取 i 游标。
    #[getter]
    pub fn get_i(&self) -> u32 { self.inner.i }

    /// 写入 i 游标。
    #[setter]
    pub fn set_i(&mut self, val: u32) { self.inner.i = val; }

    /// 读取 j 游标。
    #[getter]
    pub fn get_j(&self) -> u32 { self.inner.j }

    /// 写入 j 游标。
    #[setter]
    pub fn set_j(&mut self, val: u32) { self.inner.j = val; }

    /// 返回主状态数组的副本。
    pub fn get_val(&self) -> Vec<u8> { self.inner.main_val.to_vec() }

    /// 读取指定下标的状态值。
    pub fn get_val_at(&self, index: u8) -> u8 { self.inner.get_val(index) }

    /// 写入指定下标的状态值。
    pub fn set_val_at(&mut self, index: u8, value: u8) { self.inner.set_val(index, value); }

    /// 用新密钥与轮数更新状态。
    pub fn update(&mut self, keys: Vec<u8>, round: Option<usize>) -> PyResult<()> {
        Self::ensure_non_empty_keys(&keys)?;
        self.inner.update(&keys, round.unwrap_or(1));
        Ok(())
    }

    /// 对字节数组原地异或并返回。
    pub fn xor_bytes(&mut self, mut bytes: Vec<u8>) -> Vec<u8> {
        self.inner.xor_bytes(&mut bytes);
        bytes
    }

    /// 按 JS 兼容方式异或字节数组并返回。
    pub fn js_xor_bytes(&mut self, mut bytes: Vec<u8>) -> Vec<u8> {
        self.inner.js_xor_bytes(&mut bytes);
        bytes
    }

    /// 对字符串执行 RC4 异或。
    pub fn xor_str(&mut self, text: String) { self.inner.xor_str(&text); }

    /// 按 JS 兼容方式异或字符串。
    pub fn js_xor_str(&mut self, text: String) { self.inner.js_xor_str(&text); }

    /// 加密字节数组并返回。
    pub fn encrypt_bytes(&mut self, mut bytes: Vec<u8>) -> Vec<u8> {
        self.inner.encrypt_bytes(&mut bytes);
        bytes
    }

    /// 以不修改输入的方式加密字符串。
    pub fn encrypt_bytes_no_change(&mut self, text: String) { self.inner.encrypt_bytes_no_change(&text); }

    /// 解密字节数组并返回。
    pub fn decrypt_bytes(&mut self, mut bytes: Vec<u8>) -> Vec<u8> {
        self.inner.decrypt_bytes(&mut bytes);
        bytes
    }

    /// 取下一个随机 u8。
    pub fn next_u8(&mut self) -> u8 { self.inner.next_u8() }

    /// 取下一个小于 max 的随机 i32。
    pub fn next_i32(&mut self, max: i32) -> i32 { self.inner.next_i32(max) }

    /// 按新密钥与轮数执行 round。
    pub fn round(&mut self, keys: Vec<u8>, round: Option<usize>) -> PyResult<()> {
        Self::ensure_non_empty_keys(&keys)?;
        self.inner.round(&keys, round);
        Ok(())
    }

    /// 查看下一字节但不推进状态，便于与实际 next_u8 结果对照。
    pub fn peek_next_u8(&self) -> u8 { self.inner.peek_next_u8() }

    /// 按 94% 概率判定。
    pub fn c94(&mut self) -> bool { self.inner.c94() }

    /// 按 75% 概率判定。
    pub fn c75(&mut self) -> bool { self.inner.c75() }

    /// 按 50% 概率判定。
    pub fn c50(&mut self) -> bool { self.inner.c50() }

    /// 按 25% 概率判定。
    pub fn c25(&mut self) -> bool { self.inner.c25() }

    /// 按 12% 概率判定。
    pub fn c12(&mut self) -> bool { self.inner.c12() }

    /// 按 33% 概率判定。
    pub fn c33(&mut self) -> bool { self.inner.c33() }

    /// 按 66% 概率判定。
    pub fn c66(&mut self) -> bool { self.inner.c66() }

    /// 取 24 位随机数（[0, 0xFFFFFF]）。
    #[allow(non_snake_case)]
    pub fn rFFFFFF(&mut self) -> u32 { self.inner.rFFFFFF() }

    /// 取 16 位随机数（[0, 0xFFFF]）。
    #[allow(non_snake_case)]
    pub fn rFFFF(&mut self) -> u32 { self.inner.rFFFF() }

    /// 取 [0, 256) 随机数。
    pub fn r256(&mut self) -> u32 { self.inner.r256() }

    /// 取 [0, 64) 随机数。
    pub fn r64(&mut self) -> u32 { self.inner.r64() }

    /// 取 [0, 16) 随机数。
    pub fn r16(&mut self) -> u32 { self.inner.r16() }

    /// 取 [0, 255] 随机数。
    pub fn r255(&mut self) -> u32 { self.inner.r255() }

    /// 取 [0, 127] 随机数。
    pub fn r127(&mut self) -> u32 { self.inner.r127() }

    /// 取 [0, 63] 随机数。
    pub fn r63(&mut self) -> u32 { self.inner.r63() }

    /// 取 [0, 31] 随机数。
    pub fn r31(&mut self) -> u32 { self.inner.r31() }

    /// 取 [0, 15] 随机数。
    pub fn r15(&mut self) -> u32 { self.inner.r15() }

    /// 取 [0, 7] 随机数。
    pub fn r7(&mut self) -> u32 { self.inner.r7() }

    /// 取 [0, 3] 随机数。
    pub fn r3(&mut self) -> u32 { self.inner.r3() }

    /// 取 3x3 相关随机数。
    pub fn r3x3(&mut self) -> u32 { self.inner.r3x3() }
}

impl From<RC4> for PyRC4 {
    fn from(value: RC4) -> Self { Self { inner: value } }
}

impl From<PyRC4> for RC4 {
    fn from(value: PyRC4) -> Self { value.inner }
}

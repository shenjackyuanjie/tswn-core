use pyo3::{PyResult, exceptions::PyValueError, pyclass, pymethods};

use tswn_core::rc4::{RC4, VAL_LEN};

/// RC4 的 Python 封装
///
/// 很多麻烦事都是从这里来的
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
    #[new]
    pub fn new(keys: Vec<u8>, round: Option<usize>) -> PyResult<Self> {
        Self::ensure_non_empty_keys(&keys)?;
        Ok(Self {
            inner: RC4::new(&keys, round.unwrap_or(1)),
        })
    }

    #[staticmethod]
    pub fn val_len() -> usize { VAL_LEN }

    #[getter]
    pub fn get_i(&self) -> u32 { self.inner.i }

    #[setter]
    pub fn set_i(&mut self, val: u32) { self.inner.i = val; }

    #[getter]
    pub fn get_j(&self) -> u32 { self.inner.j }

    #[setter]
    pub fn set_j(&mut self, val: u32) { self.inner.j = val; }

    pub fn get_val(&self) -> Vec<u8> { self.inner.main_val.to_vec() }

    pub fn get_val_at(&self, index: u8) -> u8 { self.inner.get_val(index) }

    pub fn set_val_at(&mut self, index: u8, value: u8) { self.inner.set_val(index, value); }

    pub fn update(&mut self, keys: Vec<u8>, round: Option<usize>) -> PyResult<()> {
        Self::ensure_non_empty_keys(&keys)?;
        self.inner.update(&keys, round.unwrap_or(1));
        Ok(())
    }

    pub fn xor_bytes(&mut self, mut bytes: Vec<u8>) -> Vec<u8> {
        self.inner.xor_bytes(&mut bytes);
        bytes
    }

    pub fn js_xor_bytes(&mut self, mut bytes: Vec<u8>) -> Vec<u8> {
        self.inner.js_xor_bytes(&mut bytes);
        bytes
    }

    pub fn xor_str(&mut self, text: String) { self.inner.xor_str(&text); }

    pub fn js_xor_str(&mut self, text: String) { self.inner.js_xor_str(&text); }

    pub fn encrypt_bytes(&mut self, mut bytes: Vec<u8>) -> Vec<u8> {
        self.inner.encrypt_bytes(&mut bytes);
        bytes
    }

    pub fn encrypt_bytes_no_change(&mut self, text: String) { self.inner.encrypt_bytes_no_change(&text); }

    pub fn decrypt_bytes(&mut self, mut bytes: Vec<u8>) -> Vec<u8> {
        self.inner.decrypt_bytes(&mut bytes);
        bytes
    }

    pub fn next_u8(&mut self) -> u8 { self.inner.next_u8() }

    pub fn next_i32(&mut self, max: i32) -> i32 { self.inner.next_i32(max) }

    pub fn round(&mut self, keys: Vec<u8>, round: Option<usize>) -> PyResult<()> {
        Self::ensure_non_empty_keys(&keys)?;
        self.inner.round(&keys, round);
        Ok(())
    }

    pub fn peek_next_u8(&self) -> u8 { self.inner.peek_next_u8() }

    pub fn c94(&mut self) -> bool { self.inner.c94() }

    pub fn c75(&mut self) -> bool { self.inner.c75() }

    pub fn c50(&mut self) -> bool { self.inner.c50() }

    pub fn c25(&mut self) -> bool { self.inner.c25() }

    pub fn c12(&mut self) -> bool { self.inner.c12() }

    pub fn c33(&mut self) -> bool { self.inner.c33() }

    pub fn c66(&mut self) -> bool { self.inner.c66() }

    #[allow(non_snake_case)]
    pub fn rFFFFFF(&mut self) -> u32 { self.inner.rFFFFFF() }

    #[allow(non_snake_case)]
    pub fn rFFFF(&mut self) -> u32 { self.inner.rFFFF() }

    pub fn r256(&mut self) -> u32 { self.inner.r256() }

    pub fn r64(&mut self) -> u32 { self.inner.r64() }

    pub fn r16(&mut self) -> u32 { self.inner.r16() }

    pub fn r255(&mut self) -> u32 { self.inner.r255() }

    pub fn r127(&mut self) -> u32 { self.inner.r127() }

    pub fn r63(&mut self) -> u32 { self.inner.r63() }

    pub fn r31(&mut self) -> u32 { self.inner.r31() }

    pub fn r15(&mut self) -> u32 { self.inner.r15() }

    pub fn r7(&mut self) -> u32 { self.inner.r7() }

    pub fn r3(&mut self) -> u32 { self.inner.r3() }

    pub fn r3x3(&mut self) -> u32 { self.inner.r3x3() }
}

impl From<RC4> for PyRC4 {
    fn from(value: RC4) -> Self { Self { inner: value } }
}

impl From<PyRC4> for RC4 {
    fn from(value: PyRC4) -> Self { value.inner }
}

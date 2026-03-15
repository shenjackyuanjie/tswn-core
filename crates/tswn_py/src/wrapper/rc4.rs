use pyo3::{PyResult, pyclass, pymethods};

use tswn_core::rc4::{RC4, VAL_LEN};

/// Python Wrapper for RC4
///
/// 很多麻烦事都是从这里来的
#[pyclass]
#[pyo3(name = "RC4")]
pub struct PyRC4 {
    pub inner: RC4,
}

#[pymethods]
impl PyRC4 {
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

    pub fn peek_next_u8(&self) -> u8 { self.inner.peek_next_u8() }
}

impl From<RC4> for PyRC4 {
    fn from(value: RC4) -> Self { Self { inner: value } }
}

impl From<PyRC4> for RC4 {
    fn from(value: PyRC4) -> Self { value.inner }
}

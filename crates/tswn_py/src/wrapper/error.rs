//! Runner 错误类型的 Python 封装。
//!
//! 将 [`RunnerError`](tswn_core::error::runner::RunnerError) 包装为继承自
//! `PyException` 的 Python 异常类 `RunnerError`，可在 Python 层直接 `except RunnerError` 捕获。

use pyo3::{PyErr, exceptions::PyException, pyclass};
use tswn_core::error::runner::RunnerError;

#[pyclass(extends=PyException)]
#[pyo3(name = "RunnerError")]
pub struct PyRunnerError {
    pub inner: RunnerError,
}
impl From<PyRunnerError> for PyErr {
    fn from(value: PyRunnerError) -> Self { PyErr::new::<PyRunnerError, _>(value.inner.to_string()) }
}

impl PyRunnerError {
    pub fn new(inner: RunnerError) -> Self { Self { inner } }
}

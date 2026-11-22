//! Runner 错误类型的 Python 封装。
//!
//! 将主 Runtime 构造错误包装为继承自 `PyException` 的 Python 异常类
//! `RunnerError`，可在 Python 层直接 `except RunnerError` 捕获。

use pyo3::{PyErr, exceptions::PyException, pyclass};
/// Python 侧 RunnerError：继承 Exception，承载 runner 错误消息。
#[pyclass(extends=PyException)]
#[pyo3(name = "RunnerError")]
pub struct PyRunnerError {
    pub message: String,
}
impl From<PyRunnerError> for PyErr {
    fn from(value: PyRunnerError) -> Self { PyErr::new::<PyRunnerError, _>(value.message) }
}

impl PyRunnerError {
    /// 由任意 Display 错误构造 RunnerError。
    pub fn new(error: impl std::fmt::Display) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}

#[pyo3::pymethods]
impl PyRunnerError {
    #[getter]
    fn code(&self) -> &'static str { tswn_core::cli_api::CliApiErrorCode::RunnerInitFailed.as_str() }
}

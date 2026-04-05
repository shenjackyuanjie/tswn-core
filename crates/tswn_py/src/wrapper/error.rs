use pyo3::{exceptions::PyException, pyclass, PyErr};
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

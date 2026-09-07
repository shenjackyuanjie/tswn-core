//! Thin Python adapter over the canonical core BattleSession and DTOs.
use crate::cli_api::map_cli_error;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use tswn_core::cli_api::{BattleOptions, BattleSession};

pub(crate) fn options(eval_rq: Option<f64>, include_icons: bool, max_rounds: Option<usize>) -> BattleOptions {
    let defaults = BattleOptions::default();
    BattleOptions {
        eval_rq: eval_rq.unwrap_or(defaults.eval_rq),
        include_icons,
        max_rounds: max_rounds.unwrap_or(defaults.max_rounds),
    }
}

pub(crate) fn dto_to_python<T: serde::Serialize + ?Sized>(py: Python<'_>, dto: &T) -> PyResult<Py<PyAny>> {
    let json = serde_json::to_string(dto)
        .map_err(|error| map_cli_error(tswn_core::cli_api::CliApiError::Internal(error.to_string())))?;
    Ok(PyModule::import(py, "json")?.getattr("loads")?.call1((json,))?.unbind())
}

#[pyclass(name = "BattleSession")]
pub struct PyBattleSession {
    inner: BattleSession,
}

#[pymethods]
impl PyBattleSession {
    #[new]
    #[pyo3(signature = (raw, eval_rq=None, include_icons=false, max_rounds=None))]
    fn new(raw: &str, eval_rq: Option<f64>, include_icons: bool, max_rounds: Option<usize>) -> PyResult<Self> {
        Ok(Self {
            inner: BattleSession::new(raw, options(eval_rq, include_icons, max_rounds)).map_err(map_cli_error)?,
        })
    }
    fn initial_states(&self, py: Python<'_>) -> PyResult<Py<PyAny>> { dto_to_python(py, self.inner.initial_states()) }
    fn current_states(&self, py: Python<'_>) -> PyResult<Py<PyAny>> { dto_to_python(py, self.inner.current_states()) }
    fn next_frame(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.inner
            .next_frame()
            .map_err(map_cli_error)?
            .map(|frame| dto_to_python(py, &frame))
            .transpose()
    }
    fn result(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.inner.result().map(|result| dto_to_python(py, &result)).transpose()
    }
    fn status(&self, py: Python<'_>) -> PyResult<Py<PyAny>> { dto_to_python(py, &self.inner.status()) }
    fn stop_reason(&self, py: Python<'_>) -> PyResult<Py<PyAny>> { dto_to_python(py, &self.inner.stop_reason()) }
    fn is_done(&self) -> bool { self.inner.is_done() }
    fn is_finished(&self) -> bool { self.inner.is_finished() }
    fn is_truncated(&self) -> bool { self.inner.is_truncated() }
    fn rounds_advanced(&self) -> usize { self.inner.rounds_advanced() }
    fn frames_emitted(&self) -> usize { self.inner.frames_emitted() }
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> { slf }
    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> { self.next_frame(py) }
}

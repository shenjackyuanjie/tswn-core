//! 类型 wrapper

use pyo3::{PyResult, pyclass, pymethods};
use tswn_core::{RunUpdate, RunUpdates, Runner, player::PlrId};

pub mod error;

/// Python Wrapper for Runner
#[pyclass]
#[pyo3(name = "Runner")]
pub struct PyRunner {
    pub inner: Runner,
}

#[pymethods]
impl PyRunner {
    #[staticmethod]
    fn new_from_namerena_raw(raw_str: String) -> PyResult<Self> {
        Ok(Self {
            inner: Runner::new_from_namerena_raw(raw_str).map_err(error::PyRunnerError::new)?,
        })
    }

    pub fn round_tick(&mut self, update: &mut PyRunUpdates) { self.inner.round_tick(&mut update.inner); }

    pub fn round_tick_new_update(&mut self) -> PyRunUpdates {
        let mut update = RunUpdates::new();
        self.inner.round_tick(&mut update);
        update.into()
    }

    pub fn run_to_completion(&mut self) -> bool { self.inner.run_to_completion() }
}

/// Python wrapper for RunUpdates
#[pyclass]
#[pyo3(name = "RunUpdates")]
#[derive(Default)]
pub struct PyRunUpdates {
    pub inner: RunUpdates,
}

#[pymethods]
impl PyRunUpdates {
    #[new]
    pub fn new() -> Self { Self::default() }

    pub fn clear(&mut self) {
        self.inner.updates.clear();
        self.inner.on_update_end.clear();
    }

    #[getter]
    pub fn get_id(&self) -> u64 { self.inner.id }

    #[getter]
    pub fn get_updates(&self) -> Vec<PyRunUpdate> { self.inner.updates.iter().cloned().map(|u| u.into()).collect() }
}

impl From<RunUpdates> for PyRunUpdates {
    fn from(value: RunUpdates) -> Self { Self { inner: value } }
}

impl From<PyRunUpdates> for RunUpdates {
    fn from(value: PyRunUpdates) -> Self { value.inner }
}

/// Python wrapper for RunUpdate
///
/// 你可以从这里获取到每一轮的更新内容
#[pyclass]
#[pyo3(name = "RunUpdate")]
pub struct PyRunUpdate {
    pub inner: RunUpdate,
}

impl From<RunUpdate> for PyRunUpdate {
    fn from(value: RunUpdate) -> Self { Self { inner: value } }
}

impl From<PyRunUpdate> for RunUpdate {
    fn from(value: PyRunUpdate) -> Self { value.inner }
}

#[pymethods]
impl PyRunUpdate {
    #[getter]
    pub fn get_score(&self) -> u32 { self.inner.score }

    #[getter]
    pub fn get_param(&self) -> Option<u32> { self.inner.param }

    #[getter]
    pub fn get_delay0(&self) -> i32 { self.inner.delay0 }

    #[getter]
    pub fn get_delay1(&self) -> i32 { self.inner.delay1 }

    #[getter]
    pub fn get_message(&self) -> String { self.inner.message.to_string() }

    #[getter]
    pub fn get_caster_id(&self) -> PlrId { self.inner.caster }

    #[getter]
    pub fn get_target_id(&self) -> PlrId { self.inner.target }

    #[getter]
    pub fn get_targets(&self) -> smallvec::SmallVec<[PlrId; 2]> { self.inner.targets.clone() }

    pub fn target_is_empty(&self) -> bool { self.inner.targets.is_empty() }

    pub fn get_update_type(&self) -> String { format!("{:?}", self.inner.update_type) }

    pub fn msg(&self) -> String { self.inner.msg() }
}

//! 类型 wrapper

use std::sync::Arc;

use pyo3::{PyResult, pyclass, pymethods};
use tswn_core::{
    RunUpdate, RunUpdates, Runner,
    engine::{storage::Storage, world_state::WorldState},
    player::PlrId,
};

pub mod error;
pub mod player;
pub mod rc4;

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

    /// 进行一轮更新
    pub fn round_tick(&mut self, update: &mut PyRunUpdates) { self.inner.round_tick(&mut update.inner); }

    /// 进行一轮更新，并返回更新内容
    pub fn round_tick_new_update(&mut self) -> PyRunUpdates {
        let mut update = RunUpdates::new();
        self.inner.round_tick(&mut update);
        update.into()
    }

    /// 运行到结束，返回是否有赢家
    pub fn run_to_completion(&mut self) -> bool { self.inner.run_to_completion() }

    /// 获取一个 Storage 的引用
    #[getter]
    pub fn get_storage(&self) -> PyStorage { self.inner.storage.clone().into() }

    /// 获取当前的 WorldState
    #[getter]
    pub fn get_world_state(&self) -> PyWorldState { self.inner.world.clone().into() }

    /// 获取当前的 rc4
    #[getter]
    pub fn get_rc4(&self) -> rc4::PyRC4 { self.inner.randomer.clone().into() }

    /// 是否有赢家
    ///
    /// 其实就是用的 world_state 的 have_winner 方法
    pub fn have_winner(&self) -> bool { self.inner.have_winner() }
}

/// World State 的 Python Wrapper
#[pyclass]
#[pyo3(name = "WorldState")]
pub struct PyWorldState {
    pub inner: WorldState,
}

#[pymethods]
impl PyWorldState {
    /// 获取当前轮次指针
    #[getter]
    pub fn get_round_pos(&self) -> i32 { self.inner.round_pos }

    /// 是否有赢家
    pub fn have_winner(&self) -> bool { self.inner.have_winner() }
}

impl From<WorldState> for PyWorldState {
    fn from(value: WorldState) -> Self { Self { inner: value } }
}

/// Python wrapper for Storage
///
/// 用来获取世界状态/获取玩家信息等
#[pyclass]
#[pyo3(name = "Storage")]
pub struct PyStorage {
    pub inner: Arc<Storage>,
}

#[pymethods]
impl PyStorage {
    /// 获取玩家
    pub fn get_player_by_id(&self, plr_id: PlrId) -> Option<player::PyPlayer> {
        self.inner.get_player(&plr_id).map(|p| p.clone().into())
    }

    #[getter]
    pub fn get_current_plr_id(&self) -> PlrId { self.inner.current_plr_id() as PlrId }
}

impl From<Arc<Storage>> for PyStorage {
    fn from(value: Arc<Storage>) -> Self { Self { inner: value } }
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

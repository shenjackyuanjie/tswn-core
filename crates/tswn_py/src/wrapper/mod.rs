//! Python 可见类型封装。
//!
//! 本模块把 `tswn_core` 的核心类型包成 `pyclass`：`Runner` 用于逐场对局，
//! `PreparedRunner` 用于复用解析结果跑批量胜率，子模块则分别封装 RC4、回放和错误类型。

use pyo3::{Py, PyAny, PyResult, Python, pyclass, pymethods};
use tswn_core::{
    PreparedRunner as CorePreparedRunner, RunUpdate, RunUpdates, Runner, runtime::EntityIdx, runtime::PlrId,
    runtime::update::UpdateType,
};

pub mod error;
pub mod rc4;
pub mod replay;

/// PreparedRunner 的 Python 封装
#[pyclass]
#[pyo3(name = "PreparedRunner")]
pub struct PyPreparedRunner {
    pub inner: CorePreparedRunner,
}

#[pymethods]
impl PyPreparedRunner {
    #[pyo3(signature = (n, eval_rq=None, thread=0))]
    pub fn win_rate(&self, n: usize, eval_rq: Option<f64>, thread: u32) -> PyResult<f64> {
        let eval_rq = eval_rq.unwrap_or(tswn_core::namerena::eval_name::WIN_RATE_EVAL_RQ);
        crate::run_prepared_win_rate(&self.inner, n, eval_rq, thread)
    }
}

impl From<CorePreparedRunner> for PyPreparedRunner {
    fn from(value: CorePreparedRunner) -> Self { Self { inner: value } }
}

/// Runner 的 Python 封装
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

    #[staticmethod]
    fn split_namerena_into_groups(raw_str: String) -> (Vec<Vec<String>>, Vec<String>) {
        Runner::split_namerena_into_groups(raw_str)
    }

    #[staticmethod]
    fn new_from_groups_with_seed(groups: Vec<Vec<String>>, seed: Vec<String>) -> PyResult<Self> {
        Ok(Self {
            inner: Runner::new_from_groups_with_seed(&groups, &seed).map_err(error::PyRunnerError::new)?,
        })
    }

    #[staticmethod]
    fn new_from_groups_with_seed_and_eval_rq(groups: Vec<Vec<String>>, seed: Vec<String>, eval_rq: f64) -> PyResult<Self> {
        Ok(Self {
            inner: Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, eval_rq).map_err(error::PyRunnerError::new)?,
        })
    }

    #[staticmethod]
    fn prepare_groups(groups: Vec<Vec<String>>) -> PyResult<PyPreparedRunner> {
        Runner::prepare_groups(&groups)
            .map(Into::into)
            .map_err(|err| error::PyRunnerError::new(err).into())
    }

    #[staticmethod]
    fn prepare_groups_with_eval_rq(groups: Vec<Vec<String>>, eval_rq: f64) -> PyResult<PyPreparedRunner> {
        Runner::prepare_groups_with_eval_rq(&groups, eval_rq)
            .map(Into::into)
            .map_err(|err| error::PyRunnerError::new(err).into())
    }

    #[staticmethod]
    fn new_from_prepared_with_seed(prepared: &PyPreparedRunner, seed: Vec<String>) -> PyResult<Self> {
        Ok(Self {
            inner: Runner::new_from_prepared_with_seed(&prepared.inner, &seed).map_err(error::PyRunnerError::new)?,
        })
    }

    /// 进行一个主回合（直到出现可见更新），并返回更新内容
    pub fn main_round(&mut self) -> PyRunUpdates { self.inner.main_round().into() }

    /// 运行到结束，返回是否有赢家
    pub fn run_to_completion(&mut self) -> bool { self.inner.run_binding_to_completion() }

    /// 返回原始输入顺序对应的队伍 roster（不受内部排序影响）
    #[getter]
    pub fn get_input_groups(&self) -> Vec<Vec<PlrId>> {
        self.inner
            .input_groups
            .iter()
            .map(|group| group.iter().map(|entity| entity.0 as PlrId).collect())
            .collect()
    }

    /// 查询指定玩家在原始输入中的队伍下标
    pub fn player_input_group_index(&self, player_id: PlrId) -> Option<usize> {
        let entity = EntityIdx(player_id.try_into().ok()?);
        self.inner.input_groups.iter().position(|group| group.contains(&entity))
    }

    /// 获取当前的 rc4
    #[getter]
    pub fn get_rc4(&self) -> rc4::PyRC4 { self.inner.runtime.rng.clone().into() }

    /// 是否有赢家
    pub fn have_winner(&self) -> bool { self.inner.have_winner() }

    /// 获取已经获胜的输入队伍索引。
    pub fn winner_team_index(&self) -> Option<usize> { self.inner.winner_team_index() }

    /// 获取所有已经获胜的输入队伍索引。
    pub fn winner_team_indices(&self) -> Vec<usize> { self.inner.winner_team_indices() }

    /// 获取当前所有运行时实体的标准快照。
    pub fn snapshot_players(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let snapshots = replay::snapshot_players(&self.inner);
        Ok(replay::snapshots_to_pylist(py, &snapshots)?.into_any().unbind())
    }

    /// 构建用于直播/回放的高层 timeline。
    #[pyo3(signature = (limit=None))]
    pub fn build_replay(&mut self, py: Python<'_>, limit: Option<usize>) -> PyResult<Py<PyAny>> {
        replay::build_replay(py, &mut self.inner, limit)
    }

    /// 获取所有存活玩家（扁平）
    pub fn alives_flat(&self) -> Vec<PlrId> { self.inner.alive_player_ids() }

    /// 获取所有存活玩家（按组）
    pub fn alives(&self) -> Vec<Vec<PlrId>> { self.inner.alive_player_groups() }

    /// 获取所有玩家 ID（包含已死亡）
    pub fn all_plrs(&self) -> Vec<PlrId> { self.inner.all_player_ids() }

    /// 获取玩家总数（包含已死亡）
    pub fn all_plr_len(&self) -> usize { self.inner.runtime.entities.len() }
}

/// RunUpdates 的 Python 封装
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

    #[staticmethod]
    pub fn new_no_capture() -> Self {
        Self {
            inner: RunUpdates::new_no_capture(),
        }
    }

    pub fn clear(&mut self) { self.inner.reset(); }

    pub fn reset(&mut self) { self.inner.reset(); }

    #[getter]
    pub fn get_id(&self) -> u64 { self.inner.id }

    #[getter]
    pub fn get_capture_updates(&self) -> bool { self.inner.capture_updates }

    #[getter]
    pub fn get_updates(&self) -> Vec<PyRunUpdate> { self.inner.updates.iter().cloned().map(|u| u.into()).collect() }

    #[getter]
    pub fn get_on_update_end(&self) -> Vec<PlrId> { self.inner.on_update_end.to_vec() }

    pub fn len(&self) -> usize { self.inner.updates.len() }

    pub fn is_empty(&self) -> bool { self.inner.updates.is_empty() }

    pub fn had_updates(&self) -> bool { self.inner.had_updates() }
}

impl From<RunUpdates> for PyRunUpdates {
    fn from(value: RunUpdates) -> Self { Self { inner: value } }
}

impl From<PyRunUpdates> for RunUpdates {
    fn from(value: PyRunUpdates) -> Self { value.inner }
}

/// RunUpdate 的 Python 封装
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

    pub fn is_win(&self) -> bool { self.inner.update_type == UpdateType::Win }

    pub fn is_none(&self) -> bool { self.inner.update_type == UpdateType::None }

    pub fn is_next_line(&self) -> bool { self.inner.update_type == UpdateType::NextLine }

    pub fn msg(&self) -> String { self.inner.msg() }

    #[pyo3(signature = (rendered=true))]
    pub fn to_dict(&self, py: Python<'_>, rendered: bool) -> PyResult<Py<PyAny>> {
        let names = std::collections::HashMap::new();
        let mut event = replay::update_to_dto(&self.inner, &names);
        if !rendered {
            event.message_rendered.clear();
        }
        Ok(replay::event_to_pydict(py, &event)?.into_any().unbind())
    }
}

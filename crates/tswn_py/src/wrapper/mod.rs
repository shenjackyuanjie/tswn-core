//! 类型 wrapper

use std::sync::Arc;

use pyo3::{pyclass, pymethods, PyResult};
use tswn_core::{
    engine::{storage::Storage, update::UpdateType, world_state::WorldState},
    player::PlrId,
    PreparedRunner as CorePreparedRunner, RunUpdate, RunUpdates, Runner,
};

pub mod error;
pub mod player;
pub mod rc4;

/// Python Wrapper for PreparedRunner
#[pyclass]
#[pyo3(name = "PreparedRunner")]
pub struct PyPreparedRunner {
    pub inner: CorePreparedRunner,
}

#[pymethods]
impl PyPreparedRunner {}

impl From<CorePreparedRunner> for PyPreparedRunner {
    fn from(value: CorePreparedRunner) -> Self { Self { inner: value } }
}

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

    /// 进行一轮更新
    pub fn round_tick(&mut self, update: &mut PyRunUpdates) { self.inner.round_tick(&mut update.inner); }

    /// 进行一轮更新，并返回更新内容
    pub fn round_tick_new_update(&mut self) -> PyRunUpdates {
        let mut update = RunUpdates::new();
        self.inner.round_tick(&mut update);
        update.into()
    }

    /// 进行一轮更新，但不采集详细更新帧（仅保留活动标记与 on_update_end 队列处理）。
    pub fn round_tick_new_update_no_capture(&mut self) -> PyRunUpdates {
        let mut update = RunUpdates::new_no_capture();
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

    /// 返回原始输入顺序对应的队伍 roster（不受内部排序影响）
    #[getter]
    pub fn get_input_groups(&self) -> Vec<Vec<PlrId>> { self.inner.input_groups.clone() }

    /// 获取当前的 rc4
    #[getter]
    pub fn get_rc4(&self) -> rc4::PyRC4 { self.inner.randomer.clone().into() }

    /// 是否有赢家
    ///
    /// 其实就是用的 world_state 的 have_winner 方法
    pub fn have_winner(&self) -> bool { self.inner.have_winner() }

    /// 获取所有存活玩家（扁平）
    pub fn alives_flat(&self) -> Vec<PlrId> { self.inner.alives_flat() }

    /// 获取所有存活玩家（按组）
    pub fn alives(&self) -> Vec<Vec<PlrId>> { self.inner.alives() }

    /// 获取所有玩家 ID（包含已死亡）
    pub fn all_plrs(&self) -> Vec<PlrId> { self.inner.all_plrs() }

    /// 获取玩家总数（包含已死亡）
    pub fn all_plr_len(&self) -> usize { self.inner.all_plr_len() }
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

    /// 当前行动顺序中的玩家列表（仅存活）
    #[getter]
    pub fn get_players(&self) -> Vec<PlrId> { self.inner.players.clone() }

    /// 胜者阵容（若有）
    #[getter]
    pub fn get_winner(&self) -> Option<Vec<PlrId>> { self.inner.winner.clone() }

    /// 是否有赢家
    pub fn have_winner(&self) -> bool { self.inner.have_winner() }

    /// 全部玩家（包含已死亡）
    pub fn all_plrs(&self) -> Vec<PlrId> { self.inner.all_plrs() }

    /// 全部玩家数量（包含已死亡）
    pub fn all_plr_len(&self) -> usize { self.inner.all_plr_len() }

    /// 阵容（队伍）数量
    pub fn roster_count(&self) -> usize { self.inner.roster_count() }

    /// 查询玩家所属队伍下标
    pub fn team_index_of(&self, actor: PlrId) -> Option<usize> { self.inner.team_index_of(actor) }

    /// 获取队伍全员 roster
    pub fn team_roster(&self, team_idx: usize) -> Option<Vec<PlrId>> { self.inner.team_roster(team_idx).map(|v| v.to_vec()) }

    /// 获取队伍存活列表
    pub fn team_alive(&self, team_idx: usize) -> Option<Vec<PlrId>> { self.inner.team_alive(team_idx).map(|v| v.to_vec()) }

    /// 某玩家当前是否存活
    pub fn contains_alive(&self, plr_id: PlrId) -> bool { self.inner.contains_alive(plr_id) }

    /// 获取指定队伍的 winner roster 快照
    pub fn winner_roster(&self, team_idx: usize) -> Option<Vec<PlrId>> { self.inner.winner_roster(team_idx) }
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

    /// 获取玩家（若尚未同步入 players，则尝试从 pending_spawns 查询）
    pub fn get_player_or_pending_by_id(&self, plr_id: PlrId) -> Option<player::PyPlayer> {
        self.inner.get_player_or_pending(&plr_id).map(|p| p.clone().into())
    }

    /// 从 pending_spawns 中按 ID 查询玩家
    pub fn get_pending_spawn_player_by_id(&self, plr_id: PlrId) -> Option<player::PyPlayer> {
        self.inner.get_pending_spawn_player(plr_id).map(|p| p.clone().into())
    }

    /// 按队伍索引获取 roster
    pub fn get_group(&self, group_id: usize) -> Option<Vec<PlrId>> { self.inner.get_group(group_id).cloned() }

    /// 获取包含某玩家的 roster
    pub fn group_containing(&self, actor: PlrId) -> Option<Vec<PlrId>> { self.inner.group_containing(actor).cloned() }

    /// 查询某玩家所在队伍索引
    pub fn group_index_of(&self, actor: PlrId) -> Option<usize> { self.inner.group_index_of(actor) }

    /// 获取包含某玩家的存活组
    pub fn alive_group_containing(&self, actor: PlrId) -> Option<Vec<PlrId>> { self.inner.alive_group_containing(actor).cloned() }

    /// 按某玩家所在队伍返回其存活组（玩家本身可死亡）
    pub fn alive_group_at_team_of(&self, actor: PlrId) -> Option<Vec<PlrId>> { self.inner.alive_group_at_team_of(actor).cloned() }

    /// 所有存活玩家 ID（扁平）
    pub fn all_alive_ids(&self) -> Vec<PlrId> { self.inner.all_alive_ids() }

    /// 所有玩家 ID（包含已死亡）
    pub fn all_player_ids(&self) -> Vec<PlrId> { self.inner.all_player_ids() }

    /// 当前待同步的召唤数量
    #[getter]
    pub fn get_pending_spawn_count(&self) -> usize { self.inner.pending_spawn_count() }

    /// 指定 owner 的待同步召唤数量
    pub fn pending_spawn_count_for_owner(&self, owner: PlrId) -> usize { self.inner.pending_spawn_count_for_owner(owner) }

    /// 指定 owner 的待同步召唤 ID 列表
    pub fn pending_spawn_ids_for_owner(&self, owner: PlrId) -> Vec<PlrId> { self.inner.pending_spawn_ids_for_owner(owner) }

    /// 指定队员集合对应 owner 的待同步召唤 ID 列表
    pub fn pending_spawn_ids_for_group(&self, group_members: Vec<PlrId>) -> Vec<PlrId> {
        self.inner.pending_spawn_ids_for_group(&group_members)
    }

    /// 当前 alive group 数量（仅非空组）
    #[getter]
    pub fn get_alive_group_count(&self) -> usize { self.inner.alive_group_count() }

    /// 是否存在待同步运行期实体变更
    #[getter]
    pub fn get_needs_sync(&self) -> bool { self.inner.needs_sync() }

    #[getter]
    pub fn get_current_plr_id(&self) -> PlrId { self.inner.current_plr_id() as PlrId }

    #[getter]
    pub fn get_eval_rq(&self) -> f64 { self.inner.eval_rq() }
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

    pub fn is_win(&self) -> bool { self.inner.update_type == UpdateType::Win }

    pub fn is_none(&self) -> bool { self.inner.update_type == UpdateType::None }

    pub fn is_next_line(&self) -> bool { self.inner.update_type == UpdateType::NextLine }

    pub fn msg(&self) -> String { self.inner.msg() }
}

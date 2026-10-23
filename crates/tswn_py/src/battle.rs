//! 规范 core BattleSession 和 DTO 之上的轻量 Python 适配器。
//!
//! 会话独占运行时，raw 只在构造时读取；DTO 经 JSON 转为新的 Python dict/list，
//! 不暴露 Rust 借用。领域错误由 map_cli_error 转换，JSON/Python 对象转换也可能失败；
//! next_frame 已推进后发生转换错误不会回滚运行时，因此不要据此假定该帧可重试。
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
/// 可迭代的同步战斗会话，返回 core 规范 DTO 的 dict/list 副本。
///
/// Python 管理对象释放，无需显式 free；迭代耗尽与失败不同，Runtime 失败会抛出
/// 带 code 的异常并保留 sticky failure，此后停止推进并丢弃会话。
pub struct PyBattleSession {
    inner: BattleSession,
}

#[pymethods]
impl PyBattleSession {
    /// 解析空行分队的 raw；max_rounds 为正数回合预算，None 使用 core 默认值。
    /// include_icons 会把图标 data URL 加入快照，eval_rq 仅在创建时确定。
    #[new]
    #[pyo3(signature = (raw, eval_rq=None, include_icons=false, max_rounds=None))]
    fn new(raw: &str, eval_rq: Option<f64>, include_icons: bool, max_rounds: Option<usize>) -> PyResult<Self> {
        Ok(Self {
            inner: BattleSession::new(raw, options(eval_rq, include_icons, max_rounds)).map_err(map_cli_error)?,
        })
    }
    /// 返回创建时快照的新副本；修改 Python 结果不会修改会话。
    fn initial_states(&self, py: Python<'_>) -> PyResult<Py<PyAny>> { dto_to_python(py, self.inner.initial_states()) }
    /// 返回当前回合边界的快照副本，不推进运行时。
    fn current_states(&self, py: Python<'_>) -> PyResult<Py<PyAny>> { dto_to_python(py, self.inner.current_states()) }
    /// 推进到下一可见帧；正常终止返回 None，Runtime 失败抛异常而不是 StopIteration。
    fn next_frame(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.inner
            .next_frame()
            .map_err(map_cli_error)?
            .map(|frame| dto_to_python(py, &frame))
            .transpose()
    }
    /// 正常结束或截断后返回结果副本；运行中/失败时为 None，重复读取不消费结果。
    fn result(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.inner.result().map(|result| dto_to_python(py, &result)).transpose()
    }
    fn status(&self, py: Python<'_>) -> PyResult<Py<PyAny>> { dto_to_python(py, &self.inner.status()) }
    fn stop_reason(&self, py: Python<'_>) -> PyResult<Py<PyAny>> { dto_to_python(py, &self.inner.stop_reason()) }
    /// 只表示正常结束或截断；Runtime 失败时仍为 false，应同时检查 is_failed。
    fn is_done(&self) -> bool { self.inner.is_done() }
    fn is_failed(&self) -> bool { self.inner.is_failed() }
    fn is_finished(&self) -> bool { self.inner.is_finished() }
    fn is_truncated(&self) -> bool { self.inner.is_truncated() }
    fn rounds_advanced(&self) -> usize { self.inner.rounds_advanced() }
    fn frames_emitted(&self) -> usize { self.inner.frames_emitted() }
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> { slf }
    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> { self.next_frame(py) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_session_exposes_real_sticky_runtime_failure() {
        Python::initialize();
        Python::attach(|py| {
            let mut session = PyBattleSession::new("alpha@red+bed2[3000]\n\nbeta@blue", None, false, None).unwrap();
            session.inner.invalidate_runtime_for_test();
            let object = Py::new(py, session).unwrap();
            let object = object.bind(py);
            assert!(!object.call_method0("is_failed").unwrap().extract::<bool>().unwrap());
            let mut previous_error = None;
            for _ in 0..2 {
                let error = object.call_method0("next_frame").unwrap_err();
                let code = error.value(py).getattr("code").unwrap().extract::<String>().unwrap();
                assert_eq!(code, "RUNTIME_FAILED");
                let current = (code, error.to_string());
                if let Some(previous) = &previous_error {
                    assert_eq!(&current, previous);
                }
                previous_error = Some(current);
                assert!(object.call_method0("is_failed").unwrap().extract::<bool>().unwrap());
                assert!(!object.call_method0("is_done").unwrap().extract::<bool>().unwrap());
                assert!(object.call_method0("result").unwrap().is_none());
                assert!(object.call_method0("stop_reason").unwrap().is_none());
                assert_eq!(object.call_method0("status").unwrap().extract::<String>().unwrap(), "running");
            }
        });
    }
}

//! tswn-py — tswn-core 的 Python 绑定。
//!
//! 通过 `pyo3` 暴露战斗引擎、预构建胜率计算、玩家/RC4 包装类型以及图标渲染函数。
//! Python 侧接口尽量保持直接：复杂对局仍由 `Runner` / `PreparedRunner` 承担，
//! 批量胜率和图标输出则提供便于脚本调用的顶层函数。

/// 类型 wrapper
pub mod cli_api;
pub mod wrapper;

use pyo3::{
    Bound, Py, PyAny, PyRef, PyResult, Python,
    exceptions::PyValueError,
    pyfunction, pymodule,
    types::{PyDictMethods, PyList, PyListMethods, PyModule, PyModuleMethods},
    wrap_pyfunction,
};
use tswn_core::{PreparedRunner, Runner};

fn ensure_win_rate_group_count(groups: &[Vec<String>]) -> PyResult<()> {
    let group_count = groups.iter().filter(|g| !g.is_empty()).count();
    if group_count < 2 {
        Err(PyValueError::new_err("win_rate requires at least two non-empty groups"))
    } else {
        Ok(())
    }
}

pub fn run_prepared_win_rate(prepared: &PreparedRunner, n: usize, eval_rq: f64, thread: u32) -> PyResult<f64> {
    let summary =
        tswn_core::win_rate::prepared_win_rate(prepared, n, eval_rq, thread).map_err(wrapper::error::PyRunnerError::new)?;
    Ok(summary.win_rate_percent())
}

/// tswn-py 的版本字符串
#[pyfunction]
fn wrapper_version_str() -> String { env!("CARGO_PKG_VERSION").to_string() }

/// tswn-core 的版本字符串
#[pyfunction]
fn core_version_str() -> String { tswn_core::version().to_string() }

/// 根据玩家名称生成 PNG 图标的 Base64 编码字符串
#[pyfunction]
fn name_to_png_base64(name: String) -> String { tswn_core::player::icon_render::render_icon_b64_from_name(&name) }

/// 根据玩家名称生成 PNG 图标的字节数据
#[pyfunction]
fn name_to_png_bytes(name: String) -> Vec<u8> { tswn_core::player::icon_render::render_icon_png_from_name(&name) }

/// 根据玩家名称生成 16x16 RGBA 图标像素数据
#[pyfunction]
fn name_to_icon_rgba(name: String) -> Vec<u8> { tswn_core::player::icon_render::render_icon_vec_from_name(&name) }

/// 以 CLI 默认语义计算第一组对其余组的胜率（百分比）
#[pyfunction(signature = (raw, n, eval_rq=None, thread=0))]
fn win_rate(raw: String, n: usize, eval_rq: Option<f64>, thread: u32) -> PyResult<f64> {
    let eval_rq = eval_rq.unwrap_or(tswn_core::player::eval_name::WIN_RATE_EVAL_RQ);
    let groups = Runner::split_namerena_into_groups(raw).0;
    ensure_win_rate_group_count(&groups)?;
    let prepared = Runner::prepare_groups_with_eval_rq(&groups, eval_rq).map_err(wrapper::error::PyRunnerError::new)?;
    run_prepared_win_rate(&prepared, n, eval_rq, thread)
}

/// 以 CLI 默认语义批量计算 target 对多个 opponent 的胜率（百分比）
#[pyfunction(signature = (target, against, n, eval_rq=None, thread=0))]
fn group_win_rate(
    target: String,
    against: Vec<String>,
    n: usize,
    eval_rq: Option<f64>,
    thread: u32,
) -> PyResult<Vec<(String, f64)>> {
    let eval_rq = eval_rq.unwrap_or(tswn_core::player::eval_name::WIN_RATE_EVAL_RQ);
    let mut results = Vec::with_capacity(against.len());
    for opponent in against {
        let raw = format!("{target}\n\n{opponent}");
        let rate = win_rate(raw, n, Some(eval_rq), thread)?;
        results.push((opponent, rate));
    }
    Ok(results)
}

/// 基于 PreparedRunner 以 CLI 默认语义计算第一组对其余组的胜率（百分比）
#[pyfunction(signature = (prepared, n, eval_rq=None, thread=0))]
fn prepared_win_rate(
    prepared: PyRef<'_, wrapper::PyPreparedRunner>,
    n: usize,
    eval_rq: Option<f64>,
    thread: u32,
) -> PyResult<f64> {
    let eval_rq = eval_rq.unwrap_or(tswn_core::player::eval_name::WIN_RATE_EVAL_RQ);
    run_prepared_win_rate(&prepared.inner, n, eval_rq, thread)
}

/// Compute show.html-compatible per-event delays for a list of RunUpdate objects.
#[pyfunction(signature = (updates, player_count, scale=true))]
fn compute_show_timeline(
    py: Python<'_>,
    updates: Vec<PyRef<'_, wrapper::PyRunUpdate>>,
    player_count: usize,
    scale: bool,
) -> PyResult<Py<PyAny>> {
    let names = std::collections::HashMap::new();
    let events = updates
        .iter()
        .map(|update| wrapper::replay::update_to_dto(&update.inner, &names))
        .collect::<Vec<_>>();
    let delays = wrapper::replay::compute_event_delays(&events, scale, player_count, false);
    let result = PyList::empty(py);

    for (idx, event) in events.iter().enumerate() {
        let item = pyo3::types::PyDict::new(py);
        item.set_item("event", wrapper::replay::event_to_pydict(py, event)?)?;
        item.set_item("delay_ms", *delays.get(idx).unwrap_or(&0))?;
        item.set_item("row_break", event.is_next_line)?;
        result.append(item)?;
    }

    Ok(result.into_any().unbind())
}

/// tswn-py
#[pymodule]
#[pyo3(name = "tswn_py")]
fn module_init(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("DEFAULT_EVAL_RQ", tswn_core::player::eval_name::DEFAULT_EVAL_RQ)?;
    m.add("WIN_RATE_EVAL_RQ", tswn_core::player::eval_name::WIN_RATE_EVAL_RQ)?;
    m.add_function(wrap_pyfunction!(wrapper_version_str, m)?)?;
    m.add_function(wrap_pyfunction!(core_version_str, m)?)?;
    m.add_function(wrap_pyfunction!(name_to_icon_rgba, m)?)?;
    m.add_function(wrap_pyfunction!(name_to_png_base64, m)?)?;
    m.add_function(wrap_pyfunction!(name_to_png_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(win_rate, m)?)?;
    m.add_function(wrap_pyfunction!(group_win_rate, m)?)?;
    m.add_function(wrap_pyfunction!(prepared_win_rate, m)?)?;
    m.add_function(wrap_pyfunction!(compute_show_timeline, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::win_rate_summary, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::team_win_rate_summary, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::group_win_rate_summary, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::score, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::namer_pf, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::batch_rate, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::pair_rate, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::to_diy, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::to_diy_batch, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::icon_info, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::parse_group_lines, m)?)?;
    m.add_class::<cli_api::PyWinRateResult>()?;
    m.add_class::<cli_api::PyScoreResult>()?;
    m.add_class::<cli_api::PyNamerPfResult>()?;
    m.add_class::<cli_api::PyBatchRateResult>()?;
    m.add_class::<cli_api::PyPairRateResult>()?;
    m.add_class::<cli_api::PyIconInfo>()?;
    m.add_class::<wrapper::PyRunner>()?;
    m.add_class::<wrapper::PyPreparedRunner>()?;
    m.add_class::<wrapper::PyWorldState>()?;
    m.add_class::<wrapper::PyStorage>()?;
    m.add_class::<wrapper::PyRunUpdate>()?;
    m.add_class::<wrapper::PyRunUpdates>()?;
    m.add_class::<wrapper::player::PyPlayer>()?;
    m.add_class::<wrapper::rc4::PyRC4>()?;
    m.add_class::<wrapper::error::PyRunnerError>()?;
    Ok(())
}

//! tswn-py
//!
//! 某个满是怨念的人向你问好

/// 类型 wrapper
pub mod wrapper;

use pyo3::{
    Bound, PyRef, PyResult,
    exceptions::PyValueError,
    pyfunction, pymodule,
    types::{PyModule, PyModuleMethods},
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

pub(crate) fn run_prepared_win_rate(prepared: &PreparedRunner, n: usize, eval_rq: f64, thread: u32) -> PyResult<f64> {
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

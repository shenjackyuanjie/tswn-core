//! Python / PyO3 适配层，通过 Python 数据与对象接口复用 core 规则。
//!
//! # Python 使用入口
//!
//! | 需求 | Python 接口 | 数据形式 |
//! | --- | --- | --- |
//! | 逐帧战斗 | `tswn_py.BattleSession` | 可迭代会话，dict / list 规范 DTO |
//! | 完整回放与 CLI 工具 | `tswn_py.battle_replay` 等顶层函数 | 高层结果与稳定错误 code |
//! | 批量复用阵容 | `Runner` / `PreparedRunner`、`win_rate` | 同步计算；百分比胜率 |
//! | 机制诊断与随机快照 | [`wrapper`] | 低层 pyclass 封装 |
//! | 图标 | `name_to_png_bytes` / `name_to_png_base64` | PNG 数据或 data URL |
//!
//! ```python
//! import tswn_py
//!
//! session = tswn_py.BattleSession("left@red\n\nright@blue")
//! initial = session.initial_states()
//! for frame in session:
//!     print(frame["frame_index"], frame["round_index"])
//! result = session.result()
//! ```
//!
//! 会话 DTO 由 core 序列化后转为 Python 数据，类型声明位于 Python 包的 `.pyi`。
//! `next_frame()` 结束返回 `None`；Runtime 错误抛出异常并保留 sticky failure，
//! 此时 `is_failed()` 为真、`result()` 为 None，调用方沿异常通道处理。
//! 预构建的 `eval_rq` 保持固定，换口径时重新准备。Rust 端的 [`run_prepared_win_rate`] 是绑定辅助，
//! 普通 Rust 调用方应直接使用 core。
//!
//! # 生命周期与执行边界
//!
//! Python 对象由引用计数/垃圾回收管理，调用方无需 C 风格 free；返回的 dict/list、
//! 更新对象和随机数快照均为独立数据，修改它们不会修改原会话。计算函数同步执行，
//! 本层未主动调用 PyO3 detach 让出解释器；大批任务应由宿主安排后台执行。
//! 业务失败通过带稳定 `code` 的异常上报；Rust panic 不在本层映射为业务错误，
//! 不能将 PyO3 的 panic 异常视为可继续推进同一对象的信号。
//!
//! 安装、setuptools-rust 构建和完整 Python 签名见
//! [Python 绑定说明](https://github.com/shenjackyuanjie/tswn-core/tree/main/crates/tswn_py)。
//! Cargo doc 用于阅读 Rust 适配实现，Python 签名由 `.pyi` 与运行时 help 提供。

mod battle;
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

/// 同步运行预构建阵容的胜率批任务，返回 0～100 百分比，而非 0～1 比值。
///
/// 模板只被共享借用，评估口径由创建时固定；`n` 是试验次数，`thread=0/1/n`
/// 表示自动/单线程/指定线程。core 失败转换为 Python `RunnerError`，不返回部分统计。
pub fn run_prepared_win_rate(prepared: &PreparedRunner, n: usize, thread: u32) -> PyResult<f64> {
    let summary =
        tswn_core::runtime::prepared_runtime_win_rate(prepared, n, thread).map_err(wrapper::error::PyRunnerError::new)?;
    Ok(summary.win_rate_percent())
}

/// tswn-py 的版本字符串
#[pyfunction]
fn wrapper_version_str() -> String { env!("CARGO_PKG_VERSION").to_string() }

/// tswn-core 的版本字符串
#[pyfunction]
fn core_version_str() -> String { tswn_core::version().to_string() }

/// 返回 `data:image/png;base64,...` 图标文本，可直接用作图片 URL，不是裸 Base64。
#[pyfunction]
fn name_to_png_base64(name: String) -> String { tswn_core::namerena::icon_render::render_icon_b64_from_name(&name) }

/// 返回完整 PNG 文件的数据序列；Python 可用 `bytes(...)` 转换后写文件。
#[pyfunction]
fn name_to_png_bytes(name: String) -> Vec<u8> { tswn_core::namerena::icon_render::render_icon_png_from_name(&name) }

/// 返回按行排列的 16×16 RGBA 像素数据序列，共 1024 个字节，不是 PNG 文件。
#[pyfunction]
fn name_to_icon_rgba(name: String) -> Vec<u8> { tswn_core::namerena::icon_render::render_icon_vec_from_name(&name) }

/// 按空行分队，以第一组为统计对象同步计算百分比胜率。
///
/// 至少需两个非空队伍；`eval_rq=None` 使用胜率口径，固定输入种子不用于批任务。
/// `n` 为试验次数，`thread=0/1/n` 表示自动/单线程/指定线程，失败抛出异常。
#[pyfunction(signature = (raw, n, eval_rq=None, thread=0))]
fn win_rate(raw: String, n: usize, eval_rq: Option<f64>, thread: u32) -> PyResult<f64> {
    let eval_rq = eval_rq.unwrap_or(tswn_core::namerena::eval_name::WIN_RATE_EVAL_RQ);
    let groups = Runner::split_namerena_into_groups(raw).0;
    ensure_win_rate_group_count(&groups)?;
    let prepared = Runner::prepare_groups_with_eval_rq(&groups, eval_rq).map_err(wrapper::error::PyRunnerError::new)?;
    run_prepared_win_rate(&prepared, n, thread)
}

/// 按 against 输入顺序返回 `(opponent, 百分比胜率)` 列表。
///
/// target 和每个 opponent 各自是一个多行队伍；逐个同步计算，空 against 返回空列表，
/// 任一对局失败会抛出异常，不返回已完成的列表前缀。
#[pyfunction(signature = (target, against, n, eval_rq=None, thread=0))]
fn group_win_rate(
    target: String,
    against: Vec<String>,
    n: usize,
    eval_rq: Option<f64>,
    thread: u32,
) -> PyResult<Vec<(String, f64)>> {
    let eval_rq = eval_rq.unwrap_or(tswn_core::namerena::eval_name::WIN_RATE_EVAL_RQ);
    let mut results = Vec::with_capacity(against.len());
    for opponent in against {
        let raw = format!("{target}\n\n{opponent}");
        let rate = win_rate(raw, n, Some(eval_rq), thread)?;
        results.push((opponent, rate));
    }
    Ok(results)
}

/// 使用 PreparedRunner 固定的评估口径计算百分比胜率，不会换成全局胜率默认值。
///
/// 显式传入 `eval_rq` 时必须与模板创建值按浮点位模式相同，否则抛出 `ValueError`。
/// 只借用模板、不改变其状态；返回值与 `PreparedRunner.win_rate` 一致。
#[pyfunction(signature = (prepared, n, eval_rq=None, thread=0))]
fn prepared_win_rate(
    prepared: PyRef<'_, wrapper::PyPreparedRunner>,
    n: usize,
    eval_rq: Option<f64>,
    thread: u32,
) -> PyResult<f64> {
    if let Some(eval_rq) = eval_rq
        && eval_rq.to_bits() != prepared.eval_rq.to_bits()
    {
        return Err(PyValueError::new_err("eval_rq is fixed when the PreparedRunner is created"));
    }
    run_prepared_win_rate(&prepared.inner, n, thread)
}

/// 为 RunUpdate 列表生成独立的事件字典、毫秒延迟与换行标记列表。
///
/// `player_count` 用于人数缩放，`scale=false` 保留原始等待；输入更新不被修改，
/// 返回对象不借用原列表，不需要推进 runner 或保留它的生命周期。
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

/// 注册 Python 扩展的顶层函数、结果类型及异常；高级函数不是独立 Python 子模块。
#[pymodule]
#[pyo3(name = "tswn_py")]
fn module_init(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("DEFAULT_EVAL_RQ", tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ)?;
    m.add("WIN_RATE_EVAL_RQ", tswn_core::namerena::eval_name::WIN_RATE_EVAL_RQ)?;
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
    m.add_function(wrap_pyfunction!(cli_api::batch_rate_factored, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::pair_rate, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::pair_rate_factored, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::to_diy, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::to_diy_batch, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::icon_info, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::parse_group_lines, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::default_custom_runtime_normalized_run, m)?)?;
    m.add_function(wrap_pyfunction!(cli_api::battle_replay, m)?)?;
    m.add_class::<battle::PyBattleSession>()?;
    m.add_class::<cli_api::PyInvalidArgumentError>()?;
    m.add_class::<cli_api::PyUnsupportedOptionError>()?;
    m.add_class::<cli_api::PyInternalError>()?;
    m.add_class::<cli_api::PyWinRateResult>()?;
    m.add_class::<cli_api::PyScoreResult>()?;
    m.add_class::<cli_api::PyNamerPfResult>()?;
    m.add_class::<cli_api::PyBatchRateResult>()?;
    m.add_class::<cli_api::PyPairRateResult>()?;
    m.add_class::<cli_api::PyIconInfo>()?;
    m.add_class::<wrapper::PyRunner>()?;
    m.add_class::<wrapper::PyPreparedRunner>()?;
    m.add_class::<wrapper::PyRunUpdate>()?;
    m.add_class::<wrapper::PyRunUpdates>()?;
    m.add_class::<wrapper::rc4::PyRC4>()?;
    m.add_class::<wrapper::error::PyRunnerError>()?;
    m.add_class::<cli_api::PyInvalidInputError>()?;
    m.add_class::<cli_api::PyCliRuntimeError>()?;
    Ok(())
}

//! tswn-py
//!
//! 某个满是怨念的人向你问好

/// 类型 wrapper
pub mod wrapper;

use pyo3::{
    pyfunction, pymodule,
    types::{PyModule, PyModuleMethods},
    wrap_pyfunction, Bound, PyResult,
};

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

/// tswn-py
#[pymodule]
#[pyo3(name = "tswn_py")]
fn module_init(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(wrapper_version_str, m)?)?;
    m.add_function(wrap_pyfunction!(core_version_str, m)?)?;
    m.add_function(wrap_pyfunction!(name_to_png_base64, m)?)?;
    m.add_function(wrap_pyfunction!(name_to_png_bytes, m)?)?;
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

use pyo3::{pyclass, pymethods};
use tswn_core::player::Player;

#[pyclass]
#[pyo3(name = "Player")]
pub struct PyPlayer {
    pub inner: Player,
}

#[pymethods]
impl PyPlayer {
    #[getter]
    pub fn get_id(&self) -> u64 { self.inner.id() }

    #[getter]
    pub fn get_name_factor(&self) -> f64 { self.inner.get_name_factor() }

    #[getter]
    pub fn get_id_name(&self) -> String { self.inner.id_name() }

    #[getter]
    pub fn get_id_key_name(&self) -> String { self.inner.id_key_name() }

    #[getter]
    pub fn get_display_name(&self) -> String { self.inner.display_name() }

    #[getter]
    pub fn get_clan_name(&self) -> String { self.inner.clan_name() }

    #[getter]
    pub fn get_base_name(&self) -> String { self.inner.base_name() }

    pub fn __str__(&self) -> String { self.inner.to_string() }
}

impl From<Player> for PyPlayer {
    fn from(value: Player) -> Self { Self { inner: value } }
}

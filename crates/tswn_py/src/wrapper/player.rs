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
    pub fn get_ptr(&self) -> usize { self.inner.ptr() }

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

    #[getter]
    pub fn get_weapon_name(&self) -> Option<String> { self.inner.get_weapon_name().map(ToString::to_string) }

    #[getter]
    pub fn get_player_type(&self) -> String { format!("{:?}", self.inner.player_type()) }

    #[getter]
    pub fn get_sort_int(&self) -> i32 { self.inner.get_sort_int() }

    #[getter]
    pub fn get_move_point(&self) -> i32 { self.inner.move_point() }

    #[getter]
    pub fn get_magic_point(&self) -> i32 { self.inner.magic_point() }

    #[getter]
    pub fn get_hp(&self) -> i32 { self.inner.get_status().hp }

    #[getter]
    pub fn get_max_hp(&self) -> i32 { self.inner.get_status().max_hp }

    #[getter]
    pub fn get_attack(&self) -> i32 { self.inner.get_status().attack }

    #[getter]
    pub fn get_defense(&self) -> i32 { self.inner.get_status().defense }

    #[getter]
    pub fn get_speed(&self) -> i32 { self.inner.get_status().speed }

    #[getter]
    pub fn get_agility(&self) -> i32 { self.inner.get_status().agility }

    #[getter]
    pub fn get_magic(&self) -> i32 { self.inner.get_status().magic }

    #[getter]
    pub fn get_resistance(&self) -> i32 { self.inner.get_status().resistance }

    #[getter]
    pub fn get_wisdom(&self) -> i32 { self.inner.get_status().wisdom }

    #[getter]
    pub fn get_point(&self) -> u32 { self.inner.get_status().point }

    #[getter]
    pub fn get_frozen(&self) -> bool { self.inner.get_status().frozen }

    #[getter]
    pub fn get_at_boost(&self) -> f64 { self.inner.get_status().at_boost }

    #[getter]
    pub fn get_attract(&self) -> f64 { self.inner.get_status().attract }

    #[getter]
    pub fn get_attr_sum(&self) -> i32 { self.inner.attr_sum() }

    #[getter]
    pub fn get_atk_sum(&self) -> i32 { self.inner.get_status().atk_sum }

    #[getter]
    pub fn get_all_sum(&self) -> u32 { self.inner.get_status().all_sum }

    #[getter]
    pub fn get_negative_state_count(&self) -> usize { self.inner.negative_state_count() }

    pub fn active(&self) -> bool { self.inner.active() }

    pub fn alive(&self) -> bool { self.inner.alive() }

    pub fn check_move(&self) -> bool { self.inner.check_move() }

    pub fn __str__(&self) -> String { self.inner.to_string() }
}

impl From<Player> for PyPlayer {
    fn from(value: Player) -> Self { Self { inner: value } }
}

pub struct Weapon {
    weapon_type: WeaponType,
}

impl Weapon {
    pub fn new(weapon_type: WeaponType) -> Self { Self { weapon_type } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WeaponType {}

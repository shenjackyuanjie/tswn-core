pub mod action_targets;
pub mod boss;
pub mod eval_name;
pub mod icon;
pub mod skill;
pub mod state;
pub mod status;
pub mod utils;
pub mod weapons;

mod impl_attr;
mod impl_ctor;
mod impl_runtime;

pub use action_targets::*;
pub use state::*;
pub use status::*;

use std::cmp::{Ordering, min};
use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::error::player::{PlayerError, PlayerResult};
use crate::player::skill::{Skill, SkillTargetDomain, store::SkillStorage};
use crate::rc4::RC4;

pub const NAME_MAX_LEN: usize = 256;
pub const TEAM_MAX_LEN: usize = 256;
pub const MOVE_POINT_THRESHOLD: i32 = 2048;

pub type PlrId = usize;

pub type OnDamageFunc = fn(PlrId, PlrId, i32, &mut RC4, &mut RunUpdates, &Arc<Storage>);

pub fn noop_on_damage(
    _caster: PlrId,
    _target: PlrId,
    _dmg: i32,
    _r: &mut RC4,
    _updates: &mut RunUpdates,
    _storage: &Arc<Storage>,
) {
}

pub fn player_id_as_mut_plr(ptr: PlrId, storage: &Arc<Storage>) -> &mut Player {
    storage.just_get_player_mut(ptr).expect("cannot get mutable player by player handle")
}

pub const BOSS_NAMES: [&str; 11] = [
    "mario", "sonic", "mosquito", "yuri", "slime", "ikaruga", "conan", "aokiji", "lazy", "covid", "saitama",
];

pub fn boss_display_name(name: &str) -> &str {
    match name {
        "mario" => "马里奥",
        "sonic" => "索尼克",
        "mosquito" => "蚊",
        "yuri" => "尤里",
        "slime" => "史莱姆",
        "ikaruga" => "斑鸠",
        "conan" => "柯南",
        "aokiji" => "青雉",
        "lazy" => "懒癌",
        "covid" => "新冠病毒",
        "saitama" => "一拳超人",
        _ => name,
    }
}

pub fn boss_append_attr(name: &str) -> [i32; 8] {
    match name {
        "covid" => [10, 9, 0, 12, 0, 12, 0, 60],
        "lazy" => [0, 88, 10, -20, 0, 50, 0, 120],
        "saitama" => [72, 39, 69, 76, 67, 66, 0, 84],
        "mario" => [20, 5, 15, 10, 20, 5, 0, 50],
        "sonic" => [10, 5, 40, 20, 10, 5, 0, 50],
        "mosquito" => [5, 5, 20, 30, 5, 5, 0, 80],
        "yuri" => [10, 10, 10, 10, 30, 30, 0, 50],
        "slime" => [5, 20, 5, 5, 5, 20, 0, 100],
        "ikaruga" => [15, 15, 10, 10, 15, 15, 0, 50],
        "conan" => [10, 10, 15, 15, 10, 10, 0, 50],
        "aokiji" => [30, 30, 10, 10, 30, 30, 0, 50],
        _ => [0; 8],
    }
}

pub const BOOST_NAMES: [&str; 3] = ["云剑狄卡敢", "云剑穸跄祇", "田一人"];

pub fn boost_value(name: &str) -> u32 {
    match name {
        "云剑狄卡敢" => 25,
        "云剑穸跄祇" => 35,
        "田一人" => 18,
        _ => 0,
    }
}

pub const SEED_PREFIX: &str = "seed:";

pub fn filter_char(s: char) -> bool {
    matches!(s as u32 , 9..12 | 133 | 160 | 5760 | 8192..8202 | 8232..8233 | 8239 | 8287 | 12288 | 65279)
}

pub fn median<T>(x: T, y: T, z: T) -> T
where
    T: std::cmp::Ord + std::marker::Copy,
{
    if x < y {
        if y < z {
            y
        } else if x < z {
            z
        } else {
            x
        }
    } else if x < z {
        x
    } else if y < z {
        z
    } else {
        y
    }
}

#[derive(Default, PartialEq, Eq, Debug, Clone, Copy)]
pub enum PlayerType {
    #[default]
    Normal,
    Seed,
    Clone,
    Boss,
    Boost,
    Test1,
    Test2,
    TestEx,
}

#[derive(Clone, Debug)]
pub struct Player {
    team: Option<String>,
    name: String,
    weapon: Option<String>,
    player_type: PlayerType,
    skil_id: Vec<u32>,
    skil_prop: Vec<u32>,
    pub sort_int: i32,
    pub rand: RC4,
    pub name_base: Vec<u8>,
    raw_name_base: [u8; 128],
    attr: [u32; 8],
    status: PlayerStatus,
    state: PlayerStateStore,
    skills: SkillStorage,
    name_factor: f64,
    pub weapon_state: Option<weapons::WeaponState>,
    id: u64,
}

impl PartialOrd for Player {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.p_cmp(other)) }
}

impl PartialEq for Player {
    fn eq(&self, other: &Self) -> bool { self.p_cmp(other) == Ordering::Equal }
}

impl std::fmt::Display for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Player{{{}{}, status: {}}}",
            if let Some(team) = &self.team {
                format!("{}@{}", self.name, team)
            } else {
                self.name.to_string()
            },
            if let Some(weapon) = &self.weapon {
                format!("+{}", weapon)
            } else {
                "".to_string()
            },
            self.status
        )
    }
}

#[cfg(test)]
mod test;

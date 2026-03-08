use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::error::runner::RunnerResult;
use crate::player::{Player, PlrId};
use crate::rc4::RC4;

pub type PlayerGroup = Vec<Player>;
pub type RawPlayers = (Vec<Vec<String>>, Vec<String>);

pub struct Runner {
    pub randomer: RC4,
    pub storage: Arc<Storage>,
    pub world: crate::engine::world_state::WorldState,
    pub core: crate::engine::engine_core::EngineCore,
}

impl Runner {
    pub fn new_from_namerena_raw(raw_input: String) -> RunnerResult<Runner> {
        let (players, seed) = Runner::split_namerena_into_groups(raw_input);
        let mut names = players
            .iter()
            .flatten()
            .filter(|str| !Player::check_is_seed(str))
            .map(|str| Player::raw_namerena_to_idname(str))
            .chain(std::iter::once(seed.clone()))
            .collect::<Vec<String>>();
        names.sort();
        names.dedup();

        let keys = names.join("\r");
        let mut randomer = RC4::new(keys.as_bytes(), 1);
        randomer.js_xor_str(&keys);

        let storage = Storage::new_arc();

        let mut initialized_players: Vec<Vec<PlrId>> = Vec::with_capacity(players.len());
        for plrs in &players {
            let mut group = Vec::with_capacity(plrs.len());
            for plr in plrs {
                if Player::check_is_seed(plr) {
                    continue;
                }
                let player = Player::new_from_namerena_raw(plr.to_string(), storage.clone())?;
                let ptr = storage.just_insert_player(player);
                group.push(ptr);
            }
            if !group.is_empty() {
                initialized_players.push(group);
            }
        }

        let mut local_players = initialized_players
            .iter()
            .map(|x| {
                x.iter()
                    .map(|id| {
                        let player = storage.just_get_player_mut(*id);
                        player.unwrap().init_values();
                        id
                    })
                    .copied()
                    .collect::<Vec<PlrId>>()
            })
            .collect::<Vec<Vec<PlrId>>>();

        let world = crate::engine::world_state::WorldState::new(local_players);
        let core = crate::engine::engine_core::EngineCore::default();

        Ok(Self {
            randomer,
            storage,
            world,
            core,
        })
    }

    fn split_namerena_into_groups(raw_input: String) -> (Vec<Vec<String>>, String) {
        let lines: Vec<&str> = raw_input.lines().collect();
        let mut players = Vec::new();
        let mut seed = String::new();

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if Player::check_is_seed(trimmed) {
                seed = trimmed.to_string();
            } else {
                players.push(vec![trimmed.to_string()]);
            }
        }

        (players, seed)
    }
}

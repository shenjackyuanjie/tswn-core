use std::sync::Arc;

use crate::{
    engine::storage::Storage,
    player::{Player, PlrId, StateTrait},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MinionKind {
    #[default]
    Clone,
    Summon,
    Shadow,
    Zombie,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MinionRuntimeState {
    pub owner: Option<PlrId>,
    pub kind: MinionKind,
}

impl MinionRuntimeState {
    #[inline]
    pub fn is_combat_minion(&self) -> bool { !matches!(self.kind, MinionKind::Clone) }
}

#[inline]
pub fn root_minion_name_owner_id(storage: &Arc<Storage>, start_id: PlrId) -> PlrId {
    let mut current = start_id;
    loop {
        let Some(player) = storage.get_player_or_pending(&current) else {
            return current;
        };
        let Some(minion) = player.get_state::<MinionRuntimeState>() else {
            return current;
        };
        if minion.kind != MinionKind::Clone {
            return current;
        }
        let Some(owner) = minion.owner else {
            return current;
        };
        current = owner;
    }
}

#[inline]
pub fn alloc_minion_name(storage: &Arc<Storage>, owner_id: PlrId) -> String {
    let root_owner_id = root_minion_name_owner_id(storage, owner_id);
    let owner_name = storage
        .get_player_or_pending(&root_owner_id)
        .map(|owner| owner.id_name())
        .expect("cannot get minion root owner from storage");
    let index = {
        let owner = storage
            .just_get_player_or_pending_mut(root_owner_id)
            .expect("cannot get mutable minion root owner from storage");
        owner.take_next_minion_name_index()
    };
    // JS getMinionName 返回的是 name?N@team；Rust 把 team 单独存放，
    // 所以这里只写入 name?N，最终显示时再由 id_key_name()/格式化补上 @team。
    format!("{owner_name}?{index}")
}

#[inline]
pub fn is_combat_minion(player: &crate::player::Player) -> bool {
    player
        .get_state::<MinionRuntimeState>()
        .map(MinionRuntimeState::is_combat_minion)
        .unwrap_or(false)
}

#[inline]
pub fn prepare_combat_minion(player: &mut Player) {
    // JS 的 Minion.bf() 会把 shadow/summon/zombie 的 x/name_factor 强制设为 0。
    player.name_factor = 0.0;
}

impl StateTrait for MinionRuntimeState {
    fn meta_type(&self) -> i32 { 0 }

    fn die_message_priority(&self) -> i32 { 100 }

    fn die_message(&self) -> Option<&'static str> { self.is_combat_minion().then_some("[1]消失了") }

    fn linked_owner(&self) -> Option<PlrId> { self.is_combat_minion().then_some(self.owner).flatten() }

    fn on_linked_owner_die(&mut self, owner: PlrId, self_id: PlrId, updates: &mut crate::engine::update::RunUpdates) -> bool {
        if !self.is_combat_minion() {
            return false;
        }
        updates.emit(crate::engine::update::RunUpdate::new_newline);
        updates.emit(|| crate::engine::update::RunUpdate::new("[1]消失了", owner, self_id, 50));
        true
    }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

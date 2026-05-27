use std::sync::Arc;

use crate::{
    engine::storage::Storage,
    player::{
        Player, PlrId, StateTrait,
        overlay::MinionOverlay,
        skill::{Skill, SkillBoost, SkillExt, skill_name_to_id, store::SkillStorage},
    },
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

pub fn owner_minion_overlay(storage: &Arc<Storage>, owner_id: PlrId, kind: MinionKind) -> Option<MinionOverlay> {
    let owner = storage.get_player(&owner_id)?;
    let overlay = owner.overlay.as_ref()?;
    match kind {
        MinionKind::Shadow => overlay.shadow.clone(),
        MinionKind::Summon => overlay.summon.clone(),
        MinionKind::Zombie => overlay.zombie.clone(),
        MinionKind::Clone => None,
    }
}

pub fn apply_minion_attrs(player: &mut Player, overlay: Option<&MinionOverlay>) -> bool {
    let Some(attrs) = overlay.and_then(|overlay| overlay.attrs) else {
        return false;
    };
    player.attr = attrs.map(|value| value.max(0) as u32);
    true
}

pub fn apply_summon_attrs(player: &mut Player, owner: &Player, overlay: Option<&MinionOverlay>) -> bool {
    if !apply_minion_attrs(player, overlay) {
        return false;
    }
    if overlay.map(|overlay| overlay.inherit_owner_def_res).unwrap_or(false) {
        player.attr[1] = owner.attr[1];
        player.attr[5] = owner.attr[5];
    }
    true
}

pub fn apply_minion_skill_overlay(player: &mut Player, overlay: Option<&MinionOverlay>) -> bool {
    let Some(skill_levels) = overlay.and_then(|overlay| overlay.skills.as_ref()) else {
        return false;
    };
    if player
        .get_state::<MinionRuntimeState>()
        .map(|state| state.kind == MinionKind::Summon)
        .unwrap_or(false)
    {
        return apply_summon_skill_overlay(player, skill_levels);
    }

    let mut skills = SkillStorage::new();
    for (name, boost) in skill_levels {
        let Some(mut skill) = minion_skill_from_overlay(name, boost) else {
            continue;
        };
        apply_overlay_boost(&mut skill, boost);
        skills.add_skill(skill);
    }
    skills.is_diy = true;
    skills.update_proc();
    player.skills = skills;
    true
}

fn apply_summon_skill_overlay(player: &mut Player, skill_levels: &[(String, SkillBoost)]) -> bool {
    let mut skills = SkillStorage::new();
    skills.add_skill(Skill::new_with_id(0, 0));
    skills.add_skill(Skill::new_with_id(0, 0));
    skills.add_skill(Skill::new(0, Box::new(super::summon::SummonExplodeSkill::new())));
    skills.slot_skill = vec![0, 1, 2];
    skills.skill.clear();

    for (name, boost) in skill_levels {
        let Some(skill_key) = summon_skill_key_from_overlay(name) else {
            continue;
        };
        apply_overlay_boost(skills.skill_by_id_mut(skill_key), boost);
        if !skills.skill.contains(&skill_key) {
            skills.skill.push(skill_key);
        }
    }

    skills.is_diy = true;
    skills.update_proc();
    player.skills = skills;
    true
}

fn summon_skill_key_from_overlay(name: &str) -> Option<usize> {
    match normalize_minion_skill_name(name).as_str() {
        "fire1" => Some(0),
        "fire2" => Some(1),
        "explode" | "selfdestruct" | "self_destruct" | "summonexplode" | "鑷垎" => Some(2),
        _ => None,
    }
}

fn minion_skill_from_overlay(name: &str, boost: &SkillBoost) -> Option<Skill> {
    let level = boost.base_level();
    match normalize_minion_skill_name(name).as_str() {
        "possess" | "possession" | "附体" => Some(Skill::new(level, super::possess::PossessSkill::box_new())),
        "explode" | "selfdestruct" | "self_destruct" | "summonexplode" | "自爆" => {
            Some(Skill::new(level, super::summon::SummonExplodeSkill::box_new()))
        }
        _ => skill_name_to_id(name).map(|id| Skill::new_with_id(level, id as u8)),
    }
}

fn normalize_minion_skill_name(name: &str) -> String {
    let lower = name.trim().to_ascii_lowercase();
    lower
        .strip_prefix("skl")
        .or_else(|| lower.strip_prefix("skill"))
        .unwrap_or(lower.as_str())
        .to_string()
}

fn apply_overlay_boost(skill: &mut Skill, boost: &SkillBoost) {
    skill.set_level(boost.base_level());
    skill.boosted = false;
    skill.diy_boost = None;
    match boost {
        SkillBoost::Normal(_) => {}
        SkillBoost::LastBoost(_) => {
            skill.set_level(skill.level().saturating_mul(2));
            skill.boosted = true;
            skill.diy_boost = Some(boost.clone());
        }
        SkillBoost::SlotBoost { boost: amount, .. } => {
            let amount = (*amount).min(skill.level());
            skill.set_level(skill.level().saturating_add(amount));
            skill.boosted = true;
            skill.diy_boost = Some(boost.clone());
        }
    }
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

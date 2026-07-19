use crate::runtime::entity::PlayerTemplate;
use crate::runtime::{BattleSlotId, EntitySlotId, ExtensionRegistry, TemplateSlotId};
use smallvec::SmallVec;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_SLOT_BASELINE_ID: AtomicU64 = AtomicU64::new(1);

fn next_slot_baseline_id() -> u64 { NEXT_SLOT_BASELINE_ID.fetch_add(1, Ordering::Relaxed) }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotValue {
    Bool(bool),
    I64(i64),
    U64(u64),
    Text(String),
    PlayerTemplate(Box<PlayerTemplate>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotError {
    InvalidTemplateSlot(TemplateSlotId),
    InvalidBattleSlot(BattleSlotId),
    InvalidEntitySlot(EntitySlotId),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TemplateSlotStorage {
    values: Vec<Option<SlotValue>>,
}

impl TemplateSlotStorage {
    pub fn from_registry(registry: &ExtensionRegistry) -> Self { Self::with_len(registry.template_slots().len()) }

    pub fn with_len(len: usize) -> Self { Self { values: vec![None; len] } }

    pub fn set(&mut self, id: TemplateSlotId, value: SlotValue) -> Result<(), SlotError> {
        let Some(slot) = self.values.get_mut(id.0 as usize) else {
            return Err(SlotError::InvalidTemplateSlot(id));
        };
        *slot = Some(value);
        Ok(())
    }

    pub fn get(&self, id: TemplateSlotId) -> Option<&SlotValue> { self.values.get(id.0 as usize).and_then(Option::as_ref) }

    pub fn iter(&self) -> impl Iterator<Item = (TemplateSlotId, &SlotValue)> {
        self.values
            .iter()
            .enumerate()
            .filter_map(|(idx, value)| value.as_ref().map(|value| (TemplateSlotId(idx as u32), value)))
    }

    pub fn len(&self) -> usize { self.values.len() }

    pub fn is_empty(&self) -> bool { self.values.is_empty() }
}

#[derive(Debug, Clone)]
pub struct BattleSlotStorage {
    values: Vec<Option<SlotValue>>,
    baseline_id: u64,
    battle_dirty: bool,
}

impl Default for BattleSlotStorage {
    fn default() -> Self { Self::with_len(0) }
}

impl PartialEq for BattleSlotStorage {
    fn eq(&self, other: &Self) -> bool { self.values == other.values }
}

impl Eq for BattleSlotStorage {}

impl BattleSlotStorage {
    pub fn from_registry(registry: &ExtensionRegistry) -> Self { Self::with_len(registry.battle_slots().len()) }

    pub fn with_len(len: usize) -> Self {
        Self {
            values: vec![None; len],
            baseline_id: next_slot_baseline_id(),
            battle_dirty: false,
        }
    }

    pub fn set(&mut self, id: BattleSlotId, value: SlotValue) -> Result<(), SlotError> {
        let Some(slot) = self.values.get_mut(id.0 as usize) else {
            return Err(SlotError::InvalidBattleSlot(id));
        };
        *slot = Some(value);
        self.battle_dirty = true;
        Ok(())
    }

    pub fn get(&self, id: BattleSlotId) -> Option<&SlotValue> { self.values.get(id.0 as usize).and_then(Option::as_ref) }

    pub fn clear(&mut self) {
        let mut changed = false;
        for value in &mut self.values {
            changed |= value.is_some();
            *value = None;
        }
        self.battle_dirty |= changed;
    }

    /// 封存 prepared runner 的全局槽位基线。
    pub(crate) fn mark_battle_baseline(&mut self) { self.battle_dirty = false; }

    /// 仅在本局写过全局槽位时恢复内容，常见只读路径不再克隆 Vec。
    pub(crate) fn reset_battle_state_from(&mut self, prepared: &Self) {
        if self.baseline_id != prepared.baseline_id {
            self.clone_from(prepared);
            return;
        }
        if !self.battle_dirty {
            return;
        }
        self.values.clone_from(&prepared.values);
        self.battle_dirty = false;
    }

    pub fn len(&self) -> usize { self.values.len() }

    pub fn is_empty(&self) -> bool { self.values.is_empty() }
}

const ENTITY_SLOT_DIRTY_TEAM: u64 = 1;
const ENTITY_SLOT_DIRTY_VALUE: u64 = 2;
const ENTITY_SLOTS_PER_DIRTY_WORD: usize = 32;

#[derive(Debug, Clone)]
pub struct EntitySlotStorage {
    values: Vec<Option<SlotValue>>,
    baseline_id: u64,
    /// 每槽 2 bit 区分蓝图 team-only 与完整值写入；默认 core 槽位只占一个栈内 word。
    battle_dirty_words: SmallVec<[u64; 1]>,
}

impl Default for EntitySlotStorage {
    fn default() -> Self { Self::with_len(0) }
}

impl PartialEq for EntitySlotStorage {
    fn eq(&self, other: &Self) -> bool { self.values == other.values }
}

impl Eq for EntitySlotStorage {}

impl EntitySlotStorage {
    pub fn from_registry(registry: &ExtensionRegistry) -> Self { Self::with_len(registry.entity_slots().len()) }

    pub fn with_len(len: usize) -> Self {
        Self {
            values: vec![None; len],
            baseline_id: next_slot_baseline_id(),
            battle_dirty_words: std::iter::repeat_n(0, len.div_ceil(ENTITY_SLOTS_PER_DIRTY_WORD)).collect(),
        }
    }

    pub fn set(&mut self, id: EntitySlotId, value: SlotValue) -> Result<(), SlotError> {
        let index = id.0 as usize;
        if index >= self.values.len() {
            return Err(SlotError::InvalidEntitySlot(id));
        }
        self.mark_battle_dirty(index, ENTITY_SLOT_DIRTY_VALUE);
        self.values[index] = Some(value);
        Ok(())
    }

    pub fn get(&self, id: EntitySlotId) -> Option<&SlotValue> { self.values.get(id.0 as usize).and_then(Option::as_ref) }

    pub fn get_mut(&mut self, id: EntitySlotId) -> Option<&mut SlotValue> {
        let index = id.0 as usize;
        if !self.values.get(index).is_some_and(Option::is_some) {
            return None;
        }
        self.mark_battle_dirty(index, ENTITY_SLOT_DIRTY_VALUE);
        self.values[index].as_mut()
    }

    /// 仅在蓝图队伍确实变化时写入并标脏，避免种子复位的只读命中触发整槽深拷贝。
    pub(crate) fn update_player_template_team(&mut self, id: EntitySlotId, team: usize) {
        let Some(Some(SlotValue::PlayerTemplate(template))) = self.values.get_mut(id.0 as usize) else {
            return;
        };
        if template.team != team {
            template.team = team;
            self.mark_battle_dirty(id.0 as usize, ENTITY_SLOT_DIRTY_TEAM);
        }
    }

    pub(crate) fn remove(&mut self, id: EntitySlotId) -> Option<SlotValue> {
        let index = id.0 as usize;
        let removed = self.values.get_mut(index).and_then(Option::take);
        if removed.is_some() {
            self.mark_battle_dirty(index, ENTITY_SLOT_DIRTY_VALUE);
        }
        removed
    }

    pub fn clear(&mut self) {
        for index in 0..self.values.len() {
            if self.values[index].take().is_some() {
                self.mark_battle_dirty(index, ENTITY_SLOT_DIRTY_VALUE);
            }
        }
    }

    /// 封存 prepared runner 的实体槽位基线。
    pub(crate) fn mark_battle_baseline(&mut self) { self.battle_dirty_words.fill(0); }

    /// 仅恢复本局写过的槽位，计数器变化不再连带深拷贝同表中的 PlayerTemplate。
    pub(crate) fn reset_battle_state_from(&mut self, prepared: &Self) {
        if self.baseline_id != prepared.baseline_id {
            self.clone_from(prepared);
            return;
        }
        if !self.has_battle_dirty() {
            return;
        }
        for word_index in 0..self.battle_dirty_words.len() {
            let mut dirty = std::mem::take(&mut self.battle_dirty_words[word_index]);
            while dirty != 0 {
                let local_index = dirty.trailing_zeros() as usize / 2;
                let shift = local_index * 2;
                let flags = (dirty >> shift) & 3;
                let index = word_index * ENTITY_SLOTS_PER_DIRTY_WORD + local_index;
                if flags & ENTITY_SLOT_DIRTY_VALUE != 0 {
                    self.values[index].clone_from(&prepared.values[index]);
                } else if flags & ENTITY_SLOT_DIRTY_TEAM != 0 {
                    let (Some(SlotValue::PlayerTemplate(current)), Some(SlotValue::PlayerTemplate(prepared))) =
                        (self.values[index].as_mut(), prepared.values[index].as_ref())
                    else {
                        unreachable!("team-only 实体槽必须保留玩家蓝图")
                    };
                    current.team = prepared.team;
                }
                dirty &= !(3u64 << shift);
            }
        }
    }

    #[inline]
    fn mark_battle_dirty(&mut self, index: usize, flags: u64) {
        let word = index / ENTITY_SLOTS_PER_DIRTY_WORD;
        let shift = (index % ENTITY_SLOTS_PER_DIRTY_WORD) * 2;
        self.battle_dirty_words[word] |= flags << shift;
    }

    fn has_battle_dirty(&self) -> bool { self.battle_dirty_words.iter().any(|word| *word != 0) }

    pub fn len(&self) -> usize { self.values.len() }

    pub fn is_empty(&self) -> bool { self.values.is_empty() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::ExtensionRegistryBuilder;

    #[test]
    fn slot_storages_size_from_registry_specs() {
        let mut builder = ExtensionRegistryBuilder::default();
        let template = builder
            .reserve_template_slot("custom", "template", "custom.template")
            .expect("template slot should reserve");
        let battle = builder
            .reserve_battle_slot("custom", "battle", "custom.battle")
            .expect("battle slot should reserve");
        let entity = builder
            .reserve_entity_slot("custom", "entity", "custom.entity")
            .expect("entity slot should reserve");
        let registry = builder.build();

        let mut template_slots = TemplateSlotStorage::from_registry(&registry);
        let mut battle_slots = BattleSlotStorage::from_registry(&registry);
        let mut entity_slots = EntitySlotStorage::from_registry(&registry);

        template_slots.set(template, SlotValue::Text("ready".to_owned())).unwrap();
        battle_slots.set(battle, SlotValue::U64(7)).unwrap();
        entity_slots.set(entity, SlotValue::Bool(true)).unwrap();

        assert_eq!(template_slots.get(template), Some(&SlotValue::Text("ready".to_owned())));
        assert_eq!(battle_slots.get(battle), Some(&SlotValue::U64(7)));
        assert_eq!(entity_slots.get(entity), Some(&SlotValue::Bool(true)));
    }

    #[test]
    fn battle_and_entity_slots_clear_without_resizing() {
        let mut battle_slots = BattleSlotStorage::with_len(2);
        let mut entity_slots = EntitySlotStorage::with_len(2);

        battle_slots.set(BattleSlotId(1), SlotValue::I64(-3)).unwrap();
        entity_slots.set(EntitySlotId(1), SlotValue::Bool(true)).unwrap();

        battle_slots.clear();
        entity_slots.clear();

        assert_eq!(battle_slots.len(), 2);
        assert_eq!(entity_slots.len(), 2);
        assert_eq!(battle_slots.get(BattleSlotId(1)), None);
        assert_eq!(entity_slots.get(EntitySlotId(1)), None);
    }

    #[test]
    fn slot_storages_reject_unknown_ids() {
        let mut template_slots = TemplateSlotStorage::with_len(1);
        let mut battle_slots = BattleSlotStorage::with_len(1);
        let mut entity_slots = EntitySlotStorage::with_len(1);

        assert_eq!(
            template_slots.set(TemplateSlotId(1), SlotValue::Bool(true)),
            Err(SlotError::InvalidTemplateSlot(TemplateSlotId(1)))
        );
        assert_eq!(
            battle_slots.set(BattleSlotId(1), SlotValue::Bool(true)),
            Err(SlotError::InvalidBattleSlot(BattleSlotId(1)))
        );
        assert_eq!(
            entity_slots.set(EntitySlotId(1), SlotValue::Bool(true)),
            Err(SlotError::InvalidEntitySlot(EntitySlotId(1)))
        );
    }

    #[test]
    fn unchanged_blueprint_team_does_not_mark_entity_slots_dirty() {
        let slot = EntitySlotId(0);
        let mut slots = EntitySlotStorage::with_len(1);
        slots
            .set(
                slot,
                SlotValue::PlayerTemplate(Box::new(PlayerTemplate::new(1, "blueprint", 2, 10, 3))),
            )
            .unwrap();
        slots.mark_battle_baseline();
        let prepared = slots.clone();

        slots.update_player_template_team(slot, 2);
        assert!(!slots.has_battle_dirty());

        slots.update_player_template_team(slot, 3);
        assert!(slots.has_battle_dirty());
        assert!(matches!(slots.get(slot), Some(SlotValue::PlayerTemplate(template)) if template.team == 3));
        slots.reset_battle_state_from(&prepared);
        assert!(matches!(slots.get(slot), Some(SlotValue::PlayerTemplate(template)) if template.team == 2));
        assert!(!slots.has_battle_dirty());
    }

    #[test]
    fn entity_slots_restore_only_bits_written_in_current_battle() {
        let low = EntitySlotId(0);
        let high = EntitySlotId(65);
        let mut prepared = EntitySlotStorage::with_len(66);
        prepared.set(low, SlotValue::Text("baseline".to_owned())).unwrap();
        prepared.set(high, SlotValue::U64(7)).unwrap();
        prepared.mark_battle_baseline();
        let mut current = prepared.clone();

        current.set(high, SlotValue::U64(9)).unwrap();
        assert_eq!(current.battle_dirty_words.as_slice(), &[0, 0, 8]);
        current.reset_battle_state_from(&prepared);

        assert_eq!(current, prepared);
        assert!(!current.has_battle_dirty());
    }
}

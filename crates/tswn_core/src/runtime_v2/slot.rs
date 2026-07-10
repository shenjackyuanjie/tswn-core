use crate::runtime_v2::entity::PlayerTemplate;
use crate::runtime_v2::{BattleSlotId, EntitySlotId, ExtensionRegistry, TemplateSlotId};

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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BattleSlotStorage {
    values: Vec<Option<SlotValue>>,
}

impl BattleSlotStorage {
    pub fn from_registry(registry: &ExtensionRegistry) -> Self { Self::with_len(registry.battle_slots().len()) }

    pub fn with_len(len: usize) -> Self { Self { values: vec![None; len] } }

    pub fn set(&mut self, id: BattleSlotId, value: SlotValue) -> Result<(), SlotError> {
        let Some(slot) = self.values.get_mut(id.0 as usize) else {
            return Err(SlotError::InvalidBattleSlot(id));
        };
        *slot = Some(value);
        Ok(())
    }

    pub fn get(&self, id: BattleSlotId) -> Option<&SlotValue> { self.values.get(id.0 as usize).and_then(Option::as_ref) }

    pub fn clear(&mut self) {
        for value in &mut self.values {
            *value = None;
        }
    }

    pub fn len(&self) -> usize { self.values.len() }

    pub fn is_empty(&self) -> bool { self.values.is_empty() }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EntitySlotStorage {
    values: Vec<Option<SlotValue>>,
}

impl EntitySlotStorage {
    pub fn from_registry(registry: &ExtensionRegistry) -> Self { Self::with_len(registry.entity_slots().len()) }

    pub fn with_len(len: usize) -> Self { Self { values: vec![None; len] } }

    pub fn set(&mut self, id: EntitySlotId, value: SlotValue) -> Result<(), SlotError> {
        let Some(slot) = self.values.get_mut(id.0 as usize) else {
            return Err(SlotError::InvalidEntitySlot(id));
        };
        *slot = Some(value);
        Ok(())
    }

    pub fn get(&self, id: EntitySlotId) -> Option<&SlotValue> { self.values.get(id.0 as usize).and_then(Option::as_ref) }

    pub fn clear(&mut self) {
        for value in &mut self.values {
            *value = None;
        }
    }

    pub fn len(&self) -> usize { self.values.len() }

    pub fn is_empty(&self) -> bool { self.values.is_empty() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_v2::ExtensionRegistryBuilder;

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
}

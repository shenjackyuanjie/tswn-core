use crate::player::PlrId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerTemplate {
    pub id: PlrId,
    pub name: String,
    pub team: usize,
    pub max_hp: i32,
    pub attack: i32,
}

impl PlayerTemplate {
    pub fn new(id: PlrId, name: impl Into<String>, team: usize, max_hp: i32, attack: i32) -> Self {
        assert!(max_hp > 0, "runtime_v2 player max_hp must be positive");
        assert!(attack >= 0, "runtime_v2 player attack must be non-negative");
        Self {
            id,
            name: name.into(),
            team,
            max_hp,
            attack,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerRuntime {
    pub hp: i32,
    pub alive: bool,
}

impl PlayerRuntime {
    fn from_template(template: &PlayerTemplate) -> Self {
        Self {
            hp: template.max_hp,
            alive: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityRecord {
    pub template: PlayerTemplate,
    pub runtime: PlayerRuntime,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EntityArena {
    entities: Vec<EntityRecord>,
}

impl EntityArena {
    pub fn from_templates(players: Vec<PlayerTemplate>) -> Self {
        let entities = players
            .into_iter()
            .map(|template| {
                let runtime = PlayerRuntime::from_template(&template);
                EntityRecord { template, runtime }
            })
            .collect();
        Self { entities }
    }

    pub fn len(&self) -> usize { self.entities.len() }

    pub fn is_empty(&self) -> bool { self.entities.is_empty() }

    pub fn get(&self, idx: EntityIdx) -> Option<&EntityRecord> { self.entities.get(idx.0 as usize) }

    pub fn get_mut(&mut self, idx: EntityIdx) -> Option<&mut EntityRecord> { self.entities.get_mut(idx.0 as usize) }

    pub fn iter(&self) -> impl Iterator<Item = (EntityIdx, &EntityRecord)> {
        self.entities.iter().enumerate().map(|(idx, entity)| (EntityIdx(idx as u32), entity))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityIdx(pub u32);

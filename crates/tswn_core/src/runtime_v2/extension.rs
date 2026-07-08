use std::collections::HashMap;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerKindId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerKindSpec {
    pub id: PlayerKindId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionError {
    DuplicateName { namespace: String, name: String },
    DuplicateExportName { export_name: String },
}

impl Display for ExtensionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtensionError::DuplicateName { namespace, name } => {
                write!(f, "duplicate extension name: {namespace}::{name}")
            }
            ExtensionError::DuplicateExportName { export_name } => {
                write!(f, "duplicate extension export name: {export_name}")
            }
        }
    }
}

impl std::error::Error for ExtensionError {}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExtensionRegistryBuilder {
    player_kinds: Vec<PlayerKindSpec>,
    player_kind_names: HashMap<(String, String), PlayerKindId>,
    export_names: HashMap<String, PlayerKindId>,
}

impl ExtensionRegistryBuilder {
    pub fn register_player_kind(
        &mut self,
        namespace: impl Into<String>,
        name: impl Into<String>,
        export_name: impl Into<String>,
    ) -> Result<PlayerKindId, ExtensionError> {
        let namespace = namespace.into();
        let name = name.into();
        let export_name = export_name.into();
        let name_key = (namespace.clone(), name.clone());

        if self.player_kind_names.contains_key(&name_key) {
            return Err(ExtensionError::DuplicateName { namespace, name });
        }
        if self.export_names.contains_key(&export_name) {
            return Err(ExtensionError::DuplicateExportName { export_name });
        }

        let id = PlayerKindId(self.player_kinds.len() as u32);
        let spec = PlayerKindSpec {
            id,
            namespace,
            name,
            export_name,
        };
        self.player_kind_names.insert(name_key, id);
        self.export_names.insert(spec.export_name.clone(), id);
        self.player_kinds.push(spec);
        Ok(id)
    }

    pub fn build(self) -> ExtensionRegistry {
        ExtensionRegistry {
            player_kinds: self.player_kinds,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExtensionRegistry {
    player_kinds: Vec<PlayerKindSpec>,
}

impl ExtensionRegistry {
    pub fn player_kind(&self, id: PlayerKindId) -> Option<&PlayerKindSpec> { self.player_kinds.get(id.0 as usize) }

    pub fn player_kinds(&self) -> &[PlayerKindSpec] { &self.player_kinds }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_allocates_stable_player_kind_ids_in_registration_order() {
        let mut builder = ExtensionRegistryBuilder::default();

        let alpha = builder
            .register_player_kind("custom", "alpha", "custom.alpha")
            .expect("alpha should register");
        let beta = builder
            .register_player_kind("custom", "beta", "custom.beta")
            .expect("beta should register");

        assert_eq!(alpha, PlayerKindId(0));
        assert_eq!(beta, PlayerKindId(1));

        let registry = builder.build();
        assert_eq!(registry.player_kind(alpha).unwrap().name, "alpha");
        assert_eq!(registry.player_kind(beta).unwrap().export_name, "custom.beta");
        assert_eq!(registry.player_kinds().len(), 2);
    }

    #[test]
    fn registry_allows_same_local_name_in_different_namespaces() {
        let mut builder = ExtensionRegistryBuilder::default();

        let custom = builder
            .register_player_kind("custom", "boss", "custom.boss")
            .expect("custom boss should register");
        let fixture = builder
            .register_player_kind("fixture", "boss", "fixture.boss")
            .expect("fixture boss should register");

        assert_eq!(custom, PlayerKindId(0));
        assert_eq!(fixture, PlayerKindId(1));
    }

    #[test]
    fn registry_rejects_duplicate_player_kind_name_in_namespace() {
        let mut builder = ExtensionRegistryBuilder::default();
        builder
            .register_player_kind("custom", "boss", "custom.boss")
            .expect("first boss should register");

        assert_eq!(
            builder.register_player_kind("custom", "boss", "custom.boss.v2"),
            Err(ExtensionError::DuplicateName {
                namespace: "custom".to_owned(),
                name: "boss".to_owned(),
            })
        );
    }

    #[test]
    fn registry_rejects_duplicate_export_name() {
        let mut builder = ExtensionRegistryBuilder::default();
        builder
            .register_player_kind("custom", "boss", "custom.boss")
            .expect("first boss should register");

        assert_eq!(
            builder.register_player_kind("fixture", "boss", "custom.boss"),
            Err(ExtensionError::DuplicateExportName {
                export_name: "custom.boss".to_owned(),
            })
        );
    }
}

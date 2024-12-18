use crate::display::SentenceList;
use cache_diff::CacheDiff;
use libcnb::build::BuildContext;
use libcnb::data::layer::LayerName;
use libcnb::layer::{CachedLayerDefinition, InvalidMetadataAction, LayerRef, RestoredLayerAction};
use magic_migrate::TryMigrate;
use serde::ser::Serialize;
use std::fmt::Debug;

/// Default behavior for a cached layer, ensures new metadata is always written
///
/// The metadadata must implement `CacheDiff` and `TryMigrate` in addition
/// to the typical `Serialize` and `Debug` traits
pub fn cached_layer_write_metadata<M, B>(
    layer_name: LayerName,
    context: &BuildContext<B>,
    metadata: &'_ M,
) -> libcnb::Result<LayerRef<B, Meta<M>, Meta<M>>, B::Error>
where
    B: libcnb::Buildpack,
    M: CacheDiff + TryMigrate + Serialize + Debug + Clone,
{
    let layer_ref = context.cached_layer(
        layer_name,
        CachedLayerDefinition {
            build: true,
            launch: true,
            invalid_metadata_action: &invalid_metadata_action,
            restored_layer_action: &|old: &M, _| restored_layer_action(old, metadata),
        },
    )?;
    layer_ref.write_metadata(metadata)?;
    Ok(layer_ref)
}

/// Standardizes formatting for layer cache clearing behavior
///
/// If the diff is empty, there are no changes and the layer is kept and the old data is returned
/// If the diff is not empty, the layer is deleted and the changes are listed
pub fn restored_layer_action<M>(old: &M, now: &M) -> (RestoredLayerAction, Meta<M>)
where
    M: CacheDiff + Clone,
{
    let diff = now.diff(old);
    if diff.is_empty() {
        (RestoredLayerAction::KeepLayer, Meta::Data(old.clone()))
    } else {
        (
            RestoredLayerAction::DeleteLayer,
            Meta::Message(format!(
                "Clearing cache due to {changes}: {differences}",
                changes = if diff.len() > 1 { "changes" } else { "change" },
                differences = SentenceList::new(&diff)
            )),
        )
    }
}

/// Standardizes formatting for invalid metadata behavior
///
/// If the metadata can be migrated, it is replaced with the migrated version
/// If an error occurs, the layer is deleted and the error displayed
/// If no migration is possible, the layer is deleted and the invalid metadata is displayed
pub fn invalid_metadata_action<M, S>(invalid: &S) -> (InvalidMetadataAction<M>, Meta<M>)
where
    M: TryMigrate + Clone,
    S: Serialize + Debug,
{
    let invalid = toml::to_string(invalid);
    match invalid {
        Ok(toml) => match M::try_from_str_migrations(&toml) {
            Some(Ok(migrated)) => (
                InvalidMetadataAction::ReplaceMetadata(migrated.clone()),
                Meta::Data(migrated),
            ),
            Some(Err(error)) => (
                InvalidMetadataAction::DeleteLayer,
                Meta::Message(format!(
                    "Clearing cache due to metadata migration error: {error}"
                )),
            ),
            None => (
                InvalidMetadataAction::DeleteLayer,
                Meta::Message(format!(
                    "Clearing cache due to invalid metadata ({toml})",
                    toml = toml.trim()
                )),
            ),
        },
        Err(error) => (
            InvalidMetadataAction::DeleteLayer,
            Meta::Message(format!(
                "Clearing cache due to invalid metadata serialization error: {error}"
            )),
        ),
    }
}

/// Either contains metadata or a message describing the state
///
/// Why: The `CachedLayerDefinition` allows returning information about the cache state
/// from either `invalid_metadata_action` or `restored_layer_action` functions.
///
/// Because the function returns only a single type, that type must be the same for
/// all possible cache conditions (cleared or retained). Therefore, the type must be
/// able to represent information about the cache state when it's cleared or not.
///
/// This struct implements `Display` and `AsRef<str>` so if the end user only
/// wants to advertise the cache state, they can do so by passing the whole struct
/// to `format!` or `println!` without any further maniuplation. If they need
/// to inspect the previous metadata they can match on the enum and extract
/// what they need.
///
/// - Will only ever contain metadata when the cache is retained.
/// - Will contain a message when the cache is cleared, describing why it was cleared.
///   It is also allowable to return a message when the cache is retained, and the
///   message describes the state of the cache. (i.e. because a message is returned
///   does not guarantee the cache was cleared).
pub enum Meta<M> {
    Message(String),
    Data(M),
}

impl<M> std::fmt::Display for Meta<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl<M> AsRef<str> for Meta<M> {
    fn as_ref(&self) -> &str {
        match self {
            Meta::Message(s) => s.as_str(),
            Meta::Data(_) => "Using cache",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cache_diff::CacheDiff;
    use core::panic;
    use libcnb::data::layer_name;
    use libcnb::layer::{EmptyLayerCause, InvalidMetadataAction, LayerState, RestoredLayerAction};
    use magic_migrate::{migrate_toml_chain, try_migrate_deserializer_chain, Migrate, TryMigrate};
    use serde::Deserializer;
    use std::convert::Infallible;
    /// Struct for asserting the behavior of `cached_layer_write_metadata`
    #[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
    #[serde(deny_unknown_fields)]
    struct TestMetadata {
        value: String,
    }
    impl CacheDiff for TestMetadata {
        fn diff(&self, old: &Self) -> Vec<String> {
            if self.value == old.value {
                vec![]
            } else {
                vec![format!("value ({} to {})", old.value, self.value)]
            }
        }
    }
    migrate_toml_chain! {TestMetadata}

    #[test]
    fn test_restored_layer_action_returns_old_data() {
        #[derive(Debug, Clone)]
        struct AlwaysNoDiff {
            value: String,
        }
        impl CacheDiff for AlwaysNoDiff {
            fn diff(&self, _: &Self) -> Vec<String> {
                vec![]
            }
        }

        let old = AlwaysNoDiff {
            value: "old".to_string(),
        };
        let now = AlwaysNoDiff {
            value: "now".to_string(),
        };

        let result = restored_layer_action(&old, &now);
        match result {
            (RestoredLayerAction::KeepLayer, Meta::Data(data)) => {
                assert_eq!(data.value, "old");
            }
            _ => panic!("Expected to keep layer"),
        }
    }

    /// Struct for asserting the behavior of `invalid_metadata_action`
    #[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
    #[serde(deny_unknown_fields)]
    struct PersonV1 {
        name: String,
    }
    /// Struct for asserting the behavior of `invalid_metadata_action`
    #[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
    #[serde(deny_unknown_fields)]
    struct PersonV2 {
        name: String,
        updated_at: String,
    }
    // First define how to map from one struct to another
    impl TryFrom<PersonV1> for PersonV2 {
        type Error = NotRichard;
        fn try_from(value: PersonV1) -> Result<Self, NotRichard> {
            if &value.name == "Schneems" {
                Ok(PersonV2 {
                    name: value.name.clone(),
                    updated_at: "unknown".to_string(),
                })
            } else {
                Err(NotRichard {
                    name: value.name.clone(),
                })
            }
        }
    }
    #[derive(Debug, Eq, PartialEq)]
    struct NotRichard {
        name: String,
    }
    impl From<NotRichard> for PersonMigrationError {
        fn from(value: NotRichard) -> Self {
            PersonMigrationError::NotRichard(value)
        }
    }
    #[derive(Debug, Eq, PartialEq, thiserror::Error)]
    enum PersonMigrationError {
        #[error("Not Richard")]
        NotRichard(NotRichard),
    }
    try_migrate_deserializer_chain!(
        deserializer: toml::Deserializer::new,
        error: PersonMigrationError,
        chain: [PersonV1, PersonV2],
    );

    #[test]
    fn test_invalid_metadata_action() {
        let (action, message) = invalid_metadata_action::<PersonV1, _>(&PersonV1 {
            name: "schneems".to_string(),
        });
        assert!(matches!(action, InvalidMetadataAction::ReplaceMetadata(_)));
        assert_eq!(message.as_ref(), "Using cache");

        let (action, message) = invalid_metadata_action::<PersonV2, _>(&PersonV1 {
            name: "not_richard".to_string(),
        });
        assert!(matches!(action, InvalidMetadataAction::DeleteLayer));
        assert_eq!(
            message.as_ref(),
            "Clearing cache due to metadata migration error: Not Richard"
        );

        let (action, message) = invalid_metadata_action::<PersonV2, _>(&TestMetadata {
            value: "world".to_string(),
        });
        assert!(matches!(action, InvalidMetadataAction::DeleteLayer));
        assert_eq!(
            message.as_ref(),
            "Clearing cache due to invalid metadata (value = \"world\")"
        );
        // Unable to produce this error at will: "Clearing cache due to invalid metadata serialization error: {error}"
    }
}

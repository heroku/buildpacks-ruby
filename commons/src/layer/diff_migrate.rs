//! Declarative Layer Cache invalidation logic.
//!
//! Cache invalidation is one of the "famously" difficult problems in computer science. This module
//! provides a clean, yet opinonated interface for handling cache invalidation and migrating invalid
//! metadata.
//!
//! - Declarative interface for defining cache invalidation behavior (via [`cache_diff::CacheDiff`])
//! - Declarative interface for defining invalid metadata migration behavior (via [`magic_migrate::TryMigrate`])
//! - Prevent accidentally reading one struct type and writing a different one
//!
//! The primary interface is [`DiffMigrateLayer`].
//!
//! ## Cache invalidation logic ([`cache_diff::CacheDiff`])
//!
//! The `CacheDiff` derive macro from `cache_diff` allows you to tell [`DiffMigrateLayer`] which fields in your
//! metadata struct act as cache keys and how to compare them. If a difference is reported, the cache
//! is cleared.
//!
//! Importantly, when the cache is cleared, a clear message stating why the cache was cleared is returned
//! in a user readable format.
//!
//! ## Invalid metadata migration ([`magic_migrate::TryMigrate`])
//!
//! If previously serialized metadata cannot be deserialized into the current struct then usually the
//! only thing a buildpack can do is discard the cache. However, that may involve needing to re-do an
//! expensive operation such as re-compiling native libraries. Buildpack authors should feel free to
//! refactor and update their metadata structs without fear of busting the cache. Users should not
//! have to suffer slower builds due to internal only buildpack changes.
//!
//! The `TryMigrate` trait from `magic_migrate` allows buildpack authors to define how to migrate an
//! older struct into a newer one. If the migration fails, the cache is cleared and the reason is returned.
//! If the migration succeeds, then the regular logic in `CacheDiff` is applied.
//!
//! ## Read your write, or (read) why you can't ([`Meta`])
//!
//! If non-cache data is stored in the Metadata, then your buildpack may want to read that data back.
//! When the cache is not cleared then the old metadata is returned. This allows you to read your write.
//!
//! A buildpack cache should never be cleared without explaining why to a user via printing to the
//! build output. If the cache is cleared for any reason, then a user readable message is returned. This message should
//! be printed to the buildpack user so they can understand what caused the cache to clear.
//!
#![doc = include_str!("fixtures/metadata_migration_example.md")]

use crate::display::SentenceList;
use cache_diff::CacheDiff;
use fs_err::PathExt;
use libcnb::build::BuildContext;
use libcnb::data::layer::LayerName;
use libcnb::layer::{
    CachedLayerDefinition, InvalidMetadataAction, LayerError, LayerRef, RestoredLayerAction,
};
use magic_migrate::TryMigrate;
use serde::ser::Serialize;
use std::fmt::Debug;
use std::path::PathBuf;

#[cfg(test)]
use bullet_stream as _;

/// Creates a cached layer, potentially re-using a previously cached version with default invalidation and migration logic.
///
/// Like [`BuildContext::cached_layer`], this allows Buildpack code to create a cached layer and get
/// back a reference to the layer directory on disk. Intricacies of the CNB spec are automatically handled
/// such as the maintenance of TOML files.
///
/// In addition it provides default behavior for cache invalidation, automatic invalid metadata migration,
/// as well as ensuring that the latest metadata is set on the layer.
///
/// Uses [`BuildContext::cached_layer`] with declarative traits [`CacheDiff`] for invalidation and [`TryMigrate`]
/// for migration logic.
/// The behavior here can be manually assembled using the provided struct [`Meta`] and functions:
///
/// - [`invalid_metadata_action`]
/// - [`restored_layer_action`]
///
/// In addition to default behavior it also ensures that the metadata is updated.
///
/// The return is a [`LayerRef`] as if you had manually assembled your own [`BuildContext::cached_layer`]
/// call. This allows users to be flexible in how and when the layer is modified and to abstract layer
/// creation away if necessary.
///
/// Guarantees that new metadata is always written (prevents accidentally reading one struct type and
/// writing a different one). It also provides a standard interface to define caching behavior via
/// the [`CacheDiff`] and [`TryMigrate`] traits:
///
/// - The [`TryMigrate`] trait is for handling invalid metadata:
///   When old metadata from cache is invalid, we try to load it into a known older version and then migrate it
///   to the latest via `TryMigrate`. If that fails, the layer is deleted and the error is returned. If it
///   succeeds, then the logic in `CacheDiff` below is applied.
///
/// The [`CacheDiff`] trait defines cache invalidation behavior when metadata is valid:
///   When a `CacheDiff::diff` is empty, the layer is kept and the old data is returned. Otherwise,
///   the layer is deleted and the changes are returned.
///
/// **TUTORIAL:** In the [`diff_migrate`] module docs
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DiffMigrateLayer {
    /// Whether the layer is intended for build.
    pub build: bool,
    /// Whether the layer is intended for launch.
    pub launch: bool,
}

impl DiffMigrateLayer {
    /// Writes metadata to a layer and returns a layer reference with info about prior cache state
    ///
    /// See the struct documentation for more information.
    ///
    /// # Errors
    ///
    /// Returns an error if libcnb cannot read or write the metadata.
    pub fn cached_layer<B, M>(
        self,
        layer_name: LayerName,
        context: &BuildContext<B>,
        metadata: &M,
    ) -> libcnb::Result<LayerRef<B, Meta<M>, Meta<M>>, B::Error>
    where
        B: libcnb::Buildpack,
        M: CacheDiff + TryMigrate + Serialize + Debug + Clone,
    {
        let layer_ref = context.cached_layer(
            layer_name,
            CachedLayerDefinition {
                build: self.build,
                launch: self.launch,
                invalid_metadata_action: &invalid_metadata_action,
                restored_layer_action: &|old: &M, _| restored_layer_action(old, metadata),
            },
        )?;
        layer_ref.write_metadata(metadata)?;
        Ok(layer_ref)
    }

    /// Renames cached layer while writing metadata to a layer
    ///
    /// When given a prior [`LayerRename::from`] that exists, but the [`LayerRename::to`]
    /// does not, then the contents of the prior layer will be copied before being deleted.
    ///
    /// After that this function callse [`cached_layer`] on the new layer.
    ///
    /// # Panics
    ///
    /// This function should not panic unless there's an internal bug.
    ///
    /// # Errors
    ///
    /// Returns an error if libcnb cannot read or write the metadata. Or if
    /// there's an error while copying from one path to another.
    pub fn cached_layer_rename<B, M>(
        self,
        layer_rename: LayerRename,
        context: &BuildContext<B>,
        metadata: &M,
    ) -> libcnb::Result<LayerRef<B, Meta<M>, Meta<M>>, B::Error>
    where
        B: libcnb::Buildpack,
        M: CacheDiff + TryMigrate + Serialize + Debug + Clone,
    {
        let LayerRename {
            to: to_layer,
            from: prior_layers,
        } = layer_rename;

        if let (Some(prior_dir), None) = (
            prior_layers
                .iter()
                .map(|layer_name| is_layer_on_disk(layer_name, context))
                .collect::<Result<Vec<Option<PathBuf>>, _>>()?
                .iter()
                .find_map(std::borrow::ToOwned::to_owned),
            is_layer_on_disk(&to_layer, context)?,
        ) {
            let to_dir = context.layers_dir.join(to_layer.as_str());
            std::fs::create_dir_all(&to_dir).map_err(LayerError::IoError)?;
            std::fs::rename(&prior_dir, &to_dir).map_err(LayerError::IoError)?;
            std::fs::rename(
                prior_dir.with_extension("toml"),
                to_dir.with_extension("toml"),
            )
            .map_err(LayerError::IoError)?;
        }
        self.cached_layer(to_layer, context, metadata)
    }
}

/// Represents when we want to move contents from one (or more) layer names
///
pub struct LayerRename {
    /// The desired layer name
    pub to: LayerName,
    /// A list of prior, possibly layer names
    pub from: Vec<LayerName>,
}

/// Returns Some(PathBuf) when the layer exists on disk
fn is_layer_on_disk<B>(
    layer_name: &LayerName,
    context: &BuildContext<B>,
) -> libcnb::Result<Option<PathBuf>, B::Error>
where
    B: libcnb::Buildpack,
{
    let path = context.layers_dir.join(layer_name.as_str());

    path.fs_err_try_exists()
        .map_err(|error| libcnb::Error::LayerError(LayerError::IoError(error)))
        .map(|exists| exists.then_some(path))
}

/// Standardizes formatting for layer cache clearing behavior
///
/// If the diff is empty, there are no changes and the layer is kept and the old data is returned
/// If the diff is not empty, the layer is deleted and the changes are listed
///
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

/// Either contains (old) metadata or a message describing the state
///
/// Why: The [`CachedLayerDefinition`] allows returning information about the cache state
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
/// When produced using functions in this module:
///
/// - Will only ever contain metadata when the cache is retained.
/// - Will contain a message when the cache is cleared, describing why it was cleared.
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
    use libcnb::generic::{GenericMetadata, GenericPlatform};
    use libcnb::layer::{EmptyLayerCause, InvalidMetadataAction, LayerState, RestoredLayerAction};
    use magic_migrate::{migrate_toml_chain, try_migrate_deserializer_chain, Migrate, TryMigrate};
    use std::convert::Infallible;
    /// Struct for asserting the behavior of `CacheBuddy`
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

    struct FakeBuildpack;
    impl libcnb::Buildpack for FakeBuildpack {
        type Platform = GenericPlatform;
        type Metadata = GenericMetadata;
        type Error = Infallible;

        fn detect(
            &self,
            _context: libcnb::detect::DetectContext<Self>,
        ) -> libcnb::Result<libcnb::detect::DetectResult, Self::Error> {
            todo!()
        }

        fn build(
            &self,
            _context: BuildContext<Self>,
        ) -> libcnb::Result<libcnb::build::BuildResult, Self::Error> {
            todo!()
        }
    }

    #[test]
    fn test_migrate_layer_name_works_if_prior_dir_does_not_exist() {
        let temp = tempfile::tempdir().unwrap();
        let context = temp_build_context::<FakeBuildpack>(
            temp.path(),
            include_str!("../../../buildpacks/ruby/buildpack.toml"),
        );

        let result = DiffMigrateLayer {
            build: true,
            launch: true,
        }
        .cached_layer_rename(
            LayerRename {
                to: layer_name!("new"),
                from: vec![layer_name!("does_not_exist")],
            },
            &context,
            &TestMetadata {
                value: "hello".to_string(),
            },
        )
        .unwrap();

        assert!(matches!(result.state, LayerState::Empty { cause: _ }));
    }

    #[test]
    fn test_migrate_layer_name_copies_old_data() {
        let temp = tempfile::tempdir().unwrap();
        let old_layer_name = layer_name!("old");
        let new_layer_name = layer_name!("new");
        let context = temp_build_context::<FakeBuildpack>(
            temp.path(),
            include_str!("../../../buildpacks/ruby/buildpack.toml"),
        );

        // First write
        let result = DiffMigrateLayer {
            build: true,
            launch: true,
        }
        .cached_layer(
            old_layer_name.clone(),
            &context,
            &TestMetadata {
                value: "hello".to_string(),
            },
        )
        .unwrap();

        assert!(matches!(
            result.state,
            LayerState::Empty {
                cause: EmptyLayerCause::NewlyCreated
            }
        ));

        assert!(context
            .layers_dir
            .join(old_layer_name.as_str())
            .fs_err_try_exists()
            .unwrap());

        assert!(!context
            .layers_dir
            .join(new_layer_name.as_str())
            .fs_err_try_exists()
            .unwrap());

        let result = DiffMigrateLayer {
            build: true,
            launch: true,
        }
        .cached_layer_rename(
            LayerRename {
                to: new_layer_name.clone(),
                from: vec![old_layer_name],
            },
            &context,
            &TestMetadata {
                value: "hello".to_string(),
            },
        )
        .unwrap();

        assert!(matches!(result.state, LayerState::Restored { cause: _ }));
        assert!(context
            .layers_dir
            .join(new_layer_name.as_str())
            .fs_err_try_exists()
            .unwrap());
    }

    #[test]
    fn test_diff_migrate() {
        let temp = tempfile::tempdir().unwrap();
        let context = temp_build_context::<FakeBuildpack>(
            temp.path(),
            include_str!("../../../buildpacks/ruby/buildpack.toml"),
        );

        // First write
        let result = DiffMigrateLayer {
            build: true,
            launch: true,
        }
        .cached_layer(
            layer_name!("testing"),
            &context,
            &TestMetadata {
                value: "hello".to_string(),
            },
        )
        .unwrap();
        assert!(matches!(
            result.state,
            LayerState::Empty {
                cause: EmptyLayerCause::NewlyCreated
            }
        ));

        // Second write, preserve the contents
        let result = DiffMigrateLayer {
            build: true,
            launch: true,
        }
        .cached_layer(
            layer_name!("testing"),
            &context,
            &TestMetadata {
                value: "hello".to_string(),
            },
        )
        .unwrap();
        let LayerState::Restored { cause } = &result.state else {
            panic!("Expected restored layer")
        };
        assert_eq!(cause.as_ref(), "Using cache");

        // Third write, change the data
        let result = DiffMigrateLayer {
            build: true,
            launch: true,
        }
        .cached_layer(
            layer_name!("testing"),
            &context,
            &TestMetadata {
                value: "world".to_string(),
            },
        )
        .unwrap();

        let LayerState::Empty {
            cause: EmptyLayerCause::RestoredLayerAction { cause },
        } = &result.state
        else {
            panic!("Expected empty layer with restored layer action");
        };
        assert_eq!(
            cause.as_ref(),
            "Clearing cache due to change: value (hello to world)"
        );
    }

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

    /// Takes in a directory and returns a minimal build context for use in testing caching behavior
    ///
    /// # Panics
    ///
    /// - If a context cannot be created
    fn temp_build_context<B: libcnb::Buildpack>(
        from_dir: impl AsRef<std::path::Path>,
        buildpack_toml_string: &str,
    ) -> libcnb::build::BuildContext<B> {
        let base_dir = from_dir.as_ref().to_path_buf();
        let layers_dir = base_dir.join("layers");
        let app_dir = base_dir.join("app_dir");
        let platform_dir = base_dir.join("platform_dir");
        let buildpack_dir = base_dir.join("buildpack_dir");
        for dir in [&app_dir, &layers_dir, &buildpack_dir, &platform_dir] {
            std::fs::create_dir_all(dir).unwrap();
        }

        let target = libcnb::Target {
            os: String::new(),
            arch: String::new(),
            arch_variant: None,
            distro_name: String::new(),
            distro_version: String::new(),
        };
        let platform =
            <<B as libcnb::Buildpack>::Platform as libcnb::Platform>::from_path(&platform_dir)
                .unwrap();
        let buildpack_descriptor: libcnb::data::buildpack::ComponentBuildpackDescriptor<
            <B as libcnb::Buildpack>::Metadata,
        > = toml::from_str(buildpack_toml_string).unwrap();
        let buildpack_plan = libcnb::data::buildpack_plan::BuildpackPlan {
            entries: Vec::<libcnb::data::buildpack_plan::Entry>::new(),
        };
        let store = None;

        libcnb::build::BuildContext {
            layers_dir,
            app_dir,
            buildpack_dir,
            target,
            platform,
            buildpack_plan,
            buildpack_descriptor,
            store,
        }
    }
}

use commons::display::SentenceList;
use libcnb::build::BuildContext;
use libcnb::layer::{CachedLayerDefinition, InvalidMetadataAction, LayerRef, RestoredLayerAction};

/// Default behavior for a cached layer, ensures new metadata is always written
///
/// The metadadata must implement `MetadataDiff` and `TryMigrate` in addition
/// to the typical `Serialize` and `Debug` traits
pub(crate) fn cached_layer_write_metadata<M, B>(
    layer_name: libcnb::data::layer::LayerName,
    context: &BuildContext<B>,
    metadata: &'_ M,
) -> libcnb::Result<LayerRef<B, String, String>, B::Error>
where
    B: libcnb::Buildpack,
    M: MetadataDiff + magic_migrate::TryMigrate + serde::ser::Serialize + std::fmt::Debug,
    <M as magic_migrate::TryMigrate>::Error: std::fmt::Display,
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

/// Given another metadata object, returns a list of differences between the two
///
/// If no differences, return an empty list
pub(crate) trait MetadataDiff {
    fn diff(&self, old: &Self) -> Vec<String>;
}

/// Standardizes formatting for layer cache clearing behavior
///
/// If the diff is empty, there are no changes and the layer is kept
/// If the diff is not empty, the layer is deleted and the changes are listed
pub(crate) fn restored_layer_action<T>(old: &T, now: &T) -> (RestoredLayerAction, String)
where
    T: MetadataDiff,
{
    let diff = now.diff(old);
    if diff.is_empty() {
        (RestoredLayerAction::KeepLayer, "Using cache".to_string())
    } else {
        (
            RestoredLayerAction::DeleteLayer,
            format!(
                "Clearing cache due to {changes}: {differences}",
                changes = if diff.len() > 1 { "changes" } else { "change" },
                differences = SentenceList::new(&diff)
            ),
        )
    }
}

/// Standardizes formatting for invalid metadata behavior
///
/// If the metadata can be migrated, it is replaced with the migrated version
/// If an error occurs, the layer is deleted and the error displayed
/// If no migration is possible, the layer is deleted and the invalid metadata is displayed
pub(crate) fn invalid_metadata_action<T, S>(invalid: &S) -> (InvalidMetadataAction<T>, String)
where
    T: magic_migrate::TryMigrate,
    S: serde::ser::Serialize + std::fmt::Debug,
    // TODO: Enforce Display + Debug in the library
    <T as magic_migrate::TryMigrate>::Error: std::fmt::Display,
{
    let invalid = toml::to_string(invalid);
    match invalid {
        Ok(toml) => match T::try_from_str_migrations(&toml) {
            Some(Ok(migrated)) => (
                InvalidMetadataAction::ReplaceMetadata(migrated),
                "Replaced metadata".to_string(),
            ),
            Some(Err(error)) => (
                InvalidMetadataAction::DeleteLayer,
                format!("Clearing cache due to metadata migration error: {error}"),
            ),
            None => (
                InvalidMetadataAction::DeleteLayer,
                format!("Clearing cache due to invalid metadata ({toml:?})"),
            ),
        },
        Err(error) => (
            InvalidMetadataAction::DeleteLayer,
            format!("Clearing cache due to invalid metadata serialization error: {error}"),
        ),
    }
}

/// Takes in a directory and returns a minimal build context for use in testing shared caching behavior
///
/// Intented only for use with this buildpack, but meant to be used by multiple layers to assert caching behavior.
#[cfg(test)]
pub(crate) fn temp_build_context<B: libcnb::Buildpack>(
    from_dir: impl AsRef<std::path::Path>,
) -> BuildContext<B> {
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
    let buildpack_toml_string = include_str!("../../buildpack.toml");
    let platform =
        <<B as libcnb::Buildpack>::Platform as libcnb::Platform>::from_path(&platform_dir).unwrap();
    let buildpack_descriptor: libcnb::data::buildpack::ComponentBuildpackDescriptor<
        <B as libcnb::Buildpack>::Metadata,
    > = toml::from_str(buildpack_toml_string).unwrap();
    let buildpack_plan = libcnb::data::buildpack_plan::BuildpackPlan {
        entries: Vec::<libcnb::data::buildpack_plan::Entry>::new(),
    };
    let store = None;

    BuildContext {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RubyBuildpack;
    use libcnb::data::layer_name;
    use libcnb::layer::{EmptyLayerCause, LayerState};
    use magic_migrate::{migrate_toml_chain, Migrate};
    use serde::Deserializer;

    /// Struct for asserting the behavior of `cached_layer_write_metadata`
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    struct TestMetadata {
        value: String,
    }
    impl MetadataDiff for TestMetadata {
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
    fn test_cached_layer_write_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let context = temp_build_context::<RubyBuildpack>(temp.path());

        // First write
        let result = cached_layer_write_metadata(
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
        let result = cached_layer_write_metadata(
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
        assert_eq!(cause, "Using cache");

        // Third write, change the data
        let result = cached_layer_write_metadata(
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
            cause,
            "Clearing cache due to change: value (hello to world)"
        );
    }
}

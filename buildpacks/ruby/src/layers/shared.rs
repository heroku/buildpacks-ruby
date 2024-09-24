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
    match T::try_from_str_migrations(
        &toml::to_string(invalid).expect("TOML deserialization of GenericMetadata"),
    ) {
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
            format!("Clearing cache due to invalid metadata ({invalid:?})"),
        ),
    }
}

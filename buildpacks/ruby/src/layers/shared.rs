use commons::display::SentenceList;
use libcnb::layer::{InvalidMetadataAction, RestoredLayerAction};

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

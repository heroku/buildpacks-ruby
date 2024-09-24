use libcnb::layer::InvalidMetadataAction;

/// Given another metadata object, returns a list of differences between the two
///
/// If no differences, return an empty list
pub(crate) trait MetadataDiff {
    fn diff(&self, old: &Self) -> Vec<String>;
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

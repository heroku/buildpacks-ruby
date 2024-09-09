mod configure_env_layer;
mod default_env_layer;

use libcnb::layer::{IntoAction, InvalidMetadataAction};

pub use self::configure_env_layer::ConfigureEnvLayer;
pub use self::default_env_layer::DefaultEnvLayer;

pub enum MetadataMigrationFYI<T> {
    Migrated(T, String),
    Delete(String),
}

impl<T, E> IntoAction<InvalidMetadataAction<T>, String, E> for MetadataMigrationFYI<T> {
    fn into_action(self) -> Result<(InvalidMetadataAction<T>, String), E> {
        match self {
            MetadataMigrationFYI::Migrated(metadata, reason) => {
                Ok((InvalidMetadataAction::ReplaceMetadata(metadata), reason))
            }
            MetadataMigrationFYI::Delete(reason) => {
                Ok((InvalidMetadataAction::DeleteLayer, reason))
            }
        }
    }
}

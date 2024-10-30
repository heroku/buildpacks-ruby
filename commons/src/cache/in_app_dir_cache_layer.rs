use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// # Caches a folder in the application directory
///
/// Layers are used for caching, however layers cannot be inside of the app directory.
/// This layer can be used to hold a directory's contents so they are preserved
/// between deploys.
///
/// The primary usecase of this is for caching assets. After `rake assets:precompile` runs
/// file in `<app-dir>/public/assets` need to be preserved between deploys. This allows
/// for faster deploys, and also allows for prior generated assets to remain on the system
///  until "cleaned."
///
///  Historically, sprockets will keep 3 versions of old files on disk. This
///  allows for emails, that might live a long time, to reference a specific SHA of an
///  asset.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct Metadata {
    pub(crate) app_dir_path: PathBuf,
}

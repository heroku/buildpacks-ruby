use byte_unit::{n_mib_bytes, Byte};
use std::path::PathBuf;

/// Configure behavior of a cached path
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheConfig {
    /// Path to the directory you want to cache
    pub path: PathBuf,

    /// Prevent cache size from growing unbounded. Files over the limit
    /// will be removed in order of least recently modified
    pub limit: Byte,

    /// Specify what happens to the application path while it's being stored
    pub keep_path: KeepPath,
}

/// Indicates whether we want the cache to be available at runtime or not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeepPath {
    /// Keep the application directory where it is, copy files to the cache
    Runtime,

    /// Remove the application directory from disk, move it to the cache
    BuildOnly,
}

/// Returns a `Byte` value containing the number
/// of mebibytes given.
#[must_use]
pub fn mib(n_mebibytes: u128) -> Byte {
    Byte::from_bytes(n_mib_bytes!(n_mebibytes))
}

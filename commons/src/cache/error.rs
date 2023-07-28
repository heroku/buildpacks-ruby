use std::path::PathBuf;

#[allow(clippy::module_name_repetitions)]
#[derive(thiserror::Error, Debug)]
pub enum CacheError {
    #[error("Cached path not in application directory: {0}")]
    CachedPathNotInAppPath(String),

    #[error("Invalid layer name: {0}")]
    InvalidLayerName(libcnb::data::layer::LayerNameError),

    #[error(
        "Could not copy cached files into application.\nFrom: {cache} to {path}\nError: {error}"
    )]
    CopyCacheToAppError {
        path: PathBuf,
        cache: PathBuf,
        error: fs_extra::error::Error,
    },

    #[error("Could not copy files from the application to cache.\nFrom: {path} To: {cache}\nError: {error}")]
    CopyAppToCacheError {
        path: PathBuf,
        cache: PathBuf,
        error: fs_extra::error::Error,
    },

    #[error("Could not move files out of the application to the cache.\nFrom: {path} To: {cache}\nError: {error}")]
    DestructiveMoveAppToCacheError {
        path: PathBuf,
        cache: PathBuf,
        error: fs_extra::error::Error,
    },

    #[error("Error occured while ensuring a directory existed in your application directory.\nDirectory: {path}\nError: {error}")]
    CannotCreateAppDir {
        path: PathBuf,
        error: std::io::Error,
    },

    #[error("System error: Cannot create cache dir {0}")]
    CannotCreateCacheDir(std::io::Error),

    #[error("Internal error: Could not create dir glob pattern: {0}")]
    InternalBadGlobError(glob::PatternError),

    #[error("Internal error: Could not construct layer: {0}")]
    InternalLayerError(String),

    #[error(
        "System error: The OS does not support the retreiving `mtime` information from files: {0}"
    )]
    MtimeUnsupportedOS(std::io::Error),

    #[error("System error: Could not retrieve metadata from file {path}.\nError: {error}")]
    CannotReadMetadata {
        path: PathBuf,
        error: std::io::Error,
    },

    #[error("System error: Cannot remove file while running LRU cleaner.\nError: {0}")]
    CannotRemoveFileLRU(std::io::Error),
}

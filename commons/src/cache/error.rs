use std::path::PathBuf;

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
        error: cp_r::Error,
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

    #[error("IO error: {0}")]
    IoError(std::io::Error),

    #[error("An internal error occured while creating a dir glob pattern: {0}")]
    InternalBadGlobError(glob::PatternError),

    #[error("An internal error occured while constructing the layer: {0}")]
    InternalLayerError(String),

    #[error("The OS does not support the retreiving `mtime` information from files: {0}")]
    MtimeUnsupportedOS(std::io::Error),
}

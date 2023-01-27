#![allow(clippy::module_name_repetitions)]
mod app_cache;
mod app_cache_collection;
mod clean;
mod config;
mod error;
mod in_app_dir_cache_layer;

pub use crate::cache::app_cache::AppCache;
pub use crate::cache::app_cache::CacheState;
pub use crate::cache::app_cache::{build, PathState};
pub use crate::cache::app_cache_collection::AppCacheCollection;
pub use crate::cache::clean::FilesWithSize;
pub use crate::cache::config::CacheConfig;
pub use crate::cache::config::{mib, KeepPath};
pub use crate::cache::error::CacheError;

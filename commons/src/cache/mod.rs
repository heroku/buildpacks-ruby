mod app_cache;
mod app_cache_collection;
mod clean;
mod config;
mod error;
mod in_app_dir_cache_layer;

pub use crate::cache::app_cache::{build, PathState};
pub use crate::cache::clean::FilesWithSize;
pub use crate::cache::config::{mib, KeepPath};

#[allow(clippy::module_name_repetitions)]
#[allow(clippy::useless_attribute)]
pub use crate::cache::error::CacheError;

#[allow(clippy::module_name_repetitions)]
#[allow(clippy::useless_attribute)]
pub use crate::cache::app_cache::CacheState;

#[allow(clippy::module_name_repetitions)]
#[allow(clippy::useless_attribute)]
pub use crate::cache::app_cache::AppCache;

#[allow(clippy::module_name_repetitions)]
#[allow(clippy::useless_attribute)]
pub use crate::cache::config::CacheConfig;

#[allow(clippy::module_name_repetitions)]
#[allow(clippy::useless_attribute)]
pub use crate::cache::app_cache_collection::AppCacheCollection;

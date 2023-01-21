use crate::in_app_dir_cache::{CacheError, DirCache, FilesWithSize, InAppDirCache, State};
use byte_unit::{Byte, ByteUnit};
use libcnb::{build::BuildContext, Buildpack};
use libherokubuildpack::log as user;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppCacheCollection {
    caches: Vec<(DirCache, CacheConfig)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeepAppPath {
    Runtime,
    BuildOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheConfig {
    pub path: PathBuf,
    pub limit: Byte,
    pub on_store: KeepAppPath,
}

/// App Cache Collection
///
/// Load and store multiple cache's from an application's directory. This essentially acts
/// as a group of `InAppDirCache` that run together
///
/// Used for loading/unloading asset cache and communicating what's happening to the user.
///
/// Default logging is provided for each operation.
///
/// ```rust
///# use libcnb::build::{BuildContext, BuildResult, BuildResultBuilder};
///# use libcnb::data::launch::{LaunchBuilder, ProcessBuilder};
///# use libcnb::data::process_type;
///# use libcnb::detect::{DetectContext, DetectResult, DetectResultBuilder};
///# use libcnb::generic::{GenericError, GenericMetadata, GenericPlatform};
///# use libcnb::{buildpack_main, Buildpack};
///# use libcnb::data::layer_name;
///# use libcnb::data::layer::LayerName;
///
///# pub(crate) struct HelloWorldBuildpack;
///
/// use commons::app_cache_collection::{AppCacheCollection, CacheConfig, KeepAppPath};
/// use byte_unit::Byte;
///
///# impl Buildpack for HelloWorldBuildpack {
///#     type Platform = GenericPlatform;
///#     type Metadata = GenericMetadata;
///#     type Error = GenericError;
///
///#     fn detect(&self, _context: DetectContext<Self>) -> libcnb::Result<DetectResult, Self::Error> {
///#         todo!()
///#     }
///
///#     fn build(&self, context: BuildContext<Self>) -> libcnb::Result<BuildResult, Self::Error> {
///         let cache = AppCacheCollection::new_and_load(
///             &context,
///             [
///                 CacheConfig {
///                     path: context.app_dir.join("public").join("assets"),
///                     limit: Byte::from_bytes(byte_unit::n_mib_bytes!(100)),
///                     on_store: KeepAppPath::Runtime,
///                 },
///                 CacheConfig {
///                     path: context.app_dir.join("tmp").join("cache").join("assets"),
///                     limit: Byte::from_bytes(byte_unit::n_mib_bytes!(100)),
///                     on_store: KeepAppPath::BuildOnly,
///                 },
///             ],
///         ).unwrap();
///
///         // Do something worth caching here
///
///         cache.store().unwrap();
///
///#        todo!()
///#     }
///# }
///
/// ```
impl AppCacheCollection {
    /// # Errors
    ///
    /// - Error if the cache layer cannot be created
    /// - Error if loading (copying) files from the cache to the app fails
    /// - Error if app or cache directory cannot be created (for example, due to permissions)
    pub fn new_and_load<B: Buildpack>(
        context: &BuildContext<B>,
        config: impl IntoIterator<Item = CacheConfig>,
    ) -> Result<Self, CacheError> {
        let caches = config
            .into_iter()
            .map(|config| {
                InAppDirCache::new_and_load(context, &config.path).map(|cache| {
                    Self::log_load(&cache);

                    (cache, config)
                })
            })
            .collect::<Result<Vec<(DirCache, CacheConfig)>, _>>()?;

        Ok(Self { caches })
    }

    /// # Errors
    ///
    /// Returns an error if the cache cannot be moved or copied, for example
    /// due to file permissions or another process deleting the target directory.
    /// Returns an error if cleaning the cache directory cannot
    /// be completed. For example due to file permissions.
    pub fn store(&self) -> Result<(), CacheError> {
        for (cache, config) in &self.caches {
            Self::log_store(cache);

            match config.on_store {
                KeepAppPath::Runtime => cache.copy_app_path_to_cache()?,
                KeepAppPath::BuildOnly => cache.destructive_move_app_path_to_cache()?,
            };

            if let Some(removed) = cache.lru_clean(config.limit)? {
                Self::log_clean(cache, config, &removed);
            }
        }

        Ok(())
    }

    fn log_load(cache: &DirCache) {
        let path = cache.app_path.display();

        match cache.state {
            State::NewEmpty => user::log_info(format!("Creating cache for {path}")),
            State::ExistsEmpty => user::log_info(format!("Loading (empty) cache for {path}")),
            State::ExistsWithContents => user::log_info(format!("Loading cache for {path}")),
        }
    }

    fn log_store(cache: &DirCache) {
        let path = cache.app_path.display();
        if cache.is_app_dir_empty() {
            user::log_info(format!("Storing cache for (empty) {path}"));
        } else {
            user::log_info(format!("Storing cache for {path}"));
        }
    }

    fn log_clean(cache: &DirCache, config: &CacheConfig, removed: &FilesWithSize) {
        let path = cache.app_path.display();
        let limit = config.limit.get_adjusted_unit(ByteUnit::MiB);
        let removed_len = removed.files.len();
        let removed_size = removed.get_adjusted_unit(ByteUnit::MiB);

        user::log_info(format!("Cache exceeded {limit} limit by {removed_size}"));
        user::log_info(format!(
            "Removing {removed_len} files from the cache for {path}",
        ));
    }
}

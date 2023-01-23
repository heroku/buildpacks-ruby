use crate::in_app_dir_cache::{CacheError, DirCache, FilesWithSize, InAppDirCache, State};
use byte_unit::{Byte, ByteUnit};
use libcnb::{build::BuildContext, Buildpack};
use std::path::PathBuf;

#[derive(Debug)]
pub struct AppCacheCollection {
    caches: Vec<(DirCache, CacheConfig)>,
    log_func: LogFunc,
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
///                     keep_app_path: KeepAppPath::Runtime,
///                 },
///                 CacheConfig {
///                     path: context.app_dir.join("tmp").join("cache").join("assets"),
///                     limit: Byte::from_bytes(byte_unit::n_mib_bytes!(100)),
///                     keep_app_path: KeepAppPath::BuildOnly,
///                 },
///             ],
///             |log| println!("{log}"),
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
        logger: impl Fn(&str) + 'static,
    ) -> Result<Self, CacheError> {
        let caches = config
            .into_iter()
            .map(|config| {
                InAppDirCache::new_and_load(context, &config.path).map(|cache| (cache, config))
            })
            .collect::<Result<Vec<(DirCache, CacheConfig)>, _>>()?;

        let log_func = LogFunc(Box::new(logger));
        let out = Self { caches, log_func };
        for (cache, _) in &out.caches {
            out.log_load(cache);
        }
        Ok(out)
    }

    /// # Errors
    ///
    /// Returns an error if the cache cannot be moved or copied, for example
    /// due to file permissions or another process deleting the target directory.
    /// Returns an error if cleaning the cache directory cannot
    /// be completed. For example due to file permissions.
    pub fn store(&self) -> Result<(), CacheError> {
        for (cache, config) in &self.caches {
            self.log_store(cache);

            match config.keep_app_path {
                KeepAppPath::Runtime => cache.copy_app_path_to_cache()?,
                KeepAppPath::BuildOnly => cache.destructive_move_app_path_to_cache()?,
            };

            if let Some(removed) = cache.lru_clean(config.limit)? {
                self.log_clean(cache, config, &removed);
            }
        }

        Ok(())
    }

    fn log_load(&self, cache: &DirCache) {
        let path = cache.app_path.display();

        match cache.state {
            State::NewEmpty => self.log_func.log(&format!("Creating cache for {path}")),
            State::ExistsEmpty => self
                .log_func
                .log(&format!("Loading (empty) cache for {path}")),
            State::ExistsWithContents => self.log_func.log(&format!("Loading cache for {path}")),
        }
    }

    fn log_store(&self, cache: &DirCache) {
        let path = cache.app_path.display();
        if cache.is_app_dir_empty() {
            self.log_func
                .log(&format!("Storing cache for (empty) {path}"));
        } else {
            self.log_func.log(&format!("Storing cache for {path}"));
        }
    }

    fn log_clean(&self, cache: &DirCache, config: &CacheConfig, removed: &FilesWithSize) {
        let path = cache.app_path.display();
        let limit = config.limit.get_adjusted_unit(ByteUnit::MiB);
        let removed_len = removed.files.len();
        let removed_size = removed.get_adjusted_unit(ByteUnit::MiB);

        self.log_func.log(&format!(
            "Detected cache size exceeded (over {limit} limit by {removed_size}) for {path}"
        ));
        self.log_func.log(&format!(
            "Removed {removed_len} files from the cache for {path}",
        ));
    }
}

/// Indicates whether we want the cache to be available at runtime or not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeepAppPath {
    /// Keep the application directory where it is, copy files to the cache
    Runtime,

    /// Remove the application directory from disk, move it to the cache
    BuildOnly,
}

/// Configure behavior of a cached path
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheConfig {
    /// Path to the directory you want to cache
    pub path: PathBuf,

    /// Prevent cache size from growing unbounded. Files over the limit
    /// will be removed in order of least recently modified
    pub limit: Byte,

    /// Specify what happens to the application path while it's being stored
    pub keep_app_path: KeepAppPath,
}

/// Small wrapper for storing a logging function
///
/// Implements: Debug. Does not implement Clone or Eq
struct LogFunc(Box<dyn Fn(&str)>);

impl LogFunc {
    fn log(&self, s: &str) {
        (self).0(s);
    }
}

impl std::fmt::Debug for LogFunc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("LogFunc").field(&"Fn(&str)").finish()
    }
}

use crate::cache::{AppCache, CacheConfig, CacheError, CacheState, FilesWithSize, PathState};
use libcnb::{build::BuildContext, Buildpack};

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
/// use commons::cache::{AppCacheCollection, CacheConfig, KeepPath, mib};
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
///                     limit: mib(100),
///                     keep_path: KeepPath::Runtime,
///                 },
///                 CacheConfig {
///                     path: context.app_dir.join("tmp").join("cache").join("assets"),
///                     limit: mib(100),
///                     keep_path: KeepPath::BuildOnly,
///                 },
///             ],
///             |log| println!("{log}"),
///         ).unwrap();
///
///         // Do something worth caching here
///
///         cache.save_and_clean().unwrap();
///
///#        todo!()
///#     }
///# }
///
/// ```
#[derive(Debug)]
pub struct AppCacheCollection {
    collection: Vec<AppCache>,
    log_func: LogFunc,
}

impl AppCacheCollection {
    /// Store multiple application paths in the cache
    ///
    /// # Errors
    ///
    /// - Error if the cache layer cannot be created
    /// - Error if loading (copying) files from the cache to the app fails
    /// - Error if app or cache directory cannot be created (for example, due to permissions)
    pub fn new_and_load<'a, B: Buildpack>(
        context: &BuildContext<B>,
        config: impl IntoIterator<Item = CacheConfig>,
        logger: impl Fn(&str) + 'static,
    ) -> Result<Self, CacheError> {
        let log_func = LogFunc(Box::new(logger));
        let caches = config
            .into_iter()
            .map(|config| {
                AppCache::new_and_load(context, config).map(|store| {
                    log_load(&log_func, &store);
                    store
                })
            })
            .collect::<Result<Vec<AppCache>, CacheError>>()?;

        Ok(Self {
            collection: caches,
            log_func,
        })
    }

    /// # Errors
    ///
    /// Returns an error if the cache cannot be moved or copied, for example
    /// due to file permissions or another process deleting the target directory.
    /// Returns an error if cleaning the cache directory cannot
    /// be completed. For example due to file permissions.
    pub fn save_and_clean(&self) -> Result<(), CacheError> {
        for store in &self.collection {
            self.log_save(store);

            if let Some(removed) = store.save_and_clean()? {
                self.log_clean(store, &removed);
            }
        }

        Ok(())
    }

    fn log_save(&self, store: &AppCache) {
        let path = store.path().display();

        self.log_func.log(&match store.path_state() {
            PathState::Empty => format!("Storing cache for (empty) {path}"),
            PathState::HasFiles => format!("Storing cache for {path}"),
        });
    }

    fn log_clean(&self, store: &AppCache, removed: &FilesWithSize) {
        let path = store.path().display();
        let limit = store.limit();
        let removed_len = removed.files.len();
        let removed_size = removed.adjusted_bytes();

        self.log_func.log(&format!(
            "Detected cache size exceeded (over {limit} limit by {removed_size}) for {path}"
        ));
        self.log_func.log(&format!(
            "Removed {removed_len} files from the cache for {path}",
        ));
    }
}

fn log_load(log_func: &LogFunc, store: &AppCache) {
    let path = store.path().display();

    log_func.log(&match store.cache_state() {
        CacheState::NewEmpty => format!("Creating cache for {path}"),
        CacheState::ExistsEmpty => format!("Loading (empty) cache for {path}"),
        CacheState::ExistsWithContents => format!("Loading cache for {path}"),
    });
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

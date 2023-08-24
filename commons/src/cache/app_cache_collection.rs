use crate::cache::{AppCache, CacheConfig, CacheError, CacheState, FilesWithSize, PathState};
use crate::output::layer_logger::LayerLogger;
use libcnb::{build::BuildContext, Buildpack};
use std::fmt::Debug;

/// App Cache Collection
///
/// Load and store multiple cache's from an application's directory. This essentially acts
/// as a group of `InAppDirCache` that run together
///
/// Used for loading/unloading asset cache and communicating what's happening to the user.
///
/// Default logging is provided for each operation.
///
#[derive(Debug)]
pub struct AppCacheCollection {
    log: LayerLogger,
    collection: Vec<AppCache>,
}

impl AppCacheCollection {
    /// Store multiple application paths in the cache
    ///
    /// # Errors
    ///
    /// - Error if the cache layer cannot be created
    /// - Error if loading (copying) files from the cache to the app fails
    /// - Error if app or cache directory cannot be created (for example, due to permissions)
    pub fn new_and_load<B: Buildpack>(
        context: &BuildContext<B>,
        config: impl IntoIterator<Item = CacheConfig>,
        log: LayerLogger,
    ) -> Result<Self, CacheError> {
        let caches = config
            .into_iter()
            .map(|config| {
                AppCache::new_and_load(context, config).map(|store| {
                    log_load(&log, &store);
                    store
                })
            })
            .collect::<Result<Vec<AppCache>, CacheError>>()?;

        Ok(Self {
            collection: caches,
            log,
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

        self.log.lock().step(match store.path_state() {
            PathState::Empty => format!("Storing cache for (empty) {path}"),
            PathState::HasFiles => format!("Storing cache for {path}"),
        });
    }

    fn log_clean(&self, store: &AppCache, removed: &FilesWithSize) {
        let path = store.path().display();
        let limit = store.limit();
        let removed_len = removed.files.len();
        let removed_size = removed.adjusted_bytes();

        self.log.lock().step(format!(
            "Detected cache size exceeded (over {limit} limit by {removed_size}) for {path}"
        ));
        self.log.lock().step(format!(
            "Removed {removed_len} files from the cache for {path}",
        ));
    }
}

fn log_load(log: &LayerLogger, store: &AppCache) {
    let path = store.path().display();

    log.lock().step(match store.cache_state() {
        CacheState::NewEmpty => format!("Creating cache for {path}"),
        CacheState::ExistsEmpty => format!("Loading (empty) cache for {path}"),
        CacheState::ExistsWithContents => format!("Loading cache for {path}"),
    });
}

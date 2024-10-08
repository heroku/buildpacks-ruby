use crate::cache::{AppCache, CacheConfig, CacheError, CacheState, PathState};
use crate::output::{interface::SectionLogger, section_log as log};
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
    ) -> Result<Self, CacheError> {
        let caches = config
            .into_iter()
            .map(|config| {
                AppCache::new_and_load(context, config).inspect(|store| {
                    let path = store.path().display();

                    log::log_step(match store.cache_state() {
                        CacheState::NewEmpty => format!("Creating cache for {path}"),
                        CacheState::ExistsEmpty => format!("Loading (empty) cache for {path}"),
                        CacheState::ExistsWithContents => format!("Loading cache for {path}"),
                    });
                })
            })
            .collect::<Result<Vec<AppCache>, CacheError>>()?;

        Ok(Self { collection: caches })
    }

    /// # Errors
    ///
    /// Returns an error if the cache cannot be moved or copied, for example
    /// due to file permissions or another process deleting the target directory.
    /// Returns an error if cleaning the cache directory cannot
    /// be completed. For example due to file permissions.
    pub fn save_and_clean(&self) -> Result<(), CacheError> {
        for store in &self.collection {
            let path = store.path().display();

            log::log_step(match store.path_state() {
                PathState::Empty => format!("Storing cache for (empty) {path}"),
                PathState::HasFiles => format!("Storing cache for {path}"),
            });

            if let Some(removed) = store.save_and_clean()? {
                let path = store.path().display();
                let limit = store.limit();
                let removed_len = removed.files.len();
                let removed_size = removed.adjusted_bytes();

                log::log_step(format!(
                    "Detected cache size exceeded (over {limit} limit by {removed_size}) for {path}"
                ));
                log::log_step(format!(
                    "Removed {removed_len} files from the cache for {path}",
                ));
            }
        }

        Ok(())
    }
}

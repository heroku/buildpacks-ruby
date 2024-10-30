use crate::cache::clean::{lru_clean, FilesWithSize};
use crate::cache::in_app_dir_cache_layer;
use crate::cache::{CacheConfig, CacheError, KeepPath};
use byte_unit::{AdjustedByte, Byte, UnitType};
use fs_extra::dir::CopyOptions;
use libcnb::build::BuildContext;
use libcnb::data::layer::LayerName;
use libcnb::layer::{CachedLayerDefinition, InvalidMetadataAction, RestoredLayerAction};
use std::path::Path;
use std::path::PathBuf;
use walkdir::WalkDir;

use tempfile as _;

/// Store data generated in the `<app_dir>` between builds
///
/// Requires `ByteUnit` from the `byte-unit` crate to configure.
/// To store multiple directories use `AppCacheCollection`.
///
/// Example of storing public/assets directory, limiting cache size to 100mb
/// and keeping the public/assets directory visible at runtime:
///
///```rust
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
///  use commons::cache::{AppCache, CacheConfig, KeepPath, mib};
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
///         let config = CacheConfig {
///             path: context.app_dir.join("public").join("assets"),
///             limit: mib(100),
///             keep_path: KeepPath::Runtime
///         };
///
///         let store = AppCache::new_and_load(&context, config).unwrap();
///
///         std::fs::write(context.app_dir.join("public").join("assets").join("lol"), "hahaha");
///
///         store.save_and_clean().unwrap();
///
///#        todo!()
///#     }
///# }
/// ```
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AppCache {
    /// Path to the directory you want to cache
    path: PathBuf,

    /// Prevent cache size from growing unbounded. Files over the limit
    /// will be removed in order of least recently modified
    limit: Byte,

    /// Directory where files will be cached
    cache: PathBuf,

    /// Specify what happens to the application path while it's being stored
    keep_path: KeepPath,

    /// Status of the cache directory when struct was instantiated
    cache_state: CacheState,
}

impl AppCache {
    /// Create an `AppCache` from context and config
    ///
    /// # Errors
    ///
    /// - If the cache or applications directory cannot be created
    ///   (possibly due to permissions error).
    /// - If files from the cache cannot be loaded into the
    ///   application directory (possibly due to permissions error).
    /// - Internal errors from libcnb layer creation and execution.
    pub fn new_and_load<B: libcnb::Buildpack>(
        context: &BuildContext<B>,
        config: CacheConfig,
    ) -> Result<Self, CacheError> {
        let store = build(context, config)?;
        store.load()?;

        Ok(store)
    }

    /// The path in the application being cached
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The value (in adjusted bytes) of the limit for the cached directory
    #[must_use]
    pub fn limit(&self) -> AdjustedByte {
        self.limit.get_appropriate_unit(UnitType::Binary)
    }

    /// The state of the cache directory when the object was created
    #[must_use]
    pub fn cache_state(&self) -> &CacheState {
        &self.cache_state
    }

    /// Stores the contetents from the path into the cache
    ///
    /// Depending on the state of `keep_path` the contents
    /// of `path` will persist into build time, or be removed
    /// so they're only present at build time.
    ///
    /// # Errors
    ///
    /// - If the files cannot be moved/coppied into the cache
    ///   then then an error will be raised.
    pub fn save(&self) -> Result<&AppCache, CacheError> {
        save(self)?;
        match self.keep_path {
            KeepPath::Runtime => {}
            KeepPath::BuildOnly => {
                fs_err::remove_dir_all(&self.path).map_err(CacheError::IoError)?;
            }
        };

        Ok(self)
    }

    /// Load files from cache into the path
    ///
    /// Files in the path will take precedent over files in the
    /// cache.
    /// Ensures that both cache and path exist an disk.
    ///
    /// # Errors
    ///
    /// - If files cannot be moved from the cache to the path
    ///   then an error will be raised.
    pub fn load(&self) -> Result<&Self, CacheError> {
        fs_err::create_dir_all(&self.path).map_err(CacheError::IoError)?;
        fs_err::create_dir_all(&self.cache).map_err(CacheError::IoError)?;

        fs_extra::dir::copy(
            &self.cache,
            &self.path,
            &CopyOptions {
                overwrite: false,
                skip_exist: true,
                copy_inside: true,
                content_only: true,
                ..CopyOptions::default()
            },
        )
        .map_err(|error| CacheError::CopyCacheToAppError {
            path: self.path.clone(),
            cache: self.cache.clone(),
            error,
        })?;
        copy_mtime_r(&self.cache, &self.path)?;

        fs_err::remove_dir_all(&self.cache).map_err(CacheError::IoError)?;

        Ok(self)
    }

    /// Save and Clean out files in the cache above the configured limit
    ///
    /// Files will first be moved from the path into the cache
    /// according to the configured `keep_path` policy.
    ///
    /// Afterwards, files in the cache will be cleaned:
    /// If the cache directory is above the given `limit` then
    /// files will be deleted in LRU order based on disk mtime.
    ///
    /// If any files were removed in this process then they will
    /// be returned via `Some`. If no files were removed, `None`
    /// will be returned indicating the directory is not yet
    /// at the stated limit.
    ///
    /// # Errors
    ///
    /// - If files cannot be deleted an error will be raised
    /// - If the operating system does not support the `mtime` an
    ///   error will be raised.
    /// - If metadata of a file cannot be read, an error will be raised
    pub fn save_and_clean(&self) -> Result<Option<FilesWithSize>, CacheError> {
        self.save()?;
        lru_clean(&self.cache, self.limit)
    }

    /// Returns an enum representing the state
    /// of the target path.
    #[must_use]
    pub fn path_state(&self) -> PathState {
        if is_empty_dir(&self.path) {
            PathState::Empty
        } else {
            PathState::HasFiles
        }
    }
}

/// The state of the cache directory when the
/// layer is created.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheState {
    /// Cache was just created, no files in it
    NewEmpty,

    /// Cache was previously created, however there are no files in it
    ExistsEmpty,

    /// Cache was previously created, it is non-empty
    ExistsWithContents,
}

/// Current state of the path
pub enum PathState {
    /// No files are present in the path
    Empty,

    /// Path has files present
    HasFiles,
}

/// Converts a `CacheConfig` into an `AppCache`
///
/// Same as `AppCache::new_and_load` without loading
/// files from the cache into the path.
///
/// # Errors
///
/// - If the layer cannot be created
#[allow(deprecated)]
pub fn build<B: libcnb::Buildpack>(
    context: &BuildContext<B>,
    config: CacheConfig,
) -> Result<AppCache, CacheError> {
    let CacheConfig {
        path,
        limit,
        keep_path,
    } = config;

    let layer_name = create_layer_name(&context.app_dir, &path)?;
    let create_state = layer_name_cache_state(&context.layers_dir, &layer_name);

    let metadata = in_app_dir_cache_layer::Metadata {
        app_dir_path: path.clone(),
    };
    let cache = context
        .cached_layer(
            layer_name,
            CachedLayerDefinition {
                build: true,
                launch: true,
                invalid_metadata_action: &|_| InvalidMetadataAction::DeleteLayer,
                restored_layer_action: &|old: &in_app_dir_cache_layer::Metadata, _| {
                    if old == &metadata {
                        RestoredLayerAction::KeepLayer
                    } else {
                        RestoredLayerAction::DeleteLayer
                    }
                },
            },
        )
        .map_err(|error| CacheError::InternalLayerError(format!("{error:?}")))
        .map(|layer_ref| layer_ref.path())?;

    Ok(AppCache {
        path,
        limit,
        cache,
        keep_path,
        cache_state: create_state,
    })
}

/// Copy contents of application path into the cache
///
/// This action preserves the contents in the application path.
/// Files from the application path are considered
/// cannonical and will overwrite files with the same name in the
/// cache.
///
/// # Errors
///
/// - If the copy command fails an `IoExtraError` will be raised.
fn save(store: &AppCache) -> Result<&AppCache, CacheError> {
    fs_extra::dir::copy(
        &store.path,
        &store.cache,
        &CopyOptions {
            overwrite: true,
            copy_inside: true,  // Recursive
            content_only: true, // Don't copy top level directory name
            ..CopyOptions::default()
        },
    )
    .map_err(|error| CacheError::CopyAppToCacheError {
        path: store.path.clone(),
        cache: store.cache.clone(),
        error,
    })?;

    copy_mtime_r(&store.path, &store.cache)?;

    Ok(store)
}

/// Copies the mtime information from a path to another path
///
/// This is information used for the LRU cleaner so that older files are removed first.
fn copy_mtime_r(from: &Path, to_path: &Path) -> Result<(), CacheError> {
    for entry in WalkDir::new(from).into_iter().filter_map(Result::ok) {
        let relative = entry
            .path()
            .strip_prefix(from)
            .expect("Walkdir path should return path with prefix of called root");

        let mtime = entry
            .metadata()
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))
            .map(|metadata| filetime::FileTime::from_last_modification_time(&metadata))
            .map_err(|error| CacheError::Mtime {
                from: entry.path().to_path_buf(),
                to_path: to_path.join(relative),
                error,
            })?;

        filetime::set_file_mtime(to_path.join(relative), mtime).map_err(|error| {
            CacheError::Mtime {
                from: entry.path().to_path_buf(),
                to_path: to_path.join(relative),
                error,
            }
        })?;
    }
    Ok(())
}

/// Converts a path inside of an app to a valid layer name for libcnb.
fn create_layer_name(app_root: &Path, path: &Path) -> Result<LayerName, CacheError> {
    let name = path
        .strip_prefix(app_root)
        .map_err(|_| {
            CacheError::CachedPathNotInAppPath(format!(
                "Expected cached app path {} to be in {} but it was not",
                path.display(),
                app_root.display(),
            ))
        })?
        .iter()
        .map(std::ffi::OsStr::to_string_lossy)
        .collect::<Vec<_>>()
        .join("_");

    format!("cache_{name}")
        .parse()
        .map_err(CacheError::InvalidLayerName)
}

/// Determines if a cache directory in a layer previously existed or not.
fn layer_name_cache_state(layers_base_dir: &Path, layer_name: &LayerName) -> CacheState {
    let layer_dir = layers_base_dir.join(layer_name.as_str());

    if !layer_dir.exists() {
        CacheState::NewEmpty
    } else if is_empty_dir(&layer_dir) {
        CacheState::ExistsEmpty
    } else {
        CacheState::ExistsWithContents
    }
}

/// Returns true if path has no valid readable files
fn is_empty_dir(path: &Path) -> bool {
    if let Ok(read_dir) = fs_err::read_dir(path) {
        let dir_has_files = read_dir
            .filter_map(std::result::Result::ok)
            .any(|entry| entry.path().exists());

        !dir_has_files
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use filetime::FileTime;
    use libcnb::data::layer_name;
    use std::str::FromStr;

    #[test]
    fn test_to_layer_name() {
        let dir = PathBuf::from_str("muh_base").unwrap();
        let layer = create_layer_name(&dir, &dir.join("my").join("input")).unwrap();
        assert_eq!(layer_name!("cache_my_input"), layer);
    }

    #[test]
    fn test_layer_name_cache_state() {
        let layer_name = layer_name!("name");
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path();
        assert_eq!(
            CacheState::NewEmpty,
            layer_name_cache_state(path, &layer_name)
        );
        fs_err::create_dir_all(path.join(layer_name.as_str())).unwrap();
        assert_eq!(
            CacheState::ExistsEmpty,
            layer_name_cache_state(path, &layer_name)
        );
        fs_err::write(path.join(layer_name.as_str()).join("file"), "data").unwrap();
        assert_eq!(
            CacheState::ExistsWithContents,
            layer_name_cache_state(path, &layer_name)
        );
    }

    #[test]
    fn test_load_does_not_clobber_files() {
        let tmpdir = tempfile::tempdir().unwrap();
        let cache_path = tmpdir.path().join("cache");
        let app_path = tmpdir.path().join("app");
        fs_err::create_dir_all(&cache_path).unwrap();
        fs_err::create_dir_all(&app_path).unwrap();

        fs_err::write(app_path.join("a.txt"), "app").unwrap();
        fs_err::write(cache_path.join("a.txt"), "cache").unwrap();

        let store = AppCache {
            path: app_path.clone(),
            cache: cache_path,
            limit: Byte::from_u64(512),
            keep_path: KeepPath::Runtime,
            cache_state: CacheState::NewEmpty,
        };

        store.load().unwrap();

        let contents = fs_err::read_to_string(app_path.join("a.txt")).unwrap();
        assert_eq!("app", contents);
    }

    #[test]
    fn test_copying_back_to_cache() {
        let tmpdir = tempfile::tempdir().unwrap();
        let cache_path = tmpdir.path().join("cache");
        let app_path = tmpdir.path().join("app");
        let store = AppCache {
            path: app_path.clone(),
            cache: cache_path,
            limit: Byte::from_u64(512),
            keep_path: KeepPath::Runtime,
            cache_state: CacheState::NewEmpty,
        };

        assert!(is_empty_dir(&app_path)); // Assert empty dir

        store.load().unwrap();

        assert!(is_empty_dir(&app_path)); // Assert empty dir

        fs_err::write(app_path.join("lol.txt"), "hahaha").unwrap();

        // Test copy logic from app to cache
        assert!(!store.cache.join("lol.txt").exists());
        assert!(store.path.join("lol.txt").exists());

        store.save().unwrap();

        assert!(store.cache.join("lol.txt").exists());
        assert!(store.path.join("lol.txt").exists());
    }

    #[test]
    fn test_moving_back_to_cache() {
        let tmpdir = tempfile::tempdir().unwrap();
        let cache_path = tmpdir.path().join("cache");
        let app_path = tmpdir.path().join("app");
        let store = AppCache {
            path: app_path.clone(),
            cache: cache_path,
            limit: Byte::from_u64(512),
            keep_path: KeepPath::BuildOnly,
            cache_state: CacheState::NewEmpty,
        };

        assert!(is_empty_dir(&app_path));

        store.load().unwrap();

        assert!(is_empty_dir(&app_path));

        fs_err::write(app_path.join("lol.txt"), "hahaha").unwrap();

        // Test copy logic from app to cache
        assert!(!store.cache.join("lol.txt").exists());
        assert!(store.path.join("lol.txt").exists());

        store.save().unwrap();

        assert!(store.cache.join("lol.txt").exists());
        assert!(!store.path.join("lol.txt").exists());
    }

    #[test]
    fn mtime_preserved_keep_path_build_only() {
        let mtime = FileTime::from_unix_time(1000, 0);
        let tmpdir = tempfile::tempdir().unwrap();
        let filename = "totoro.txt";
        let app_path = tmpdir.path().join("app");
        let cache_path = tmpdir.path().join("cache");

        fs_err::create_dir_all(&cache_path).unwrap();
        fs_err::create_dir_all(&app_path).unwrap();

        let store = AppCache {
            path: app_path.clone(),
            cache: cache_path.clone(),
            limit: Byte::from_u64(512),
            keep_path: KeepPath::BuildOnly,
            cache_state: CacheState::NewEmpty,
        };

        fs_err::write(app_path.join(filename), "catbus").unwrap();
        filetime::set_file_mtime(app_path.join(filename), mtime).unwrap();

        store.save().unwrap();

        // Cache file mtime should match app file mtime
        let actual = fs_err::metadata(cache_path.join(filename))
            .map(|metadata| FileTime::from_last_modification_time(&metadata))
            .unwrap();
        assert_eq!(mtime, actual);

        // File was removed already after save
        assert!(!store.path.join(filename).exists());

        store.load().unwrap();

        // App path mtime should match cache file mtime
        let actual = fs_err::metadata(app_path.join(filename))
            .map(|metadata| FileTime::from_last_modification_time(&metadata))
            .unwrap();
        assert_eq!(mtime, actual);
    }

    #[test]
    fn mtime_preserved_keep_path_runtime() {
        let mtime = FileTime::from_unix_time(1000, 0);
        let tmpdir = tempfile::tempdir().unwrap();
        let filename = "totoro.txt";
        let app_path = tmpdir.path().join("app");
        let cache_path = tmpdir.path().join("cache");

        fs_err::create_dir_all(&cache_path).unwrap();
        fs_err::create_dir_all(&app_path).unwrap();

        let store = AppCache {
            path: app_path.clone(),
            cache: cache_path.clone(),
            limit: Byte::from_u64(512),
            keep_path: KeepPath::Runtime,
            cache_state: CacheState::NewEmpty,
        };

        fs_err::write(app_path.join(filename), "catbus").unwrap();
        filetime::set_file_mtime(app_path.join(filename), mtime).unwrap();

        store.save().unwrap();

        // Cache file mtime should match app file mtime
        let actual = fs_err::metadata(cache_path.join(filename))
            .map(|metadata| FileTime::from_last_modification_time(&metadata))
            .unwrap();
        assert_eq!(mtime, actual);

        // Remove app path to test loading
        fs_err::remove_dir_all(&app_path).unwrap();
        assert!(!store.path.join(filename).exists());

        store.load().unwrap();

        // App path mtime should match cache file mtime
        let actual = fs_err::metadata(app_path.join(filename))
            .map(|metadata| FileTime::from_last_modification_time(&metadata))
            .unwrap();
        assert_eq!(mtime, actual);
    }
}

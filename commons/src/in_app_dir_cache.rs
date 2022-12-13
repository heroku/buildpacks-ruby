use crate::in_app_dir_cache_layer::InAppDirCacheLayer;
use byte_unit::Byte;
use fs_extra::dir::CopyOptions;
use libcnb::build::BuildContext;
use libcnb::data::layer::LayerName;
use libcnb::Buildpack;
use std::fs::Metadata;
use std::marker::PhantomData;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::path::PathBuf;

/// Store data generated in the `<app_dir>` between builds
///
/// Example:
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
///use commons::in_app_dir_cache::InAppDirCacheWithLayer;
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
///         let public_assets_cache = InAppDirCacheWithLayer::new_and_load(
///             &context,
///             &context.app_dir.join("public").join("assets"),
///         ).unwrap();
///
///         std::fs::write(context.app_dir.join("public").join("assets").join("lol"), "hahaha");
///
///         public_assets_cache.copy_app_path_to_cache();
///
///#        todo!()
///#     }
///# }
/// ```
///
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InAppDirCache {
    pub app_path: PathBuf,
    pub cache_path: PathBuf,
}

pub struct InAppDirCacheWithLayer<B> {
    buildpack: PhantomData<B>,
}

#[derive(thiserror::Error, Debug)]
pub enum InAppDirCacheError {
    #[error("Cached path not in application directory: {0}")]
    CachedPathNotInAppPath(String),

    #[error("Invalid layer name: {0}")]
    InvalidLayerName(libcnb::data::layer::LayerNameError),

    #[error("IO error: {0}")]
    IoExtraError(fs_extra::error::Error),

    #[error("IO error: {0}")]
    IoError(std::io::Error),

    #[error("Cannot convert OsString into UTF-8 string: {0}")]
    OsStringErr(String),
}

fn to_layer_name(base: &Path, app_path: &Path) -> Result<LayerName, InAppDirCacheError> {
    let name = app_path
        .strip_prefix(base)
        .map_err(|_| {
            InAppDirCacheError::CachedPathNotInAppPath(format!(
                "Expected cached app path {} to be in {} but it was not",
                app_path.display(),
                base.display(),
            ))
        })?
        .iter()
        .map(|p| p.to_string_lossy())
        .collect::<Vec<_>>()
        .join("_");

    format!("cache_{}", name)
        .parse()
        .map_err(InAppDirCacheError::InvalidLayerName)
}

impl<B: Buildpack> InAppDirCacheWithLayer<B> {
    pub fn new_and_load(
        context: &BuildContext<B>,
        app_path: &Path,
    ) -> Result<InAppDirCache, InAppDirCacheError> {
        let app_path = app_path.to_path_buf();

        let cache_path = context
            .handle_layer(
                to_layer_name(&context.app_dir, &app_path)?,
                InAppDirCacheLayer::new(app_path.clone()),
            )
            .expect("Internal error with layer")
            .path;

        let out = InAppDirCache {
            app_path,
            cache_path,
        };
        out.mkdir_p()?;
        out.move_cache_to_app()?;

        Ok(out)
    }
}

impl InAppDirCache {
    fn mkdir_p(&self) -> Result<(), InAppDirCacheError> {
        std::fs::create_dir_all(&self.app_path).map_err(InAppDirCacheError::IoError)?;
        std::fs::create_dir_all(&self.cache_path).map_err(InAppDirCacheError::IoError)?;

        Ok(())
    }

    fn move_cache_to_app(&self) -> Result<&Self, InAppDirCacheError> {
        fs_extra::dir::move_dir(
            &self.cache_path,
            &self.app_path,
            &CopyOptions {
                overwrite: false,
                skip_exist: true,
                copy_inside: true,
                content_only: true,
                ..CopyOptions::default()
            },
        )
        .map_err(InAppDirCacheError::IoExtraError)?;

        Ok(self)
    }

    pub fn destructive_move_app_path_to_cache(&self) -> Result<&Self, InAppDirCacheError> {
        println!("---> Storing cache for {}", self.app_path.display());
        fs_extra::dir::move_dir(
            &self.app_path,
            &self.cache_path,
            &CopyOptions {
                overwrite: false,
                skip_exist: true,
                copy_inside: true,
                content_only: true,
                ..CopyOptions::default()
            },
        )
        .map_err(InAppDirCacheError::IoExtraError)?;

        Ok(self)
    }

    pub fn copy_app_path_to_cache(&self) -> Result<&Self, InAppDirCacheError> {
        println!("---> Storing cache for {}", self.app_path.display());
        fs_extra::dir::copy(
            &self.app_path,
            &self.cache_path,
            &CopyOptions {
                overwrite: false,
                skip_exist: true,
                copy_inside: true,

                content_only: true,
                ..CopyOptions::default()
            },
        )
        .map_err(InAppDirCacheError::IoExtraError)?;

        Ok(self)
    }

    pub fn least_recently_used_files_above_limit(
        &self,
        max_bytes: Byte,
    ) -> Result<FilesWithSize, InAppDirCacheError> {
        Self::least_recently_used_files_above_limit_from_path(&self.cache_path, max_bytes)
    }

    fn least_recently_used_files_above_limit_from_path(
        cache_path: &Path,
        max_bytes: Byte,
    ) -> Result<FilesWithSize, InAppDirCacheError> {
        let max_bytes = max_bytes.get_bytes();
        let glob_string = cache_path
            .join("**/*")
            .into_os_string()
            .into_string()
            .map_err(|e| InAppDirCacheError::OsStringErr(e.to_string_lossy().to_string()))?;

        let mut files = glob::glob(&glob_string)
            .expect("Bad glob pattern")
            .filter_map(Result::ok)
            .filter(|p| p.is_file()) // Means we aren't removing empty directories
            .map(|p| {
                std::fs::metadata(&p)
                    .map(|m| (m, p))
                    .map_err(InAppDirCacheError::IoError)
            })
            .collect::<Result<Vec<(Metadata, PathBuf)>, _>>()?;

        let bytes = files
            .iter()
            .map(|(metadata, _)| u128::from(metadata.size()))
            .sum::<u128>();

        if bytes >= max_bytes {
            let mut current_bytes = bytes;
            files.sort_by(|(meta_a, _), (meta_b, _)| {
                let a_modified = meta_a
                    .modified()
                    .expect("Operating system must support file mtime");
                let b_modified = meta_b
                    .modified()
                    .expect("Operating system must support file mtime");

                a_modified.cmp(&b_modified)
            });

            Ok(FilesWithSize {
                bytes,
                files: files
                    .iter()
                    .take_while(|(metadata, _)| {
                        current_bytes -= u128::from(metadata.size());
                        current_bytes >= max_bytes
                    })
                    .map(|(_, path)| path.clone())
                    .collect::<Vec<PathBuf>>(),
            })
        } else {
            Ok(FilesWithSize::default())
        }
    }
}

#[derive(Debug, Eq, PartialEq, Default)]
pub struct FilesWithSize {
    pub bytes: u128,
    pub files: Vec<PathBuf>,
}

impl FilesWithSize {
    pub fn to_byte(&self) -> Byte {
        Byte::from_bytes(self.bytes)
    }
    pub fn clean(&self) -> Result<(), InAppDirCacheError> {
        for file in &self.files {
            std::fs::remove_file(file).map_err(InAppDirCacheError::IoError)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use byte_unit::n_mib_bytes;
    use libcnb::data::layer_name;

    use super::*;

    pub fn touch_file(path: &PathBuf, f: impl FnOnce(&PathBuf)) {
        if let Some(parent) = path.parent() {
            println!("path {:?}", path);
            println!("parent {:?}", parent);
            if !parent.exists() {
                std::fs::create_dir_all(parent).unwrap();
            }
        }
        std::fs::write(path, "").unwrap();
        f(path);
        std::fs::remove_file(path).unwrap();
    }

    // fn buildpack_toml<'a>() -> &'a str {
    //     include_str!("../../buildpacks/ruby/buildpack.toml")
    // }

    // #[test]
    // fn test_makes_layer_correctly() {
    //     let tmp_context = crate::test_helper::TempContext::new(buildpack_toml());

    //     let app_path = tmp_context.build.app_dir.join("hahaha");

    //     assert!(!app_path.exists());
    //     let cache = InAppDirCacheWithLayername::new_and_load(
    //         &tmp_context.build,
    //         layer_name!("lol"),
    //         &app_path,
    //     );

    //     assert!(cache.app_path.exists()); // Creates app path
    //     assert_eq!(cache.app_path, app_path);
    //     assert_eq!(cache.cache_path, tmp_context.build.layers_dir.join("lol"));
    // }

    // #[test]
    // fn test_makes_app_dir_if_it_doesnt_already_exist() {
    //     let tmp_context = crate::test_helper::TempContext::new(buildpack_toml());
    //     let cache = InAppDirCacheWithLayername::new_and_load(
    //         &tmp_context.build,
    //         layer_name!("lol"),
    //         &tmp_context
    //             .build
    //             .app_dir
    //             .join("make")
    //             .join("path")
    //             .join("here"),
    //     );

    //     assert!(cache.cache_path.exists());
    //     assert!(cache.app_path.exists());
    // }

    // #[test]
    // fn test_populates_app_dir_automatically() {
    //     let tmp_context = crate::test_helper::TempContext::new(buildpack_toml());

    //     let lol_layer = tmp_context.build.layers_dir.clone();
    //     let app_path = tmp_context.build.app_dir.join("muh_path");

    //     std::fs::write(&lol_layer.join("lol.txt"), "lol").unwrap();

    //     assert!(!app_path.exists());

    //     InAppDirCacheWithLayername::new_and_load(&tmp_context.build, layer_name!("lol"), &app_path);

    //     assert!(app_path.exists());
    // }
    #[test]
    fn test_to_layer_name() {
        let dir = PathBuf::from_str("muh_base").unwrap();
        let layer = to_layer_name(&dir, &dir.join("my").join("input")).unwrap();
        assert_eq!(layer_name!("cache_my_input"), layer);
    }

    #[test]
    fn test_copying_back_to_cache() {
        let tmpdir = tempfile::tempdir().unwrap();
        let cache_path = tmpdir.path().join("cache");
        let app_path = tmpdir.path().join("app");
        let cache = InAppDirCache {
            app_path: app_path.clone(),
            cache_path: cache_path.clone(),
        };
        cache.mkdir_p().unwrap();

        assert!(app_path.read_dir().unwrap().next().is_none()); // Assert empty dir
        cache.move_cache_to_app().unwrap();
        assert!(app_path.read_dir().unwrap().next().is_none()); // Assert dir not changed

        std::fs::write(app_path.join("lol.txt"), "hahaha").unwrap();

        // Test copy logic from app to cache
        assert!(!cache.cache_path.join("lol.txt").exists());
        assert!(cache_path.read_dir().unwrap().next().is_none());
        cache.copy_app_path_to_cache().unwrap();
        assert!(cache.cache_path.join("lol.txt").exists());
        assert!(cache.app_path.join("lol.txt").exists());
    }

    #[test]
    fn test_moving_back_to_cache() {
        let tmpdir = tempfile::tempdir().unwrap();
        let cache_path = tmpdir.path().join("cache");
        let app_path = tmpdir.path().join("app");
        let cache = InAppDirCache {
            app_path: app_path.clone(),
            cache_path: cache_path.clone(),
        };
        cache.mkdir_p().unwrap();

        assert!(app_path.read_dir().unwrap().next().is_none()); // Assert empty dir
        cache.move_cache_to_app().unwrap();
        assert!(app_path.read_dir().unwrap().next().is_none()); // Assert dir not changed

        std::fs::write(app_path.join("lol.txt"), "hahaha").unwrap();

        // Test copy logic from app to cache
        assert!(!cache.cache_path.join("lol.txt").exists());
        assert!(cache_path.read_dir().unwrap().next().is_none());
        cache.destructive_move_app_path_to_cache().unwrap();
        assert!(cache.cache_path.join("lol.txt").exists());
        assert!(!cache.app_path.join("lol.txt").exists());
    }

    #[test]
    fn test_lru_only_returns_based_on_size() {
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path().join("dir");

        std::fs::create_dir_all(&dir).unwrap();

        assert_eq!(
            InAppDirCache::least_recently_used_files_above_limit_from_path(
                &dir,
                Byte::from_bytes(n_mib_bytes!(0)),
            )
            .unwrap()
            .files
            .len(),
            0
        );

        touch_file(&dir.join("a"), |file| {
            let overage = InAppDirCache::least_recently_used_files_above_limit_from_path(
                &dir,
                Byte::from_bytes(n_mib_bytes!(0)),
            )
            .unwrap();
            assert_eq!(overage.files, vec![file.clone()]);

            let overage = InAppDirCache::least_recently_used_files_above_limit_from_path(
                &dir,
                Byte::from_bytes(n_mib_bytes!(10)),
            )
            .unwrap();
            assert_eq!(overage.files.len(), 0);
        });
    }

    #[test]
    fn test_lru_returns_older_files_first() {
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path().join("");

        touch_file(&dir.join("z_older"), |a| {
            touch_file(&dir.join("a_newer"), |b| {
                filetime::set_file_mtime(a, filetime::FileTime::from_unix_time(0, 0)).unwrap();
                filetime::set_file_mtime(b, filetime::FileTime::from_unix_time(1, 0)).unwrap();

                let overage = InAppDirCache::least_recently_used_files_above_limit_from_path(
                    &dir,
                    Byte::from_bytes(n_mib_bytes!(0)),
                )
                .unwrap();
                assert_eq!(overage.files, vec![a.clone(), b.clone()]);
            });
        });
    }

    #[test]
    fn test_lru_does_not_grab_directories() {
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path().join("");
        std::fs::create_dir_all(dir.join("preservation_society")).unwrap();
        let overage = InAppDirCache::least_recently_used_files_above_limit_from_path(
            &dir,
            Byte::from_bytes(n_mib_bytes!(0)),
        )
        .unwrap();
        assert_eq!(overage.files, Vec::<PathBuf>::new());
    }
}

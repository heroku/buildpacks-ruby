use crate::InAppDirCacheLayer;
use crate::RubyBuildpack;
use fs_extra::dir::CopyOptions;
use libcnb::build::BuildContext;
use libcnb::data::layer::LayerName;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::path::PathBuf;

use byte_unit::Byte;

/// Store data generated in the `<app_dir>` between builds
///
/// Example:
///
/// ```rust,no_run,not-actually-run-since-not-exposed-in-lib.rs
/// let public_assets_cache = InAppDirCacheWithLayername::new_and_load(
///     &context,
///     layer_name!("public_assets"),
///     &context.app_dir.join("public").join("assets"),
/// );
///
/// assets_precompile.call().unwrap();
///
/// public_assets_cache.to_cache();
/// ```
///
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InAppDirCache {
    pub app_path: PathBuf,
    pub cache_path: PathBuf,
}

pub struct InAppDirCacheWithLayername;

impl InAppDirCacheWithLayername {
    pub fn new_and_load(
        context: &BuildContext<RubyBuildpack>,
        name: LayerName,
        app_path: &Path,
    ) -> InAppDirCache {
        let app_path = app_path.to_path_buf();

        let cache_path = context
            .handle_layer(
                name,
                InAppDirCacheLayer {
                    app_dir_path: app_path.clone(),
                },
            )
            .unwrap()
            .path;

        let out = InAppDirCache {
            app_path,
            cache_path,
        };
        out.mkdir_p();
        out.move_cache_to_app();
        out
    }
}

impl InAppDirCache {
    fn mkdir_p(&self) {
        std::fs::create_dir_all(&self.app_path).unwrap();
        std::fs::create_dir_all(&self.cache_path).unwrap();
    }

    fn move_cache_to_app(&self) -> &Self {
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
        .unwrap();
        self
    }

    pub fn move_app_path_to_cache(&self) {
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
        .unwrap();
    }

    pub fn copy_app_path_to_cache(&self) {
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
        .unwrap();
    }

    pub fn least_recently_used_files_above_limit(&self, max_bytes: Byte) -> FilesWithSize {
        Self::least_recently_used_files_above_limit_from_path(&self.cache_path, max_bytes)
    }

    fn least_recently_used_files_above_limit_from_path(
        cache_path: &Path,
        max_bytes: Byte,
    ) -> FilesWithSize {
        let max_bytes = max_bytes.get_bytes();
        let glob_string = cache_path
            .join("**/*")
            .into_os_string()
            .into_string()
            .unwrap();

        let mut files = glob::glob(&glob_string)
            .expect("Bad glob pattern")
            .filter_map(Result::ok)
            .filter_map(|p| {
                // Note that this means we never clean empty directories
                if p.is_file() {
                    Some((std::fs::metadata(&p).unwrap(), p))
                } else {
                    None
                }
            })
            .collect::<Vec<(_, PathBuf)>>();

        let bytes = files
            .iter()
            .map(|(metadata, _)| u128::from(metadata.size()))
            .sum::<u128>();
        if bytes >= max_bytes {
            let mut current_bytes = bytes;
            files.sort_by(|(meta_a, _), (meta_b, _)| {
                meta_a.modified().unwrap().cmp(&meta_b.modified().unwrap())
            });

            FilesWithSize {
                bytes,
                files: files
                    .iter()
                    .take_while(|(metadata, _)| {
                        current_bytes -= u128::from(metadata.size());
                        current_bytes >= max_bytes
                    })
                    .map(|(_, path)| path.clone())
                    .collect::<Vec<PathBuf>>(),
            }
        } else {
            FilesWithSize::default()
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
    pub fn clean(&self) {
        for file in &self.files {
            std::fs::remove_file(file).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use byte_unit::n_mib_bytes;
    use libcnb::data::layer_name;

    use super::*;
    use crate::test_helper::touch_file;

    #[test]
    fn test_makes_layer_correctly() {
        let tmp_context =
            crate::test_helper::TempContext::new(include_str!("../../buildpack.toml"));

        let app_path = tmp_context.build.app_dir.join("hahaha");

        assert!(!app_path.exists());
        let cache = InAppDirCacheWithLayername::new_and_load(
            &tmp_context.build,
            layer_name!("lol"),
            &app_path,
        );

        assert!(cache.app_path.exists()); // Creates app path
        assert_eq!(cache.app_path, app_path);
        assert_eq!(cache.cache_path, tmp_context.build.layers_dir.join("lol"));
    }

    #[test]
    fn test_makes_app_dir_if_it_doesnt_already_exist() {
        let tmp_context =
            crate::test_helper::TempContext::new(include_str!("../../buildpack.toml"));
        let cache = InAppDirCacheWithLayername::new_and_load(
            &tmp_context.build,
            layer_name!("lol"),
            &tmp_context
                .build
                .app_dir
                .join("make")
                .join("path")
                .join("here"),
        );

        assert!(cache.cache_path.exists());
        assert!(cache.app_path.exists());
    }

    #[test]
    fn test_populates_app_dir_automatically() {
        let tmp_context =
            crate::test_helper::TempContext::new(include_str!("../../buildpack.toml"));

        let lol_layer = tmp_context.build.layers_dir.clone();
        let app_path = tmp_context.build.app_dir.join("muh_path");

        std::fs::write(&lol_layer.join("lol.txt"), "lol").unwrap();

        assert!(!app_path.exists());

        InAppDirCacheWithLayername::new_and_load(&tmp_context.build, layer_name!("lol"), &app_path);

        assert!(app_path.exists());
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
        cache.mkdir_p();

        assert!(app_path.read_dir().unwrap().next().is_none()); // Assert empty dir
        cache.move_cache_to_app();
        assert!(app_path.read_dir().unwrap().next().is_none()); // Assert dir not changed

        std::fs::write(app_path.join("lol.txt"), "hahaha").unwrap();

        // Test copy logic from app to cache
        assert!(!cache.cache_path.join("lol.txt").exists());
        assert!(cache_path.read_dir().unwrap().next().is_none());
        cache.copy_app_path_to_cache();
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
        cache.mkdir_p();

        assert!(app_path.read_dir().unwrap().next().is_none()); // Assert empty dir
        cache.move_cache_to_app();
        assert!(app_path.read_dir().unwrap().next().is_none()); // Assert dir not changed

        std::fs::write(app_path.join("lol.txt"), "hahaha").unwrap();

        // Test copy logic from app to cache
        assert!(!cache.cache_path.join("lol.txt").exists());
        assert!(cache_path.read_dir().unwrap().next().is_none());
        cache.move_app_path_to_cache();
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
            .files
            .len(),
            0
        );

        touch_file(&dir.join("a"), |file| {
            let overage = InAppDirCache::least_recently_used_files_above_limit_from_path(
                &dir,
                Byte::from_bytes(n_mib_bytes!(0)),
            );
            assert_eq!(overage.files, vec![file.clone()]);

            let overage = InAppDirCache::least_recently_used_files_above_limit_from_path(
                &dir,
                Byte::from_bytes(n_mib_bytes!(10)),
            );
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
                );
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
        );
        assert_eq!(overage.files, Vec::<PathBuf>::new());
    }
}

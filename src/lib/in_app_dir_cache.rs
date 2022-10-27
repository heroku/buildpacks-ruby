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
/// let public_assets_cache = InAppDirCache::new_and_load(
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
pub struct InAppDirCache {
    pub app_path: PathBuf,
    pub cache_path: PathBuf,
}

impl InAppDirCache {
    pub fn new_and_load(
        context: &BuildContext<RubyBuildpack>,
        name: LayerName,
        path: &Path,
    ) -> Self {
        let app_path = path.to_path_buf();
        let cache_path = context
            .handle_layer(
                name,
                InAppDirCacheLayer {
                    app_dir_path: app_path.clone(),
                },
            )
            .unwrap()
            .path;

        std::fs::create_dir_all(&app_path).unwrap();
        let out = Self {
            app_path,
            cache_path,
        };
        out.to_app();
        out
    }

    fn to_app(&self) -> &Self {
        fs_extra::dir::move_dir(
            &self.cache_path,
            &self.app_path,
            &CopyOptions {
                overwrite: false,
                skip_exist: true,
                copy_inside: true,
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

    use super::*;
    use crate::test_helper::touch_file;

    #[test]
    fn test_lru_only_returns_based_on_size() {
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path().join("");
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

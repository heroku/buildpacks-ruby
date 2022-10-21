use crate::InAppDirCacheLayer;
use crate::RubyBuildpack;
use fs_extra::dir::CopyOptions;
use libcnb::build::BuildContext;
use libcnb::data::layer::LayerName;
use std::path::Path;
use std::path::PathBuf;

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

    pub fn to_cache(&self) {
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
}

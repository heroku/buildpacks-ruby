use libcnb::Env;

use crate::RubyBuildpackError;

use crate::env_command::EnvCommand;
use crate::gem_list::GemList;
use crate::rake_detect::RakeDetect;
use std::path::Path;
use std::path::PathBuf;

use crate::InAppDirCacheLayer;
use crate::RubyBuildpack;
use fs_extra::dir::CopyOptions;
use libcnb::build::BuildContext;
use libcnb::data::layer::LayerName;
use libcnb::data::layer_name;

pub struct RakeApplicationTasksExecute;

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
    app_path: PathBuf,
    cache_path: PathBuf,
}

impl InAppDirCache {
    fn new_and_load(context: &BuildContext<RubyBuildpack>, name: LayerName, path: &Path) -> Self {
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

    fn to_cache(&self) {
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
}

impl RakeApplicationTasksExecute {
    pub fn call(
        context: &BuildContext<RubyBuildpack>,
        env: &Env,
    ) -> Result<(), RubyBuildpackError> {
        // ## Get list of gems and their versions from the system
        println!("---> Detecting gems");
        let gem_list =
            GemList::from_bundle_list(env).map_err(RubyBuildpackError::GemListGetError)?;

        // Get list of valid rake tasks
        println!("---> Detecting rake tasks");
        let rake_detect = RakeDetect::from_rake_command(env, true)
            .map_err(RubyBuildpackError::RakeDetectError)?;

        if rake_detect.has_task("assets:precompile") {
            let assets_precompile = EnvCommand::new("rake", &["assets:precompile", "--trace"], env);

            let public_assets_cache = InAppDirCache::new_and_load(
                context,
                layer_name!("public_assets"),
                &context.app_dir.join("public").join("assets"),
            );
            let fragments_cache = InAppDirCache::new_and_load(
                context,
                layer_name!("tmp_cache"),
                &context.app_dir.join("tmp").join("cache").join("assets"),
            );

            assets_precompile.call().unwrap();

            public_assets_cache.to_cache();
            fragments_cache.to_cache();
        } else {
            println!("    Rake task `rake assets:precompile` not found, skipping");
        }

        Ok(())
    }
}

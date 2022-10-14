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
use libcnb::data::layer_name;

pub struct RakeApplicationTasksExecute {
    public_assets: PathBuf,
    assets_fragments: PathBuf,
}

impl RakeApplicationTasksExecute {
    pub fn new(app_dir: &Path) -> Self {
        let public_assets = app_dir.join("public").join("assets");
        let assets_fragments = app_dir.join("tmp").join("cache").join("assets");

        RakeApplicationTasksExecute {
            public_assets,
            assets_fragments,
        }
    }

    fn get_cache(&self, public_cache: &PathBuf, fragment_cache: &PathBuf) {
        // Move contents into public/assets
        fs_extra::dir::move_dir(
            &public_cache,
            &self.public_assets,
            &CopyOptions {
                overwrite: false,
                skip_exist: true,
                copy_inside: true,
                ..CopyOptions::default()
            },
        )
        .unwrap();

        // Move contents into tmp/cache/assets
        fs_extra::dir::move_dir(
            &fragment_cache,
            &self.assets_fragments,
            &CopyOptions {
                overwrite: false,
                skip_exist: true,
                copy_inside: true,
                ..CopyOptions::default()
            },
        )
        .unwrap();
    }

    fn write_cache(&self, public_cache: &PathBuf, fragment_cache: &PathBuf) {
        // Cache public/assets
        fs_extra::dir::copy(
            &self.public_assets,
            &public_cache,
            &CopyOptions {
                overwrite: true,
                skip_exist: false,
                copy_inside: true,
                ..CopyOptions::default()
            },
        )
        .unwrap();

        // Cache tmp/cache/assets
        fs_extra::dir::move_dir(
            &self.assets_fragments,
            &fragment_cache,
            &CopyOptions {
                overwrite: false,
                skip_exist: true,
                copy_inside: true,
                ..CopyOptions::default()
            },
        )
        .unwrap();
    }

    pub fn call(
        &self,
        context: &BuildContext<RubyBuildpack>,
        env: &Env,
    ) -> Result<(), RubyBuildpackError> {
        let public_assets_cache = context
            .handle_layer(
                layer_name!("public_assets"),
                InAppDirCacheLayer {
                    app_dir_path: self.public_assets.clone(),
                },
            )
            .unwrap();

        let assets_fragments_cache = context
            .handle_layer(
                layer_name!("public_assets"),
                InAppDirCacheLayer {
                    app_dir_path: self.assets_fragments.clone(),
                },
            )
            .unwrap();

        // ## Get list of gems and their versions from the system
        println!("---> Detecting gems");
        let gem_list =
            GemList::from_bundle_list(env).map_err(RubyBuildpackError::GemListGetError)?;

        let has_sprockets = gem_list.has("sprockets");

        // Get list of valid rake tasks
        println!("---> Detecting rake tasks");
        let rake_detect = RakeDetect::from_rake_command(env, has_sprockets)
            .map_err(RubyBuildpackError::RakeDetectError)?;

        if rake_detect.has_task("assets:precompile") {
            let assets_precompile = EnvCommand::new("rake", &["assets:precompile", "--trace"], env);

            self.get_cache(&public_assets_cache.path, &assets_fragments_cache.path);
            assets_precompile.call().unwrap();
            self.write_cache(&public_assets_cache.path, &assets_fragments_cache.path);
        } else {
            println!("    Rake task `rake assets:precompile` not found, skipping");
        }

        Ok(())
    }
}

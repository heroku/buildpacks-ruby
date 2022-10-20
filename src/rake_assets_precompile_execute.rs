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

#[derive(Debug, Eq, PartialEq)]
enum CanRunRake {
    Ok,
    NoRakeGem,
    MissingRakefile,
}

fn detect_rake_can_run(has_rakefile: bool, has_rake: bool) -> CanRunRake {
    if has_rake {
        if has_rakefile {
            CanRunRake::Ok
        } else {
            CanRunRake::MissingRakefile
        }
    } else {
        CanRunRake::NoRakeGem
    }
}

fn has_rakefile(path: &Path) -> bool {
    ["rakefile", "Rakefile", "rakefile.rb;", "Rakefile.rb"]
        .iter()
        .any(|name| path.join(name).exists())
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

        println!("---> Detecting rake tasks");
        match detect_rake_can_run(has_rakefile(&context.app_dir), gem_list.has("rake")) {
            CanRunRake::NoRakeGem => {
                println!("---> Skipping rake task detection, add `gem 'rake'` to your Gemfile");
            }
            CanRunRake::MissingRakefile => {
                println!("    Rake task `rake assets:precompile` not found, skipping");
            }
            CanRunRake::Ok => {
                let rake_detect = RakeDetect::from_rake_command(env, true)
                    .map_err(RubyBuildpackError::RakeDetectError)?;
                if rake_detect.has_task("assets:precompile") {
                    let assets_precompile =
                        EnvCommand::new("rake", &["assets:precompile", "--trace"], env);

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

                    println!("    Rake task `rake assets:precompile` found, running");
                    let out = assets_precompile.stream().unwrap();
                    println!("{}", out.stderr);
                    println!("{}", out.stdout);

                    if rake_detect.has_task("assets:clean") {
                        println!("    Rake task `rake assets:clean` found, running");

                        let assets_clean =
                            EnvCommand::new("rake", &["assets:clean", "--trace"], env);
                        let out = assets_clean.stream().unwrap();
                        println!("{}", out.stderr);
                        println!("{}", out.stdout);

                        public_assets_cache.to_cache();
                        fragments_cache.to_cache();
                    } else {
                        println!("    Rake task `rake assets:clean` not found, skipping");
                        println!(
                            "    Not saving cache of  {}",
                            public_assets_cache.app_path.display()
                        );
                        println!(
                            "    Not saving cache of  {}",
                            fragments_cache.app_path.display()
                        );
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_rake_can_run() {
        assert_eq!(detect_rake_can_run(false, false), CanRunRake::NoRakeGem);
        assert_eq!(
            detect_rake_can_run(false, true),
            CanRunRake::MissingRakefile
        );
        assert_eq!(detect_rake_can_run(true, false), CanRunRake::NoRakeGem);
        assert_eq!(detect_rake_can_run(true, true), CanRunRake::Ok);
    }

    #[test]
    fn test_has_rakefile() {
        let tmpdir = tempfile::tempdir().unwrap();

        for name in &["rakefile", "Rakefile", "rakefile.rb;", "Rakefile.rb"] {
            let file = tmpdir.path().join(name);
            std::fs::write(&file, "").unwrap();
            assert!(
                has_rakefile(tmpdir.path()),
                "Expected `has_rakefile` to return true for '{}' but it did not",
                name
            );
            std::fs::remove_file(&file).unwrap();
        }

        assert!(!has_rakefile(tmpdir.path()));
    }
}

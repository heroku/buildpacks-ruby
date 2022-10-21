use libcnb::Env;

use crate::RubyBuildpackError;

use crate::lib::env_command::EnvCommand;
use crate::lib::gem_list::GemList;
use crate::lib::in_app_dir_cache::InAppDirCache;
use crate::lib::rake_detect::RakeDetect;
use std::path::Path;

use crate::RubyBuildpack;
use libcnb::build::BuildContext;
use libcnb::data::layer_name;

pub struct RakeApplicationTasksExecute;

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
        gem_list: &GemList,
        context: &BuildContext<RubyBuildpack>,
        env: &Env,
    ) -> Result<(), RubyBuildpackError> {
        match detect_rake_can_run(has_rakefile(&context.app_dir), gem_list.has("rake")) {
            CanRunRake::NoRakeGem => {
                println!("---> Skipping rake task detection, add `gem 'rake'` to your Gemfile");
            }
            CanRunRake::MissingRakefile => {
                println!("    Rake task `rake assets:precompile` not found, skipping");
            }
            CanRunRake::Ok => {
                println!("---> Detecting rake tasks");
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
                    assets_precompile.stream().unwrap();

                    if rake_detect.has_task("assets:clean") {
                        println!("    Rake task `rake assets:clean` found, running");

                        let assets_clean =
                            EnvCommand::new("rake", &["assets:clean", "--trace"], env);
                        assets_clean.stream().unwrap();

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

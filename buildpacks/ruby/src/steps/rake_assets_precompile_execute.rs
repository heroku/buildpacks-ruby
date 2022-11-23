use byte_unit::{Byte, ByteUnit};
use libcnb::Env;

use crate::RubyBuildpackError;

use commons::env_command::EnvCommand;
use commons::gem_list::GemList;
use commons::in_app_dir_cache::{InAppDirCache, InAppDirCacheWithLayer};
use commons::rake_detect::RakeDetect;
use std::path::Path;

use crate::RubyBuildpack;
use libcnb::build::BuildContext;

pub struct RakeApplicationTasksExecute;

#[derive(Debug, Eq, PartialEq)]
enum CanRunRake {
    Ok,
    NoRakeGem,
    MissingRakefile,
    AssetManifestSkip(Vec<PathBuf>),
}

#[derive(Debug, Eq, PartialEq)]
struct HasRakefile(bool);

#[derive(Debug, Eq, PartialEq)]
struct HasGem(bool);

#[derive(Debug, Eq, PartialEq)]
struct AssetManifestList(Vec<PathBuf>);

// Convert nested logic into a flat enum of possible states
// that represent whether or not `rake assets:precompile` can
// be run.
fn detect_rake_can_run(
    has_rakefile: &HasRakefile,
    has_rake_installed: &HasGem,
    asset_manifests: &AssetManifestList,
) -> CanRunRake {
    if asset_manifests.0.is_empty() {
        match has_rake_installed {
            HasGem(true) => match has_rakefile {
                HasRakefile(true) => CanRunRake::Ok,
                HasRakefile(false) => CanRunRake::MissingRakefile,
            },
            HasGem(false) => CanRunRake::NoRakeGem,
        }
    } else {
        CanRunRake::AssetManifestSkip(asset_manifests.0.clone())
    }
}

/// Checks directory for rakefile varients
fn dir_has_rakefile(path: &Path) -> HasRakefile {
    HasRakefile(
        ["rakefile", "Rakefile", "rakefile.rb;", "Rakefile.rb"]
            .iter()
            .any(|name| path.join(name).exists()),
    )
}

// Checks if GemList contains a reference to the rake gem
fn gem_list_has_rake(gem_list: &GemList) -> HasGem {
    HasGem(gem_list.has("rake"))
}
use glob::glob;
use std::path::PathBuf;

// Checks in public/assets if an existing manifest file exists
fn has_asset_manifest(app_dir: &Path) -> AssetManifestList {
    let manifests = [".sprockets-manifest-*.json", "manifest-*.json"]
        .iter()
        .map(|glob_pattern| app_dir.join("public").join("assets").join(glob_pattern))
        .map(|path| path.into_os_string().into_string().unwrap())
        .map(|string| glob(&string))
        .filter_map(Result::ok)
        .find_map(|paths| {
            let paths = paths
                .into_iter()
                .map(std::result::Result::unwrap)
                .collect::<Vec<PathBuf>>();

            if paths.is_empty() {
                None
            } else {
                Some(paths)
            }
        })
        .unwrap_or_default();
    AssetManifestList(manifests)
}

impl RakeApplicationTasksExecute {
    pub fn call(
        gem_list: &GemList,
        context: &BuildContext<RubyBuildpack>,
        env: &Env,
    ) -> Result<(), RubyBuildpackError> {
        match detect_rake_can_run(
            &dir_has_rakefile(&context.app_dir),
            &gem_list_has_rake(gem_list),
            &has_asset_manifest(&context.app_dir),
        ) {
            CanRunRake::NoRakeGem => {
                println!("---> Skipping rake task detection, add `gem 'rake'` to your Gemfile");
            }
            CanRunRake::MissingRakefile => {
                println!("    Rake task `rake assets:precompile` not found, skipping");
            }
            CanRunRake::AssetManifestSkip(paths) => {
                println!(
                    "    Manifest file(s) found {}. Skipping `rake assets:precompile`",
                    paths
                        .iter()
                        .map(|path| path.clone().into_os_string().into_string().unwrap())
                        .collect::<Vec<String>>()
                        .join(", ")
                );
            }
            CanRunRake::Ok => {
                println!("---> Detecting rake tasks");
                let rake_detect = RakeDetect::from_rake_command(env, true)
                    .map_err(RubyBuildpackError::RakeDetectError)?;
                if rake_detect.has_task("assets:precompile") {
                    let assets_precompile = EnvCommand::new(
                        "bundle",
                        &["exec", "rake", "assets:precompile", "--trace"],
                        env,
                    );

                    let public_assets_cache = InAppDirCacheWithLayer::new_and_load(
                        context,
                        &context.app_dir.join("public").join("assets"),
                    );
                    let fragments_cache = InAppDirCacheWithLayer::new_and_load(
                        context,
                        &context.app_dir.join("tmp").join("cache").join("assets"),
                    );

                    println!("    Rake task `rake assets:precompile` found, running");
                    assets_precompile.stream().unwrap();

                    if rake_detect.has_task("assets:clean") {
                        println!("    Rake task `rake assets:clean` found, running");

                        EnvCommand::new(
                            "bundle",
                            &["exec", "rake", "assets:clean", "--trace"],
                            env,
                        )
                        .stream()
                        .unwrap();

                        public_assets_cache.copy_app_path_to_cache();
                        fragments_cache.destructive_move_app_path_to_cache();

                        clean_stale_files_in_cache(
                            &fragments_cache,
                            Byte::from_bytes(byte_unit::n_mib_bytes!(100)),
                        );
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

fn clean_stale_files_in_cache(cache: &InAppDirCache, max_bytes: Byte) {
    let overage = cache.least_recently_used_files_above_limit(max_bytes);

    if overage.bytes > 0 {
        println!(
            "Cache for {} exceeded {} limit by {}, clearing {} files",
            cache.app_path.display(),
            max_bytes.get_adjusted_unit(ByteUnit::MiB),
            overage.to_byte().get_adjusted_unit(ByteUnit::MiB),
            overage.files.len()
        );
        overage.clean();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helper::touch_file;

    #[test]
    fn test_detect_rake_can_run() {
        assert_eq!(
            detect_rake_can_run(
                &HasRakefile(false),
                &HasGem(false),
                &AssetManifestList(vec![])
            ),
            CanRunRake::NoRakeGem
        );
        assert_eq!(
            detect_rake_can_run(
                &HasRakefile(false),
                &HasGem(true),
                &AssetManifestList(vec![])
            ),
            CanRunRake::MissingRakefile
        );
        assert_eq!(
            detect_rake_can_run(
                &HasRakefile(true),
                &HasGem(false),
                &AssetManifestList(vec![])
            ),
            CanRunRake::NoRakeGem
        );
        assert_eq!(
            detect_rake_can_run(
                &HasRakefile(true),
                &HasGem(true),
                &AssetManifestList(vec![])
            ),
            CanRunRake::Ok
        );
        assert_eq!(
            detect_rake_can_run(
                &HasRakefile(true),
                &HasGem(true),
                &AssetManifestList(vec![])
            ),
            CanRunRake::Ok
        );

        let path = PathBuf::new();
        assert_eq!(
            detect_rake_can_run(
                &HasRakefile(true),
                &HasGem(true),
                &AssetManifestList(vec![path.clone()])
            ),
            CanRunRake::AssetManifestSkip(vec![path])
        );
    }

    #[test]
    fn test_has_rakefile() {
        let tmpdir = tempfile::tempdir().unwrap();

        for name in &["rakefile", "Rakefile", "rakefile.rb;", "Rakefile.rb"] {
            let file = tmpdir.path().join(name);
            std::fs::write(&file, "").unwrap();
            assert_eq!(HasRakefile(true), dir_has_rakefile(tmpdir.path()));
            std::fs::remove_file(&file).unwrap();
        }

        assert_eq!(HasRakefile(false), dir_has_rakefile(tmpdir.path()));
    }

    #[test]
    fn test_has_asset_manifest() {
        let tmpdir = tempfile::tempdir().unwrap();
        let assets_dir = tmpdir.path().join("public").join("assets");
        assert_eq!(has_asset_manifest(tmpdir.path()), AssetManifestList(vec![]));

        touch_file(&assets_dir.join("manifest-lol.json"), |path| {
            assert_eq!(
                has_asset_manifest(tmpdir.path()),
                AssetManifestList(vec![path.clone()])
            );
        });

        touch_file(&assets_dir.join(".sprockets-manifest-lol.json"), |path| {
            assert_eq!(
                has_asset_manifest(tmpdir.path()),
                AssetManifestList(vec![path.clone()])
            );
        });
    }
}

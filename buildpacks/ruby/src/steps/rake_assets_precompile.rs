use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use byte_unit::{Byte, ByteUnit};
use commons::env_command::EnvCommand;
use commons::gem_list::GemList;
use commons::in_app_dir_cache::CacheError;
use commons::in_app_dir_cache::FilesWithSize;
use commons::in_app_dir_cache::State;
use commons::in_app_dir_cache::{DirCache, InAppDirCache};
use commons::rake_detect::RakeDetect;
use libcnb::build::BuildContext;
use libcnb::Env;
use libherokubuildpack::log as user;
use std::path::Path;

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

/// Invoke `rake assets:precompile`
pub(crate) fn rake_assets_precompile(
    gem_list: &GemList,
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
) -> Result<(), RubyBuildpackError> {
    user::log_header("Rake task detection");
    match detect_rake_can_run(
        &dir_has_rakefile(&context.app_dir),
        &gem_list_has_rake(gem_list),
        &has_asset_manifest(&context.app_dir),
    ) {
        CanRunRake::NoRakeGem => {
            user::log_info("Cannot run rake tasks, no rake gem in Gemfile");
            user::log_info("Add `gem 'rake'` to your Gemfile to enable");
        }
        CanRunRake::MissingRakefile => {
            user::log_info("Cannot run rake tasks, no Rakefile");
            user::log_info("Add a `Rakefile` to your project to enable");
        }
        CanRunRake::AssetManifestSkip(paths) => {
            let files = paths
                .iter()
                .map(|path| path.clone().to_string_lossy().to_string())
                .collect::<Vec<String>>()
                .join(", ");

            user::log_info("Skipping rake tasks. Manifest file(s) found");
            user::log_info(format!("To enable, delete files: {files}"));
        }
        CanRunRake::Ok => {
            user::log_info("Rakefile found");
            user::log_info("Rake gem in Gemfile found");

            user::log_info("Detecting rake tasks via `rake -P`");
            let rake_detect = RakeDetect::from_rake_command(env, true)
                .map_err(RubyBuildpackError::RakeDetectError)?;
            user::log_info("Done");

            run_rake_tasks(context, env, &rake_detect)?;
        }
    }

    Ok(())
}

enum AssetCases {
    None,
    PrecompileOnly,
    PrecompileAndClean,
}

fn asset_cases(rake: &RakeDetect) -> AssetCases {
    if !rake.has_task("assets:precompile") {
        AssetCases::None
    } else if rake.has_task("assets:clean") {
        AssetCases::PrecompileAndClean
    } else {
        AssetCases::PrecompileOnly
    }
}

fn run_rake_tasks(
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
    rake_detect: &RakeDetect,
) -> Result<(), RubyBuildpackError> {
    user::log_header("Rake asset installation");

    let cases = asset_cases(rake_detect);
    match cases {
        AssetCases::None => {
            user::log_info("Skipping 'rake assets:precompile', task not found");
            user::log_info("Help: Ensure `bundle exec rake -P` includes this task");
        }
        AssetCases::PrecompileOnly => {
            user::log_info("Running 'rake assets:precompile', task found");
            user::log_info("Skipping 'rake assets:clean', task not found");
            user::log_info("Help: Ensure `bundle exec rake -P` includes this task");

            let command = EnvCommand::new(
                "bundle",
                &["exec", "rake", "assets:precompile", "--trace"],
                env,
            );
            user::log_info("$ {command}");

            command
                .stream()
                .map_err(RubyBuildpackError::RakeAssetsPrecompileFailed)?;
        }
        AssetCases::PrecompileAndClean => {
            user::log_info("Running 'rake assets:precompile', task found");
            user::log_info("Running 'rake assets:clean', task found");

            let cache = AssetCache::new_and_load(context)
                .map_err(RubyBuildpackError::InAppDirCacheError)?;

            let command = EnvCommand::new(
                "bundle",
                &[
                    "exec",
                    "rake",
                    "assets:precompile",
                    "assets:clean",
                    "--trace",
                ],
                env,
            );

            user::log_info("$ {command}");

            command
                .stream()
                .map_err(RubyBuildpackError::RakeAssetsPrecompileFailed)?;

            cache
                .store()
                .map_err(RubyBuildpackError::InAppDirCacheError)?;

            user::log_info("Done");
        }
    }

    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AssetCache {
    caches: Vec<(DirCache, CacheConfig)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum KeepAppPath {
    Runtime,
    BuildOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CacheConfig {
    path: PathBuf,
    limit: Byte,
    on_store: KeepAppPath,
}

/// Used for loading/unloading asset cache and communicating what's happening to the user
impl AssetCache {
    fn new_and_load(context: &BuildContext<RubyBuildpack>) -> Result<Self, CacheError> {
        let caches = [
            CacheConfig {
                path: context.app_dir.join("public").join("assets"),
                limit: Byte::from_bytes(byte_unit::n_mib_bytes!(100)),
                on_store: KeepAppPath::Runtime,
            },
            CacheConfig {
                path: context.app_dir.join("tmp").join("cache").join("assets"),
                limit: Byte::from_bytes(byte_unit::n_mib_bytes!(100)),
                on_store: KeepAppPath::BuildOnly,
            },
        ]
        .into_iter()
        .map(|config| {
            InAppDirCache::new_and_load(context, &config.path).map(|cache| {
                Self::log_load(&cache);

                (cache, config)
            })
        })
        .collect::<Result<Vec<(DirCache, CacheConfig)>, _>>()?;

        Ok(Self { caches })
    }

    fn log_load(cache: &DirCache) {
        let path = cache.app_path.display();

        match cache.state {
            State::NewEmpty => user::log_info(format!("Creating cache for {path}")),
            State::ExistsEmpty => user::log_info(format!("Loading (empty) cache for {path}")),
            State::ExistsWithContents => user::log_info(format!("Loading cache for {path}")),
        }
    }

    fn log_store(cache: &DirCache) {
        let path = cache.app_path.display();
        if cache.is_app_dir_empty() {
            user::log_info(format!("Storing cache for (empty) {path}"));
        } else {
            user::log_info(format!("Storing cache for {path}"));
        }
    }

    fn log_sweep(cache: &DirCache, config: &CacheConfig, removed: &FilesWithSize) {
        let path = cache.app_path.display();
        let limit = config.limit.get_adjusted_unit(ByteUnit::MiB);
        let removed_len = removed.files.len();
        let removed_size = removed.get_adjusted_unit(ByteUnit::MiB);

        user::log_info(format!("Cache exceeded {limit} limit by {removed_size}"));
        user::log_info(format!(
            "Removing {removed_len} files from the cache for {path}",
        ));
    }

    fn store(&self) -> Result<(), CacheError> {
        for (cache, config) in &self.caches {
            Self::log_store(cache);

            match config.on_store {
                KeepAppPath::Runtime => cache.copy_app_path_to_cache()?,
                KeepAppPath::BuildOnly => cache.destructive_move_app_path_to_cache()?,
            };

            if let Some(removed) = cache.lru_clean(config.limit)? {
                Self::log_sweep(cache, config, &removed);
            }
        }

        Ok(())
    }
}

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

/// Checks directory for rakefile variants
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
        .map(|glob_pattern| {
            app_dir
                .join("public")
                .join("assets")
                .join(glob_pattern)
                .into_os_string()
                .into_string()
                .expect("Internal error: Non-unicode bytes in hardcoded internal str")
        })
        .flat_map(|string| glob(&string).expect("Internal error: Bad manifest glob pattern"))
        .filter_map(Result::ok) // Err contains io errors if directory is unreachable
        .collect::<Vec<PathBuf>>();

    AssetManifestList(manifests)
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

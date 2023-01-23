use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use commons::gem_list::GemList;
use commons::rake_detect::RakeDetect;
use glob::glob;
use libcnb::build::BuildContext;
use libcnb::Env;
use libherokubuildpack::log as user;
use std::path::Path;
use std::path::PathBuf;

pub(crate) fn detect_rake_tasks(
    gem_list: &GemList,
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
) -> Result<Option<RakeDetect>, RubyBuildpackError> {
    match detect_rake_can_run(
        find_rakefile(&context.app_dir),
        &rake_gem(gem_list),
        asset_manifest(&context.app_dir),
    ) {
        RakeStatus::MissingRakeGem => {
            user::log_info("Cannot run rake tasks, no rake gem in Gemfile");
            user::log_info("Add `gem 'rake'` to your Gemfile to enable");

            Ok(None)
        }
        RakeStatus::MissingRakefile => {
            user::log_info("Cannot run rake tasks, no Rakefile");
            user::log_info("Add a `Rakefile` to your project to enable");

            Ok(None)
        }
        RakeStatus::SkipManifestFound(paths) => {
            user::log_info("Skipping rake tasks. Manifest file(s) found");
            user::log_info(format!(
                "To enable, delete files: {}",
                paths
                    .iter()
                    .map(|path| path.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));

            Ok(None)
        }
        RakeStatus::Ready(path) => {
            let path = path.display();
            user::log_info(format!("Rakefile found {path}"));
            user::log_info("Rake gem found");

            user::log_info("Detecting rake tasks via `rake -P`");
            let rake_detect = RakeDetect::from_rake_command(env, true)
                .map_err(RubyBuildpackError::RakeDetectError)?;

            Ok(Some(rake_detect))
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum RakeStatus {
    Ready(PathBuf),
    MissingRakeGem,
    MissingRakefile,
    SkipManifestFound(Vec<PathBuf>),
}

// Convert nested logic into a flat enum of possible states
// that represent whether or not `rake assets:precompile` can
// be run.
fn detect_rake_can_run(
    rakefile: Rakefile,
    rake_gem: &RakeGem,
    manifest: AssetManifest,
) -> RakeStatus {
    match (rake_gem, rakefile, manifest) {
        (RakeGem::Found, Rakefile::Found(p), AssetManifest::Missing) => RakeStatus::Ready(p),
        (RakeGem::Missing, _, _) => RakeStatus::MissingRakeGem,
        (_, Rakefile::Missing, _) => RakeStatus::MissingRakefile,
        (_, _, AssetManifest::Found(m)) => RakeStatus::SkipManifestFound(m),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Rakefile {
    Found(PathBuf),
    Missing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RakeGem {
    Found,
    Missing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AssetManifest {
    Found(Vec<PathBuf>),
    Missing,
}

/// Checks directory for rakefile variants
fn find_rakefile(path: &Path) -> Rakefile {
    ["rakefile", "Rakefile", "rakefile.rb;", "Rakefile.rb"]
        .iter()
        .map(|name| path.join(name))
        .find_map(|path| path.exists().then_some(path))
        .map_or_else(|| Rakefile::Missing, Rakefile::Found)
}

// Checks if GemList contains a reference to the rake gem
fn rake_gem(gem_list: &GemList) -> RakeGem {
    if gem_list.has("rake") {
        RakeGem::Found
    } else {
        RakeGem::Missing
    }
}

// Checks in public/assets if an existing manifest file exists
fn asset_manifest(app_dir: &Path) -> AssetManifest {
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

    if manifests.is_empty() {
        AssetManifest::Missing
    } else {
        AssetManifest::Found(manifests)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helper::touch_file;

    #[test]
    fn test_detect_rake_can_run() {
        assert_eq!(
            detect_rake_can_run(Rakefile::Missing, &RakeGem::Found, AssetManifest::Missing),
            RakeStatus::MissingRakeGem
        );
        assert_eq!(
            detect_rake_can_run(Rakefile::Missing, &RakeGem::Missing, AssetManifest::Missing),
            RakeStatus::MissingRakefile
        );
        assert_eq!(
            detect_rake_can_run(
                Rakefile::Found(PathBuf::new()),
                &RakeGem::Found,
                AssetManifest::Missing
            ),
            RakeStatus::MissingRakeGem
        );
        assert_eq!(
            detect_rake_can_run(
                Rakefile::Found(PathBuf::new()),
                &RakeGem::Missing,
                AssetManifest::Missing
            ),
            RakeStatus::Ready(PathBuf::new())
        );
        assert_eq!(
            detect_rake_can_run(
                Rakefile::Found(PathBuf::new()),
                &RakeGem::Missing,
                AssetManifest::Missing
            ),
            RakeStatus::Ready(PathBuf::new())
        );

        let path = PathBuf::new();
        assert_eq!(
            detect_rake_can_run(
                Rakefile::Found(PathBuf::new()),
                &RakeGem::Missing,
                AssetManifest::Found(vec![path.clone()])
            ),
            RakeStatus::SkipManifestFound(vec![path])
        );
    }

    #[test]
    fn test_has_rakefile() {
        let tmpdir = tempfile::tempdir().unwrap();

        for name in &["rakefile", "Rakefile", "rakefile.rb;", "Rakefile.rb"] {
            let file = tmpdir.path().join(name);
            std::fs::write(&file, "").unwrap();
            assert_eq!(
                Rakefile::Found(tmpdir.path().to_path_buf()),
                find_rakefile(tmpdir.path())
            );
            std::fs::remove_file(&file).unwrap();
        }

        assert_eq!(Rakefile::Missing, find_rakefile(tmpdir.path()));
    }

    #[test]
    fn test_has_asset_manifest() {
        let tmpdir = tempfile::tempdir().unwrap();
        let assets_dir = tmpdir.path().join("public").join("assets");
        assert_eq!(asset_manifest(tmpdir.path()), AssetManifest::Missing);

        touch_file(&assets_dir.join("manifest-lol.json"), |path| {
            assert_eq!(
                asset_manifest(tmpdir.path()),
                AssetManifest::Found(vec![path.clone()])
            );
        });

        touch_file(&assets_dir.join(".sprockets-manifest-lol.json"), |path| {
            assert_eq!(
                asset_manifest(tmpdir.path()),
                AssetManifest::Found(vec![path.clone()])
            );
        });
    }
}

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
    match detect_rake_status(
        &context.app_dir,
        gem_list,
        [".sprockets-manifest-*.json", "manifest-*.json"],
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
fn detect_rake_status(
    app_path: &Path,
    gem_list: &GemList,
    globs: impl IntoIterator<Item = impl AsRef<str>>,
) -> RakeStatus {
    let rakefile = find_rakefile(app_path);
    let rake_gem = rake_gem(gem_list);
    let manifest = asset_manifest_from_glob(app_path, globs);
    rake_status(&rake_gem, rakefile, manifest)
}

// Convert nested logic into a flat enum of possible states
// that represent whether or not `rake assets:precompile` can
// be run.
fn rake_status(rake_gem: &RakeGem, rakefile: Rakefile, manifest: AssetManifest) -> RakeStatus {
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

fn asset_manifest_from_glob(
    app_dir: &Path,
    globs: impl IntoIterator<Item = impl AsRef<str>>,
) -> AssetManifest {
    let manifests = globs
        .into_iter()
        .map(|glob_pattern| {
            app_dir
                .join("public")
                .join("assets")
                .join(glob_pattern.as_ref())
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

    // Checks in public/assets if an existing manifest file exists
    fn asset_manifest(app_dir: &Path) -> AssetManifest {
        let globs = [".sprockets-manifest-*.json", "manifest-*.json"];

        asset_manifest_from_glob(app_dir, globs)
    }

    #[test]
    fn test_detect_rake_can_run() {
        assert_eq!(
            rake_status(&RakeGem::Found, Rakefile::Missing, AssetManifest::Missing),
            RakeStatus::MissingRakefile
        );
        assert_eq!(
            rake_status(&RakeGem::Missing, Rakefile::Missing, AssetManifest::Missing),
            RakeStatus::MissingRakeGem
        );
        assert_eq!(
            rake_status(
                &RakeGem::Found,
                Rakefile::Found(PathBuf::new()),
                AssetManifest::Missing
            ),
            RakeStatus::Ready(PathBuf::new())
        );
        assert_eq!(
            rake_status(
                &RakeGem::Missing,
                Rakefile::Found(PathBuf::new()),
                AssetManifest::Missing
            ),
            RakeStatus::MissingRakeGem
        );
        assert_eq!(
            rake_status(
                &RakeGem::Missing,
                Rakefile::Found(PathBuf::new()),
                AssetManifest::Missing
            ),
            RakeStatus::MissingRakeGem
        );

        let path = PathBuf::new();
        assert_eq!(
            rake_status(
                &RakeGem::Found,
                Rakefile::Found(PathBuf::new()),
                AssetManifest::Found(vec![path.clone()])
            ),
            RakeStatus::SkipManifestFound(vec![path])
        );
    }

    #[test]
    fn test_has_rakefile() {
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path();

        for name in &["rakefile", "Rakefile", "rakefile.rb;", "Rakefile.rb"] {
            let file = dir.join(name);
            std::fs::write(&file, "").unwrap();
            let found = match find_rakefile(dir) {
                Rakefile::Found(_) => true,
                _ => false,
            };
            assert!(found);
            std::fs::remove_file(&file).unwrap();
        }

        assert_eq!(Rakefile::Missing, find_rakefile(dir));
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

    #[test]
    fn asset_manifest_empty_glob() {
        let tmpdir = tempfile::tempdir().unwrap();
        let empty: [String; 0] = [];
        assert_eq!(
            asset_manifest_from_glob(tmpdir.path(), empty),
            AssetManifest::Missing
        );
    }
}

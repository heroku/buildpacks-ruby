use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use commons::fun_run::{self, CmdMapExt};
use commons::gemfile_lock::ResolvedBundlerVersion;
use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::Env;
use libherokubuildpack::command::CommandExt;
use libherokubuildpack::log as user;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct BundleDownloadLayerMetadata {
    version: ResolvedBundlerVersion,
}

/// # Install the bundler gem
///
/// ## Layer dir: Install bundler to disk
///
/// Installs a copy of `bundler` to the `<layer-dir>` with a bundler executable in
/// `<layer-dir>/bin`. Must run before [`crate.steps.bundle_install`].
pub(crate) struct BundleDownloadLayer {
    pub env: Env,
    pub version: ResolvedBundlerVersion,
}

impl Layer for BundleDownloadLayer {
    type Buildpack = RubyBuildpack;
    type Metadata = BundleDownloadLayerMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: true,
            launch: true,
            cache: true,
        }
    }
    fn create(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        user::log_info(format!("Installing bundler {}", self.version));

        let bin_dir = layer_path.join("bin");
        let gem_path = layer_path;

        Command::new("gem")
            .args([
                "install",
                "bundler",
                "--force",
                "--no-document", // Don't install ri or rdoc which takes extra time
                "--env-shebang", // Start the `bundle` executable with `#! /usr/bin/env ruby`
                "--version",     // Specify exact version to install
                &self.version.to_string(),
                "--install-dir", // Directory where bundler's contents will live
                &layer_path.to_string_lossy(),
                "--bindir", // Directory where `bundle` executable lives
                &bin_dir.to_string_lossy(),
            ])
            .env_clear()
            .envs(&self.env)
            .cmd_map(|cmd| {
                let name = fun_run::display(cmd);

                user::log_info(format!("Running  $ {name}"));

                cmd.output_and_write_streams(std::io::stdout(), std::io::stderr())
                    .map_err(|error| {
                        fun_run::annotate_which_problem(error, cmd, self.env.get("PATH"))
                    })
                    .map_err(|error| fun_run::on_system_error(name.clone(), error))
                    .and_then(|output| fun_run::nonzero_streamed(name.clone(), output))
            })
            .map_err(RubyBuildpackError::GemInstallBundlerCommandError)?;

        LayerResultBuilder::new(BundleDownloadLayerMetadata {
            version: self.version.clone(),
        })
        .env(
            LayerEnv::new()
                .chainable_insert(Scope::All, ModificationBehavior::Delimiter, "PATH", ":")
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Prepend,
                    "PATH", // Ensure this path comes before default bundler that ships with ruby, don't rely on the lifecycle
                    bin_dir,
                )
                .chainable_insert(Scope::All, ModificationBehavior::Delimiter, "GEM_PATH", ":")
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Prepend,
                    "GEM_PATH", // Bundler is a gem too, allow it to be required
                    gem_path,
                ),
        )
        .build()
    }

    fn existing_layer_strategy(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        let old = &layer_data.content_metadata.metadata;
        let now = BundleDownloadLayerMetadata {
            version: self.version.clone(),
        };
        match cache_state(old.clone(), now) {
            State::NothingChanged(version) => {
                user::log_info(format!("Using bundler {version} from cache"));

                Ok(ExistingLayerStrategy::Keep)
            }
            State::BundlerVersionChanged(old, now) => {
                user::log_info(format!("Bundler version changed from {old} to {now}"));
                user::log_info("Clearing bundler from cache");

                Ok(ExistingLayerStrategy::Recreate)
            }
        }
    }
}

// [derive(Debug)]
enum State {
    NothingChanged(ResolvedBundlerVersion),
    BundlerVersionChanged(ResolvedBundlerVersion, ResolvedBundlerVersion),
}

fn cache_state(old: BundleDownloadLayerMetadata, now: BundleDownloadLayerMetadata) -> State {
    let BundleDownloadLayerMetadata { version } = now; // Ensure all properties are checked

    if old.version == version {
        State::NothingChanged(version)
    } else {
        State::BundlerVersionChanged(old.version, version)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// If this test fails due to a change you'll need to implement
    /// `migrate_incompatible_metadata` for the Layer trait
    #[test]
    fn metadata_guard() {
        let metadata = BundleDownloadLayerMetadata {
            version: ResolvedBundlerVersion(String::from("2.3.6")),
        };

        let actual = toml::to_string(&metadata).unwrap();
        let expected = r#"
version = "2.3.6"
"#
        .trim();
        assert_eq!(expected, actual.trim());
    }
}

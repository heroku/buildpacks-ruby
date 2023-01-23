use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use commons::env_command::EnvCommand;
use commons::gemfile_lock::ResolvedBundlerVersion;
use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::Env;
use libherokubuildpack::log as user;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct BundleDownloadLayerMetadata {
    version: String,
}

/// # Install the bundler gem
///
/// ## Layer dir: Install bundler to disk
///
/// Installs a copy of `bundler` to the `<layer-dir>` with a bundler executable in
/// `<layer-dir>/bin`. Must run before [`crate.steps.bundle_install`].
pub(crate) struct BundleDownloadLayer {
    pub version: ResolvedBundlerVersion,
    pub env: Env,
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

        EnvCommand::new(
            "gem",
            &[
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
                &layer_path.join("bin").to_string_lossy(),
            ],
            &self.env,
        )
        .output()
        .map_err(RubyBuildpackError::GemInstallBundlerCommandError)?;

        LayerResultBuilder::new(BundleDownloadLayerMetadata {
            version: self.version.to_string(),
        })
        .env(
            LayerEnv::new()
                .chainable_insert(Scope::All, ModificationBehavior::Delimiter, "PATH", ":")
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Prepend,
                    "PATH", // Ensure this path comes before default bundler that ships with ruby, don't rely on the lifecycle
                    layer_path.join("bin"),
                )
                .chainable_insert(Scope::All, ModificationBehavior::Delimiter, "GEM_PATH", ":")
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Prepend,
                    "GEM_PATH", // Bundler is a gem too, allow it to be required
                    layer_path,
                ),
        )
        .build()
    }

    fn existing_layer_strategy(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        layer: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        let contents = CacheContents {
            current_version: &self.version.to_string(),
            old_version: &layer.content_metadata.metadata.version,
        };
        match contents.state() {
            State::NothingChanged => {
                user::log_info(format!("Using bundler {} from cache", self.version));

                Ok(ExistingLayerStrategy::Keep)
            }
            State::BundlerVersionChanged => {
                user::log_info(format!(
                    "Bundler version changed from {} to {}",
                    contents.old_version, contents.current_version
                ));
                user::log_info("Clearing bundler from cache");

                Ok(ExistingLayerStrategy::Recreate)
            }
        }
    }
}

enum State {
    NothingChanged,
    BundlerVersionChanged,
}

struct CacheContents<'a, 'b> {
    current_version: &'a str,
    old_version: &'b str,
}

impl CacheContents<'_, '_> {
    fn state(&self) -> State {
        if self.current_version == self.old_version {
            State::NothingChanged
        } else {
            State::BundlerVersionChanged
        }
    }
}

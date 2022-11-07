use crate::lib::gemfile_lock::ResolvedBundlerVersion;
use crate::RubyBuildpackError;
use libcnb::data::layer_content_metadata::LayerTypes;
use serde::{Deserialize, Serialize};

use commons::env_command::EnvCommand;
use std::path::Path;

use crate::RubyBuildpack;
use libcnb::build::BuildContext;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::Env;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct BundleInstallDownloadBundlerLayerMetadata {
    version: String,
}

/*

# Install the bundler gem

## Layer dir: Install bundler to disk

Installs a copy of `bundler` to the `<layer-dir>` with a bundler executable in
`<layer-dir>/bin`. Must run before `execute_bundle_install_layer`.

## Set environment variables

- `PATH=<layer-dir>/bin:$PATH` - Ruby ships with a bundler executable and we
want to make sure that the version installed via this layer always comes first.
To accomplish that we manually place the directory on the PATH (instead of relying on
the CNB lifecycle to place `<layer-dir>/bin` on the path).
- `GEM_PATH=<layer-dir>:$GEM_PATH` - Beyond installing bundler we want to make it
requireable to the target application. This is accomplished by prepending the layer path
to `GEM_PATH` which tells rubygems where it can search for gems.

*/
pub struct BundleInstallDownloadBundlerLayer {
    pub version: ResolvedBundlerVersion,
    pub env: Env,
}

impl Layer for BundleInstallDownloadBundlerLayer {
    type Buildpack = RubyBuildpack;
    type Metadata = BundleInstallDownloadBundlerLayerMetadata;

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
        println!("---> Installing bundler {}", self.version);

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
        .call()
        .map_err(RubyBuildpackError::GemInstallBundlerCommandError)?;

        LayerResultBuilder::new(BundleInstallDownloadBundlerLayerMetadata {
            version: self.version.to_string(),
        })
        .env(
            LayerEnv::new()
                .chainable_insert(Scope::All, ModificationBehavior::Delimiter, "PATH", ":")
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Prepend,
                    "PATH",
                    &layer_path.join("bin"),
                )
                .chainable_insert(Scope::All, ModificationBehavior::Delimiter, "GEM_PATH", ":")
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Prepend,
                    "GEM_PATH",
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
        let old_version = &layer.content_metadata.metadata.version;
        if &self.version.to_string() == old_version {
            println!("---> Bundler {} already installed", self.version);
            Ok(ExistingLayerStrategy::Keep)
        } else {
            println!(
                "---> Detected bundler version change, discarding old bundler version: {} ",
                old_version
            );
            Ok(ExistingLayerStrategy::Recreate)
        }
    }
}

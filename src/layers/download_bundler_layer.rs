use crate::{util, RubyBuildpackError};
use libcnb::data::layer_content_metadata::LayerTypes;
use serde::{Deserialize, Serialize};

use std::path::Path;
use std::process::Command;

use crate::gemfile_lock::BundlerVersion;
use crate::RubyBuildpack;
use libcnb::build::BuildContext;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::Env;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DownloadBundlerLayerMetadata {
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
pub struct DownloadBundlerLayer {
    pub version: BundlerVersion,
    pub env: Env,
}

impl DownloadBundlerLayer {
    fn version_string(&self) -> String {
        match &self.version {
            BundlerVersion::Explicit(v) => v.clone(),
            BundlerVersion::Default => String::from("2.3.7"),
        }
    }
}

impl Layer for DownloadBundlerLayer {
    type Buildpack = RubyBuildpack;
    type Metadata = DownloadBundlerLayerMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: true,
            launch: true,
            cache: true,
        }
    }

    fn update(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        let metadata = &layer_data.content_metadata.metadata;
        let old_value = metadata.version.clone();

        println!(
            "---> New bundler version detected {}, uninstalling the old version {}",
            self.version_string(),
            old_value
        );

        util::run_simple_command(
            Command::new("gem")
                .args(&[
                    "uninstall",
                    "bundler",
                    "--force",
                    "--version",
                    &old_value,
                    "--install-dir",
                    &layer_data.path.to_string_lossy(),
                ])
                .envs(&self.env),
            RubyBuildpackError::GemInstallBundlerCommandError,
            RubyBuildpackError::GemInstallBundlerUnexpectedExitStatus,
        )?;

        self.create(context, &layer_data.path)
    }

    fn create(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        println!("---> Installing bundler {}", self.version_string());

        util::run_simple_command(
            Command::new("gem")
                .args(&[
                    "install",
                    "bundler",
                    "--force",
                    "--no-document",
                    "--env-shebang",
                    "--version",
                    &self.version_string(),
                    "--install-dir",
                    &layer_path.to_string_lossy(),
                    "--bindir",
                    &layer_path.join("bin").to_string_lossy(),
                ])
                .envs(&self.env),
            RubyBuildpackError::GemInstallBundlerCommandError,
            RubyBuildpackError::GemInstallBundlerUnexpectedExitStatus,
        )?;

        LayerResultBuilder::new(DownloadBundlerLayerMetadata {
            version: self.version_string(),
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
        if self.version_string() == layer.content_metadata.metadata.version {
            println!("---> Bundler {} already installed", self.version_string());
            Ok(ExistingLayerStrategy::Keep)
        } else {
            Ok(ExistingLayerStrategy::Update)
        }
    }
}

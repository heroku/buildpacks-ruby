use crate::{util, RubyBuildpackError};
use libcnb::data::layer_content_metadata::LayerTypes;
use serde::Deserialize;
use serde::Serialize;

use std::path::Path;
use std::process::Command;

use crate::RubyBuildpack;
use libcnb::build::BuildContext;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::Env;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct BundlerLayerMetadata {
    gemfile_lock_checksum: String,
}

pub struct BundlerLayer {
    pub ruby_env: Env,
}

impl Layer for BundlerLayer {
    type Buildpack = RubyBuildpack;
    type Metadata = BundlerLayerMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: true,
            launch: true,
            cache: true,
        }
    }

    fn create(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        let version = "2.3.5";
        println!("---> Installing bundler {}", version);

        util::run_simple_command(
            Command::new("gem")
                .args(&["install", "bundler", "-v", version, "--force"])
                .envs(&self.ruby_env),
            RubyBuildpackError::GemInstallBundlerCommandError,
            RubyBuildpackError::GemInstallBundlerUnexpectedExitStatus,
        )?;

        println!("---> Installing gems");

        let mut command = Command::new("bundle");
        command.args(&[
            "install",
            "--path",
            layer_path.to_str().unwrap(),
            "--binstubs",
            layer_path.join("bin").to_str().unwrap(),
        ]);

        println!(
            "Running: {} {}",
            command.get_program().to_string_lossy(),
            command
                .get_args()
                .map(|arg| arg.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ")
        );

        util::run_simple_command(
            command.envs(&self.ruby_env),
            RubyBuildpackError::BundleInstallCommandError,
            RubyBuildpackError::BundleInstallUnexpectedExitStatus,
        )?;

        // TODO: Also record env vars
        LayerResultBuilder::new(BundlerLayerMetadata {
            gemfile_lock_checksum: util::sha256_checksum(context.app_dir.join("Gemfile.lock"))
                .map_err(RubyBuildpackError::CouldNotGenerateChecksum)?,
        })
        .build()
    }

    fn existing_layer_strategy(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        util::sha256_checksum(context.app_dir.join("Gemfile.lock"))
            .map_err(RubyBuildpackError::CouldNotGenerateChecksum)
            .map(|checksum| {
                if checksum != layer.content_metadata.metadata.gemfile_lock_checksum {
                    ExistingLayerStrategy::Update
                } else {
                    ExistingLayerStrategy::Keep
                }
            })
    }

    fn update(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer: &LayerData<Self::Metadata>,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        println!("---> Reusing gems");

        util::run_simple_command(
            Command::new("bundle")
                .args(&["config", "--local", "path", layer.path.to_str().unwrap()])
                .envs(&self.ruby_env),
            RubyBuildpackError::BundleConfigCommandError,
            RubyBuildpackError::BundleConfigUnexpectedExitStatus,
        )?;

        util::run_simple_command(
            Command::new("bundle")
                .args(&[
                    "config",
                    "--local",
                    "bin",
                    layer.path.join("bin").as_path().to_str().unwrap(),
                ])
                .envs(&self.ruby_env),
            RubyBuildpackError::BundleConfigCommandError,
            RubyBuildpackError::BundleConfigUnexpectedExitStatus,
        )?;

        LayerResultBuilder::new(BundlerLayerMetadata {
            gemfile_lock_checksum: util::sha256_checksum(context.app_dir.join("Gemfile.lock"))
                .map_err(RubyBuildpackError::CouldNotGenerateChecksum)?,
        })
        .build()
    }
}

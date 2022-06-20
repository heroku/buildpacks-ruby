#![warn(unused_crate_dependencies)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use crate::gemfile_lock::{GemfileLock, GemfileLockError, RubyVersion};
use crate::layers::{
    BundleInstallConfigureEnvLayer, BundleInstallCreatePathLayer,
    BundleInstallDownloadBundlerLayer, BundleInstallExecuteLayer, RubyVersionInstallLayer,
};
use libcnb::build::{BuildContext, BuildResult, BuildResultBuilder};
use libcnb::data::launch::{Launch, ProcessBuilder};
use libcnb::data::{layer_name, process_type};
use libcnb::detect::{DetectContext, DetectResult, DetectResultBuilder};
use libcnb::generic::{GenericMetadata, GenericPlatform};
use libcnb::layer_env::Scope;
use libcnb::Platform;
use libcnb::{buildpack_main, Buildpack};

#[cfg(test)]
use libcnb_test as _;

use core::str::FromStr;

use crate::util::{DownloadError, UntarError, UrlError};
use std::process::ExitStatus;

mod gemfile_lock;
mod layers;
mod util;

pub struct RubyBuildpack;
impl Buildpack for RubyBuildpack {
    type Platform = GenericPlatform;
    type Metadata = GenericMetadata;
    type Error = RubyBuildpackError;

    fn detect(&self, context: DetectContext<Self>) -> libcnb::Result<DetectResult, Self::Error> {
        if context.app_dir.join("Gemfile.lock").exists() {
            DetectResultBuilder::pass().build()
        } else {
            DetectResultBuilder::fail().build()
        }
    }

    fn build(&self, context: BuildContext<Self>) -> libcnb::Result<BuildResult, Self::Error> {
        println!("---> Ruby Buildpack");

        let mut env = context.platform.env().clone();

        // Gather static information about project
        let gemfile_lock = std::fs::read_to_string(context.app_dir.join("Gemfile.lock")).unwrap();
        let bundle_info = GemfileLock::from_str(&gemfile_lock)
            .map_err(RubyBuildpackError::GemfileLockParsingError)?;

        // ## Install executable ruby version
        let ruby_layer = context //
            .handle_layer(
                layer_name!("ruby"),
                RubyVersionInstallLayer {
                    version: bundle_info.ruby_version,
                },
            )?;

        env = ruby_layer.env.apply(Scope::Build, &env);

        // ## Setup bundler
        let create_bundle_path_layer = context.handle_layer(
            layer_name!("create_bundle_path"),
            BundleInstallCreatePathLayer {
                ruby_version: ruby_layer.content_metadata.metadata.version,
            },
        )?;
        env = create_bundle_path_layer.env.apply(Scope::Build, &env);

        let create_bundle_path_layer = context.handle_layer(
            layer_name!("create_bundle_path"),
            BundleInstallConfigureEnvLayer,
        )?;
        env = create_bundle_path_layer.env.apply(Scope::Build, &env);

        // ## Download bundler
        let download_bundler_layer = context.handle_layer(
            layer_name!("download_bundler"),
            BundleInstallDownloadBundlerLayer {
                version: bundle_info.bundler_version,
                env: env.clone(),
            },
        )?;
        env = download_bundler_layer.env.apply(Scope::Build, &env);

        // ## bundle install
        let _execute_bundle_install_layer = context.handle_layer(
            layer_name!("execute_bundle_install"),
            BundleInstallExecuteLayer { env },
        )?;
        // _env = execute_bundle_install_layer.env.apply(Scope::Build, &env);

        BuildResultBuilder::new()
            .launch(
                Launch::new().process(
                    ProcessBuilder::new(process_type!("web"), "bundle")
                        .args(["exec", "rackup", "--port", "$PORT", "--host", "0.0.0.0"])
                        .default(true)
                        .build(),
                ),
            )
            .build()
    }
}

#[derive(thiserror::Error, Debug)]
pub enum RubyBuildpackError {
    #[error("Cannot download: {0}")]
    RubyDownloadError(DownloadError),
    #[error("Cannot untar: {0}")]
    RubyUntarError(UntarError),
    #[error("Cannot create temporary file: {0}")]
    CouldNotCreateTemporaryFile(std::io::Error),
    #[error("Cannot generate checksum: {0}")]
    CouldNotGenerateChecksum(std::io::Error),
    #[error("Cannot install bundler: {0}")]
    GemInstallBundlerCommandError(std::io::Error),
    #[error("Bundler gem install exit: {0}")]
    GemInstallBundlerUnexpectedExitStatus(ExitStatus),
    #[error("Bundle install errored: {0}")]
    BundleInstallCommandError(std::io::Error),
    #[error("Bundle install exit: {0}")]
    BundleInstallUnexpectedExitStatus(ExitStatus),
    #[error("Bundle config error: {0}")]
    BundleConfigCommandError(std::io::Error),
    #[error("Bundle config exit: {0}")]
    BundleConfigUnexpectedExitStatus(ExitStatus),

    #[error("Url error: {0}")]
    UrlParseError(UrlError),

    #[error("Error evaluating Gemfile.lock: {0}")]
    GemfileLockParsingError(GemfileLockError),
}
impl From<RubyBuildpackError> for libcnb::Error<RubyBuildpackError> {
    fn from(error: RubyBuildpackError) -> Self {
        Self::BuildpackError(error)
    }
}

buildpack_main!(RubyBuildpack);

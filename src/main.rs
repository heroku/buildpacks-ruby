use crate::layers::{BundlerLayer, RubyLayer};
use libcnb::build::{BuildContext, BuildResult, BuildResultBuilder};
use libcnb::data::launch::{Launch, Process};
use libcnb::data::{layer_name, process_type};
use libcnb::detect::{DetectContext, DetectResult, DetectResultBuilder};
use libcnb::generic::GenericPlatform;
use libcnb::layer_env::Scope;
use libcnb::{buildpack_main, Buildpack, Env};

use crate::util::{DownloadError, UntarError, UrlError};
use serde::Deserialize;
use std::process::ExitStatus;

mod layers;
mod util;

pub struct RubyBuildpack;

impl Buildpack for RubyBuildpack {
    type Platform = GenericPlatform;
    type Metadata = RubyBuildpackMetadata;
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

        let ruby_layer = context //
            .handle_layer(
                //
                layer_name!("ruby"), //
                RubyLayer,           //
            )?;

        context.handle_layer(
            layer_name!("bundler"),
            BundlerLayer {
                ruby_env: ruby_layer.env.apply(Scope::Build, &Env::new()),
            },
        )?;

        BuildResultBuilder::new()
            .launch(
                Launch::new()
                    .process(Process::new(
                        process_type!("web"),
                        "bundle",
                        vec!["exec", "ruby", "app.rb"],
                        false,
                        true,
                    ))
                    .process(Process::new(
                        process_type!("worker"),
                        "bundle",
                        vec!["exec", "ruby", "worker.rb"],
                        false,
                        false,
                    )),
            )
            .build()
    }
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct RubyBuildpackMetadata {
    pub ruby_url: String,
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
}

buildpack_main!(RubyBuildpack);

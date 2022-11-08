#![warn(unused_crate_dependencies)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use crate::layers::{InAppDirCacheLayer, RubyVersionInstallLayer};
use commons::gem_list::GemList;
use commons::gemfile_lock::{GemfileLock, GemfileLockError};

use commons::gem_list::GemListError;
use commons::rake_detect::RakeDetectError;
use regex::Regex;

use crate::steps::bundle_install::BundleInstall;
use crate::steps::default_env::DefaultEnv;
use crate::steps::RakeApplicationTasksExecute;

use libcnb::build::{BuildContext, BuildResult, BuildResultBuilder};
use libcnb::data::launch::{LaunchBuilder, ProcessBuilder};
use libcnb::data::{layer_name, process_type};
use libcnb::detect::{DetectContext, DetectResult, DetectResultBuilder};
use libcnb::generic::{GenericMetadata, GenericPlatform};
use libcnb::layer_env::Scope;
use libcnb::Platform;
use libcnb::{buildpack_main, Buildpack};

use commons::env_command::EnvCommandError;

#[cfg(test)]
use libcnb_test as _;

use core::str::FromStr;

use crate::util::{DownloadError, UntarError, UrlError};
use std::process::ExitStatus;

mod layers;
mod lib;
mod steps;

#[cfg(test)]
mod test_helper;
mod util;

pub struct RubyBuildpack;
use libcnb::data::build_plan::BuildPlanBuilder;

fn app_needs_java(context: &DetectContext<RubyBuildpack>) -> bool {
    let gemfile_lock = std::fs::read_to_string(context.app_dir.join("Gemfile.lock")).unwrap();
    needs_java(&gemfile_lock)
}

fn needs_java(gemfile_lock: &str) -> bool {
    let java_regex = Regex::new(r"\(jruby ").unwrap();
    java_regex.is_match(gemfile_lock)
}

impl Buildpack for RubyBuildpack {
    type Platform = GenericPlatform;
    type Metadata = GenericMetadata;
    type Error = RubyBuildpackError;

    fn detect(&self, context: DetectContext<Self>) -> libcnb::Result<DetectResult, Self::Error> {
        let mut plan_builder = BuildPlanBuilder::new().provides("ruby");

        if context.app_dir.join("Gemfile.lock").exists() {
            plan_builder = plan_builder.requires("ruby");

            if context.app_dir.join("package.json").exists() {
                plan_builder = plan_builder.requires("node");
            }

            if app_needs_java(&context) {
                plan_builder = plan_builder.requires("jdk");
            }
        }

        DetectResultBuilder::pass()
            .build_plan(plan_builder.build())
            .build()
    }

    fn build(&self, context: BuildContext<Self>) -> libcnb::Result<BuildResult, Self::Error> {
        println!("---> Ruby Buildpack");

        let mut env = DefaultEnv::call(&context, &context.platform.env().clone())?;

        // Gather static information about project
        let lockfile_contents =
            std::fs::read_to_string(context.app_dir.join("Gemfile.lock")).unwrap();
        let gemfile_lock = GemfileLock::from_str(&lockfile_contents)
            .map_err(RubyBuildpackError::GemfileLockParsingError)?;
        let bundler_version = gemfile_lock.resolve_bundler("2.3.7");
        let ruby_version = gemfile_lock.resolve_ruby("3.1.2");

        // ## Install executable ruby version
        let ruby_layer = context //
            .handle_layer(
                layer_name!("ruby"),
                RubyVersionInstallLayer {
                    version: ruby_version.clone(),
                },
            )?;

        env = ruby_layer.env.apply(Scope::Build, &env);

        // Bundle install
        env = BundleInstall::call(ruby_version, bundler_version, &context, &env)?;

        println!("---> Detecting gems");
        let gem_list =
            GemList::from_bundle_list(&env).map_err(RubyBuildpackError::GemListGetError)?;

        // Assets install
        RakeApplicationTasksExecute::call(&gem_list, &context, &env)?;

        let default_process = if gem_list.has("railties") {
            ProcessBuilder::new(process_type!("web"), "bin/rails")
                .args(["server", "--port", "$PORT", "-e", "$RAILS_ENV"])
                .default(true)
                .build()
        } else {
            ProcessBuilder::new(process_type!("web"), "bundle")
                .args(["exec", "rackup", "--port", "$PORT", "--host", "0.0.0.0"])
                .default(true)
                .build()
        };
        BuildResultBuilder::new()
            .launch(LaunchBuilder::new().process(default_process).build())
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
    #[error("Bundler gem install exit: {0}")]
    GemInstallBundlerUnexpectedExitStatus(ExitStatus),
    #[error("Bundle install command errored: {0}")]
    BundleInstallCommandError(EnvCommandError),

    #[error("Could not install bundler: {0}")]
    GemInstallBundlerCommandError(EnvCommandError),

    #[error("Bundle install exit: {0}")]
    BundleInstallUnexpectedExitStatus(ExitStatus),
    #[error("Bundle config error: {0}")]
    BundleConfigCommandError(std::io::Error),
    #[error("Bundle config exit: {0}")]
    BundleConfigUnexpectedExitStatus(ExitStatus),

    #[error("Url error: {0}")]
    UrlParseError(UrlError),

    #[error("Error building list of gems for application: {0}")]
    GemListGetError(GemListError),

    #[error("Error detecting rake tasks: {0}")]
    RakeDetectError(RakeDetectError),

    #[error("Error evaluating Gemfile.lock: {0}")]
    GemfileLockParsingError(GemfileLockError),
}
impl From<RubyBuildpackError> for libcnb::Error<RubyBuildpackError> {
    fn from(error: RubyBuildpackError) -> Self {
        Self::BuildpackError(error)
    }
}

buildpack_main!(RubyBuildpack);

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_needs_java() {
        let gemfile_lock = r#""#;
        assert!(!needs_java(gemfile_lock));

        let gemfile_lock = r#"
RUBY VERSION
   ruby 2.5.7p001 (jruby 9.2.13.0)
"#;
        assert!(needs_java(gemfile_lock));
    }
}

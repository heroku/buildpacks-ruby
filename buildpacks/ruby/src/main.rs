#![warn(unused_crate_dependencies)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
use crate::layers::{RubyInstallError, RubyInstallLayer};

use commons::env_command::EnvCommandError;
use commons::gem_list::GemList;
use commons::gem_list::GemListError;
use commons::gemfile_lock::{GemfileLock, GemfileLockError};
use commons::in_app_dir_cache::InAppDirCacheError;
use commons::rake_detect::RakeError;
use core::str::FromStr;
use libcnb::build::{BuildContext, BuildResult, BuildResultBuilder};
use libcnb::data::build_plan::BuildPlanBuilder;
use libcnb::data::launch::{LaunchBuilder, ProcessBuilder};
use libcnb::data::{layer_name, process_type};
use libcnb::detect::{DetectContext, DetectResult, DetectResultBuilder};
use libcnb::generic::{GenericMetadata, GenericPlatform};
use libcnb::layer_env::Scope;
use libcnb::Platform;
use libcnb::{buildpack_main, Buildpack};
use regex::Regex;

mod layers;
mod steps;

#[cfg(test)]
use libcnb_test as _;
#[cfg(test)]
mod test_helper;

pub(crate) struct RubyBuildpack;

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

            if context.app_dir.join("yarn.lock").exists() {
                plan_builder = plan_builder.requires("yarn");
            }

            if app_needs_java(&context)? {
                plan_builder = plan_builder.requires("jdk");
            }
        }

        DetectResultBuilder::pass()
            .build_plan(plan_builder.build())
            .build()
    }

    fn build(&self, context: BuildContext<Self>) -> libcnb::Result<BuildResult, Self::Error> {
        println!("---> Ruby Buildpack");
        // ## Set default environment
        let (mut env, store) =
            crate::steps::default_env(&context, &context.platform.env().clone())?;

        // Gather static information about project
        let lockfile_contents = std::fs::read_to_string(context.app_dir.join("Gemfile.lock"))
            .map_err(RubyBuildpackError::CannotReadFile)?;
        let gemfile_lock = GemfileLock::from_str(&lockfile_contents)
            .map_err(RubyBuildpackError::GemfileLockParsingError)?;
        let bundler_version = gemfile_lock.resolve_bundler("2.3.7");
        let ruby_version = gemfile_lock.resolve_ruby("3.1.2");

        // ## Install executable ruby version
        let ruby_layer = context //
            .handle_layer(
                layer_name!("ruby"),
                RubyInstallLayer {
                    version: ruby_version.clone(),
                },
            )?;
        env = ruby_layer.env.apply(Scope::Build, &env);

        // ## Bundle install
        env = crate::steps::bundle_install(
            ruby_version,
            bundler_version,
            String::from("development:test"),
            &context,
            &env,
        )?;

        println!("---> Detecting gems");
        let gem_list =
            GemList::from_bundle_list(&env).map_err(RubyBuildpackError::GemListGetError)?;

        // ## Assets install
        crate::steps::rake_assets_precompile(&gem_list, &context, &env)?;

        let default_process = if gem_list.has("railties") {
            ProcessBuilder::new(process_type!("web"), "bin/rails")
                .args(["server", "--port", "$PORT", "--environment", "$RAILS_ENV"])
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
            .store(store)
            .build()
    }
}

fn app_needs_java(context: &DetectContext<RubyBuildpack>) -> Result<bool, RubyBuildpackError> {
    let gemfile_lock = std::fs::read_to_string(context.app_dir.join("Gemfile.lock"))
        .map_err(RubyBuildpackError::CannotReadFile)?;

    Ok(needs_java(&gemfile_lock))
}

fn needs_java(gemfile_lock: &str) -> bool {
    let java_regex = Regex::new(r"\(jruby ").expect("Internal Error: Invalid regex");
    java_regex.is_match(gemfile_lock)
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum RubyBuildpackError {
    #[error("Cannot read_file: {0}")]
    CannotReadFile(std::io::Error),

    #[error("Cannot install ruby: {0}")]
    RubyInstallError(RubyInstallError),

    #[error("Bundle install command errored: {0}")]
    BundleInstallCommandError(EnvCommandError),

    #[error("Could not install bundler: {0}")]
    GemInstallBundlerCommandError(EnvCommandError),

    #[error("Error building list of gems for application: {0}")]
    GemListGetError(GemListError),

    #[error("Error detecting rake tasks: {0}")]
    RakeDetectError(RakeError),

    #[error("Error running rake assets precompile: {0}")]
    RakeAssetsPrecompileFailed(commons::env_command::EnvCommandError),

    #[error("Error cleaning asset cache: {0}")]
    RakeAssetsCleanFailed(commons::env_command::EnvCommandError),

    #[error("Error evaluating Gemfile.lock: {0}")]
    GemfileLockParsingError(GemfileLockError),

    #[error("Error caching application path: {0}")]
    InAppDirCacheError(InAppDirCacheError),
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

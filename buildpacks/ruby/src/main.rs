#![warn(unused_crate_dependencies)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
use crate::layers::metrics_agent_install::{MetricsAgentInstall, MetricsAgentInstallError};
use crate::layers::{RubyInstallError, RubyInstallLayer};
use crate::rake_task_detect::RakeError;
use commons::build_output;
use commons::cache::CacheError;
use commons::fun_run::CmdError;
use commons::gemfile_lock::GemfileLock;

#[allow(clippy::wildcard_imports)]
use commons::output::{
    interface::*,
    log::{BuildLog, LayerLogger},
};
use core::str::FromStr;
use layers::{BundleDownloadLayer, BundleInstallLayer};
use libcnb::build::{BuildContext, BuildResult, BuildResultBuilder};
use libcnb::data::build_plan::BuildPlanBuilder;
use libcnb::data::launch::LaunchBuilder;
use libcnb::data::layer_name;
use libcnb::detect::{DetectContext, DetectResult, DetectResultBuilder};
use libcnb::generic::{GenericMetadata, GenericPlatform};
use libcnb::layer_env::Scope;
use libcnb::Platform;
use libcnb::{buildpack_main, Buildpack};
use regex::Regex;
use std::io::stdout;

mod gem_list;
mod layers;
mod rake_status;
mod rake_task_detect;
mod steps;
mod user_errors;

#[cfg(test)]
use libcnb_test as _;

use clap as _;

pub(crate) struct RubyBuildpack;

impl Buildpack for RubyBuildpack {
    type Platform = GenericPlatform;
    type Metadata = GenericMetadata;
    type Error = RubyBuildpackError;

    fn detect(&self, context: DetectContext<Self>) -> libcnb::Result<DetectResult, Self::Error> {
        let mut plan_builder = BuildPlanBuilder::new().provides("ruby");

        if let Ok(lockfile) = fs_err::read_to_string(context.app_dir.join("Gemfile.lock")) {
            plan_builder = plan_builder.requires("ruby");

            if context.app_dir.join("package.json").exists() {
                plan_builder = plan_builder.requires("node");
            }

            if context.app_dir.join("yarn.lock").exists() {
                plan_builder = plan_builder.requires("yarn");
            }

            if needs_java(&lockfile) {
                plan_builder = plan_builder.requires("jdk");
            }
        } else if context.app_dir.join("Gemfile").exists() {
            plan_builder = plan_builder.requires("ruby");
        }

        DetectResultBuilder::pass()
            .build_plan(plan_builder.build())
            .build()
    }

    fn build(&self, context: BuildContext<Self>) -> libcnb::Result<BuildResult, Self::Error> {
        let mut logger = BuildLog::new(stdout()).buildpack_name("Heroku Ruby Buildpack");
        let build_duration = build_output::buildpack_name("Heroku Ruby Buildpack");

        // ## Set default environment
        let (mut env, store) =
            crate::steps::default_env(&context, &context.platform.env().clone())?;

        // Gather static information about project
        let lockfile_contents = fs_err::read_to_string(context.app_dir.join("Gemfile.lock"))
            .map_err(RubyBuildpackError::MissingGemfileLock)?;
        let gemfile_lock = GemfileLock::from_str(&lockfile_contents).expect("Infallible");
        let bundler_version = gemfile_lock.resolve_bundler("2.4.5");
        let ruby_version = gemfile_lock.resolve_ruby("3.1.3");

        // ## Install metrics agent
        let section = build_output::section("Metrics agent");
        if lockfile_contents.contains("barnes") {
            context.handle_layer(
                layer_name!("metrics_agent"),
                MetricsAgentInstall { section },
            )?;
        } else {
            section.say_with_details(
                "Skipping install",
                "`gem 'barnes'` not found in Gemfile.lock",
            );
        };

        // ## Install executable ruby version

        (logger, env) = {
            let logger = logger.section(&format!(
                "Ruby version {} from {}",
                build_output::fmt::value(ruby_version.to_string()),
                build_output::fmt::value(gemfile_lock.ruby_source())
            ));
            let mut layer_logger = LayerLogger::new(logger);
            let ruby_layer = context //
                .handle_layer(
                    layer_name!("ruby"),
                    RubyInstallLayer {
                        version: ruby_version.clone(),
                        layer_logger: layer_logger.clone(),
                    },
                )?;
            let env = ruby_layer.env.apply(Scope::Build, &env);
            (layer_logger.finish_layer(), env)
        };

        // ## Setup bundler
        (logger, env) = {
            let section = logger.section(&format!(
                "Bundler version {} from {}",
                build_output::fmt::value(bundler_version.to_string()),
                build_output::fmt::value(gemfile_lock.bundler_source())
            ));
            let mut layer_logger = LayerLogger::new(section);
            let download_bundler_layer = context.handle_layer(
                layer_name!("bundler"),
                BundleDownloadLayer {
                    env: env.clone(),
                    version: bundler_version,
                    logger: layer_logger.clone(),
                },
            )?;
            let logger = layer_logger.finish_layer();
            let env = download_bundler_layer.env.apply(Scope::Build, &env);

            (logger, env)
        };

        // ## Bundle install
        (logger, env) = {
            let section = logger.section("Bundle install");
            let mut layer_logger = LayerLogger::new(section);

            let bundle_install_layer = context.handle_layer(
                layer_name!("gems"),
                BundleInstallLayer {
                    env: env.clone(),
                    without: BundleWithout::new("development:test"),
                    ruby_version,
                    logger: layer_logger.clone(),
                },
            )?;
            let env = bundle_install_layer.env.apply(Scope::Build, &env);
            let logger = layer_logger.finish_layer();
            (logger, env)
        };

        // ## Detect gems
        let (logger, gem_list, default_process) = {
            let section = logger.section("Setting default processes(es)");
            let layer_logger = LayerLogger::new(section);

            let gem_list = gem_list::GemList::from_bundle_list(&env, &layer_logger)
                .map_err(RubyBuildpackError::GemListGetError)?;
            let default_process = steps::get_default_process(&layer_logger, &context, &gem_list);

            (layer_logger.finish_layer(), gem_list, default_process)
        };

        // ## Assets install

        {
            let section = build_output::section("Rake assets install");
            let rake_detect = crate::steps::detect_rake_tasks(&section, &gem_list, &context, &env)?;

            if let Some(rake_detect) = rake_detect {
                crate::steps::rake_assets_install(&section, &context, &env, &rake_detect)?;
            }
        };
        build_duration.done_timed();

        if let Some(default_process) = default_process {
            BuildResultBuilder::new()
                .launch(LaunchBuilder::new().process(default_process).build())
                .store(store)
                .build()
        } else {
            BuildResultBuilder::new().store(store).build()
        }
    }

    fn on_error(&self, err: libcnb::Error<Self::Error>) {
        user_errors::on_error(err);
    }
}

fn needs_java(gemfile_lock: &str) -> bool {
    let java_regex = Regex::new(r"\(jruby ").expect("Internal Error: Invalid regex");
    java_regex.is_match(gemfile_lock)
}

#[derive(Debug)]
pub(crate) enum RubyBuildpackError {
    RakeDetectError(RakeError),
    GemListGetError(gem_list::ListError),
    RubyInstallError(RubyInstallError),
    MetricsAgentError(MetricsAgentInstallError),
    MissingGemfileLock(std::io::Error),
    InAppDirCacheError(CacheError),
    BundleInstallDigestError(commons::metadata_digest::DigestError),
    BundleInstallCommandError(CmdError),
    RakeAssetsPrecompileFailed(CmdError),
    GemInstallBundlerCommandError(CmdError),
}

impl From<RubyBuildpackError> for libcnb::Error<RubyBuildpackError> {
    fn from(error: RubyBuildpackError) -> Self {
        libcnb::Error::BuildpackError(error)
    }
}

buildpack_main!(RubyBuildpack);

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct BundleWithout(String);

impl BundleWithout {
    fn new(without: impl AsRef<str>) -> Self {
        Self(String::from(without.as_ref()))
    }
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_needs_java() {
        let gemfile_lock = r"";
        assert!(!needs_java(gemfile_lock));

        let gemfile_lock = r"
RUBY VERSION
   ruby 2.5.7p001 (jruby 9.2.13.0)
";
        assert!(needs_java(gemfile_lock));
    }
}

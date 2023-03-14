#![warn(unused_crate_dependencies)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
use crate::layers::{RubyInstallError, RubyInstallLayer};
use commons::cache::CacheError;
use commons::cute_cmd::CuteCmdError;
use commons::env_command::CommandError;
use commons::gem_list::GemList;
use commons::gemfile_lock::GemfileLock;
use commons::rake_task_detect::RakeError;
use core::str::FromStr;
use libcnb::build::{BuildContext, BuildResult, BuildResultBuilder};
use libcnb::data::build_plan::BuildPlanBuilder;
use libcnb::data::launch::LaunchBuilder;
use libcnb::data::layer_name;
use libcnb::detect::{DetectContext, DetectResult, DetectResultBuilder};
use libcnb::generic::{GenericMetadata, GenericPlatform};
use libcnb::layer_env::Scope;
use libcnb::Platform;
use libcnb::{buildpack_main, Buildpack};
use libherokubuildpack::log as user;
use regex::Regex;
use std::fmt::Display;

mod layers;
mod steps;
mod user_errors;

#[cfg(test)]
use libcnb_test as _;

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
        let total = std::time::Instant::now();
        let section = header("Heroku Ruby buildpack");
        user::log_info("Running Heroku Ruby buildpack");
        section.done_quiet();

        let section = header("Setting environment");
        user::log_info("Setting default environment values");
        // ## Set default environment
        let (mut env, store) =
            crate::steps::default_env(&context, &context.platform.env().clone())?;
        section.done();

        // Gather static information about project
        let section = header("Detecting versions");
        let lockfile_contents = fs_err::read_to_string(context.app_dir.join("Gemfile.lock"))
            .map_err(RubyBuildpackError::MissingGemfileLock)?;
        let gemfile_lock = GemfileLock::from_str(&lockfile_contents).expect("Infallible");
        let bundler_version = gemfile_lock.resolve_bundler("2.4.5");
        let ruby_version = gemfile_lock.resolve_ruby("3.1.3");
        user::log_info(format!("Detected ruby: {ruby_version}"));
        user::log_info(format!("Detected bundler: {bundler_version}"));
        section.done();

        // ## Install executable ruby version
        let section = header("Installing Ruby");
        let ruby_layer = context //
            .handle_layer(
                layer_name!("ruby"),
                RubyInstallLayer {
                    version: ruby_version.clone(),
                },
            )?;
        env = ruby_layer.env.apply(Scope::Build, &env);
        section.done();

        // ## Setup bundler
        let section = header("Installing Bundler");
        env = crate::steps::bundler_download(bundler_version, &context, &env)?;
        section.done();

        // ## Bundle install
        let section = header("Installing dependencies");
        env = crate::steps::bundle_install(
            &context,
            BundleWithout(String::from("development:test")),
            ruby_version,
            &env,
        )?;
        section.done();

        // ## Detect gems
        let section = header("Detecting gems");
        user::log_info("Detecting gems via `bundle list`");
        let gem_list =
            GemList::from_bundle_list(&env).map_err(RubyBuildpackError::GemListGetError)?;
        section.done();

        let section = header("Setting default process(es)");
        let default_process = steps::get_default_process(&context, &gem_list);
        section.done();

        // ## Assets install
        let section = header("Rake task detection");
        let rake_detect = crate::steps::detect_rake_tasks(&gem_list, &context, &env)?;
        section.done();

        if let Some(rake_detect) = rake_detect {
            let section = header("Rake asset installation");
            crate::steps::rake_assets_install(&context, &env, &rake_detect)?;
            section.done();
        }

        let duration = total.elapsed();
        user::log_header("Heroku Ruby buildpack finished");
        user::log_info(format!(
            "Finished ({} total elapsed time)\n",
            DisplayDuration::new(&duration)
        ));

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
    GemListGetError(commons::gem_list::ListError),
    RubyInstallError(RubyInstallError),
    MissingGemfileLock(std::io::Error),
    InAppDirCacheError(CacheError),
    BundleInstallDigestError(commons::metadata_digest::DigestError),
    BundleInstallCommandError(CuteCmdError),
    RakeAssetsPrecompileFailed(CommandError),
    GemInstallBundlerCommandError(CommandError),
}

impl From<RubyBuildpackError> for libcnb::Error<RubyBuildpackError> {
    fn from(error: RubyBuildpackError) -> Self {
        libcnb::Error::BuildpackError(error)
    }
}

buildpack_main!(RubyBuildpack);

/// Use for logging a duration
#[derive(Debug)]
struct LogSectionWithTime {
    start: std::time::Instant,
}

impl LogSectionWithTime {
    fn done(&self) {
        let diff = &self.start.elapsed();
        let duration = DisplayDuration::new(diff);

        user::log_info(format!("Done ({duration})"));
    }

    #[allow(clippy::unused_self)]
    fn done_quiet(&self) {}
}

/// Prints out a header and ensures a done section is printed
///
/// Returns a `LogSectionWithTime` that must be used. That
/// will print out the elapsed time.
#[must_use]
fn header(message: &str) -> LogSectionWithTime {
    user::log_header(message);

    let start = std::time::Instant::now();

    LogSectionWithTime { start }
}

#[derive(Debug)]
struct DisplayDuration<'a> {
    duration: &'a std::time::Duration,
}

impl DisplayDuration<'_> {
    fn new(duration: &std::time::Duration) -> DisplayDuration {
        DisplayDuration { duration }
    }

    fn milliseconds(&self) -> u32 {
        self.duration.subsec_millis()
    }

    fn seconds(&self) -> u64 {
        self.duration.as_secs() % 60
    }

    fn minutes(&self) -> u64 {
        (self.duration.as_secs() / 60) % 60
    }

    fn hours(&self) -> u64 {
        (self.duration.as_secs() / 3600) % 60
    }
}

impl Display for DisplayDuration<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hours = self.hours();
        let minutes = self.minutes();
        let seconds = self.seconds();
        let miliseconds = self.milliseconds();

        if self.hours() > 0 {
            f.write_fmt(format_args!("{hours}h {minutes}m {seconds}s"))
        } else if self.minutes() > 0 {
            f.write_fmt(format_args!("{minutes}m {seconds}s"))
        } else {
            f.write_fmt(format_args!("{seconds}.{miliseconds:0>3}s"))
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct BundleWithout(String);

impl BundleWithout {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_display_duration() {
        let diff = std::time::Duration::from_millis(1024);
        assert_eq!("1.024s", format!("{}", DisplayDuration::new(&diff)));

        let diff = std::time::Duration::from_millis(60 * 1024);
        assert_eq!("1m 1s", format!("{}", DisplayDuration::new(&diff)));

        let diff = std::time::Duration::from_millis(3600 * 1024);
        assert_eq!("1h 1m 26s", format!("{}", DisplayDuration::new(&diff)));
    }

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

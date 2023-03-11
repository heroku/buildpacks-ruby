#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
use crate::layers::RubyInstallError;
use bundler_version::BundleWithout;
use commons::cache::CacheError;
use commons::display::SentenceList;
use commons::env_command::CommandError;
use commons::gem_list::GemList;
use commons::rake_task_detect::RakeError;
use libcnb::build::{BuildContext, BuildResult, BuildResultBuilder};
use libcnb::data::build_plan::BuildPlanBuilder;
use libcnb::data::launch::LaunchBuilder;
use libcnb::detect::{DetectContext, DetectResult, DetectResultBuilder};
use libcnb::generic::{GenericMetadata, GenericPlatform};
use libcnb::Platform;
use libcnb::{buildpack_main, Buildpack};
use libherokubuildpack::log as user;
use regex::Regex;
use ruby_version::RubyVersionError;
use std::fmt::Display;
use std::time::Instant;

#[macro_use]
extern crate lazy_static;

mod bundler_version;
mod gemfile_lock;
mod layers;
mod ruby_version;
mod steps;
mod user_errors;

#[cfg(test)]
use libcnb_test as _;

pub(crate) struct RubyBuildpack;

/// List of known valid stacks that the buildpack supports.
const KNOWN_SUPPORTED_STACKS: &[&str] = &["heroku-20", "heroku-22"];

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
        let build_time = header("Heroku Ruby buildpack").timer();
        user::log_info("Running");

        // ## Set default environment
        let project_setup = header("Preparing build");
        let lockfile = fs_err::read_to_string(context.app_dir.join("Gemfile.lock"))
            .map_err(RubyBuildpackError::MissingGemfileLock)?;

        log_stack_support(&context);

        user::log_info("Setting default environment values");
        let (mut env, store) =
            crate::steps::default_env(&context, &context.platform.env().clone())?;

        let ruby = ruby_version::from_lockfile(&lockfile, &env)
            .map_err(RubyBuildpackError::RubyVersionError)?;
        let bundler = bundler_version::from_lockfile(&lockfile);
        project_setup.done();

        let ruby_install = header("Installing Ruby");
        env = ruby_version::download(&ruby, &context, &env)?;
        ruby_install.done();

        let bundle_install = header("Installing dependencies");
        env = bundler_version::download(bundler, &context, &env)?;
        env = bundler_version::install_dependencies(
            &context,
            BundleWithout(String::from("development:test")),
            ruby.cache_key(),
            &env,
        )?;
        bundle_install.done();

        let section = header("Detecting gems");
        user::log_info("Detecting gems via `bundle list`");
        let gem_list =
            GemList::from_bundle_list(&env).map_err(RubyBuildpackError::GemListGetError)?;
        section.done();

        let detect_rake_tasks = header("Rake tasks");
        let rake_detect = crate::steps::detect_rake_tasks(&gem_list, &context, &env)?;
        detect_rake_tasks.done();

        if let Some(rake_detect) = rake_detect {
            let assets_precompile = header("Rake asset installation");
            crate::steps::rake_assets_install(&context, &env, &rake_detect)?;
            assets_precompile.done();
        }

        let find_process_types = header("Setting default process(es)");
        let default_process = steps::get_default_process(&context, &gem_list);
        find_process_types.done();

        user::log_header("Heroku Ruby buildpack");
        let build_time = build_time.elapsed();
        let build_duration = DisplayDuration::new(&build_time);
        user::log_info(format!("Finished ({build_duration} total elapsed time)\n"));

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
    BundleInstallCommandError(CommandError),
    RakeAssetsPrecompileFailed(CommandError),
    GemInstallBundlerCommandError(CommandError),
    RubyVersionError(RubyVersionError),
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
    start: Instant,
}

impl LogSectionWithTime {
    fn done(self) {
        let diff = &self.start.elapsed();
        let duration = DisplayDuration::new(diff);

        user::log_info(format!("Done ({duration})"));
    }

    #[must_use]
    fn timer(self) -> Instant {
        self.start
    }
}

/// Prints out a header and ensures a done section is printed
///
/// Returns a `LogSectionWithTime` that must be used. That
/// will print out the elapsed time.
#[must_use]
fn header(message: &str) -> LogSectionWithTime {
    user::log_header(message);

    let start = Instant::now();

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

fn log_stack_support(context: &BuildContext<RubyBuildpack>) {
    let stack_string = context.stack_id.to_string();
    if KNOWN_SUPPORTED_STACKS.contains(&stack_string.as_str()) {
        user::log_info(format!(
            "Detected using stack {stack_string}, this is a known supported stack."
        ));
    } else {
        user::log_info(format!(
            "Detected using stack {stack_string}, support is unknown. Known stacks: {known_stacks}",
            known_stacks = SentenceList {
                list: KNOWN_SUPPORTED_STACKS,
                ..SentenceList::default()
            }
        ));
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

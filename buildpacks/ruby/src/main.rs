// cargo-llvm-cov sets the coverage_nightly attribute when instrumenting our code. In that case,
// we enable https://doc.rust-lang.org/beta/unstable-book/language-features/coverage-attribute.html
// to be able selectively opt out of coverage for functions/lines/modules.
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use bullet_stream::global::print;
use bullet_stream::{style, Print};
use commons::cache::CacheError;
use commons::gemfile_lock::GemfileLock;
use commons::metadata_digest::MetadataDigest;
use core::str::FromStr;
use fs_err::PathExt;
use fun_run::CmdError;
use layers::{
    metrics_agent_install::MetricsAgentInstallError, ruby_install_layer::RubyInstallError,
};
use libcnb::build::{BuildContext, BuildResult, BuildResultBuilder};
use libcnb::data::build_plan::BuildPlanBuilder;
use libcnb::data::launch::LaunchBuilder;
use libcnb::data::layer_name;
use libcnb::detect::{DetectContext, DetectResult, DetectResultBuilder};
use libcnb::generic::{GenericMetadata, GenericPlatform};
use libcnb::layer::UncachedLayerDefinition;
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::Platform;
use libcnb::{buildpack_main, Buildpack};

mod gem_list;
mod layers;
mod rake_status;
mod rake_task_detect;
mod steps;
mod target_id;
mod user_errors;

#[cfg(test)]
use libcnb_test as _;
#[cfg(test)]
use pretty_assertions as _;

use clap as _;

use crate::target_id::OsDistribution;

struct RubyBuildpack;

#[derive(Debug, thiserror::Error)]
enum DetectError {
    #[error("Cannot read Gemfile {0}")]
    Gemfile(std::io::Error),

    #[error("Cannot read Gemfile.lock {0}")]
    GemfileLock(std::io::Error),

    #[error("Cannot read package.json {0}")]
    PackageJson(std::io::Error),

    #[error("Cannot read yarn.lock {0}")]
    YarnLock(std::io::Error),
}

impl Buildpack for RubyBuildpack {
    type Platform = GenericPlatform;
    type Metadata = GenericMetadata;
    type Error = RubyBuildpackError;

    fn detect(&self, context: DetectContext<Self>) -> libcnb::Result<DetectResult, Self::Error> {
        let mut plan_builder = BuildPlanBuilder::new().provides("ruby");

        let lockfile = context.app_dir.join("Gemfile.lock");

        if lockfile
            .fs_err_try_exists()
            .map_err(DetectError::GemfileLock)
            .map_err(RubyBuildpackError::BuildpackDetectionError)?
        {
            plan_builder = plan_builder.requires("ruby");

            if context
                .app_dir
                .join("package.json")
                .fs_err_try_exists()
                .map_err(DetectError::PackageJson)
                .map_err(RubyBuildpackError::BuildpackDetectionError)?
            {
                plan_builder = plan_builder.requires("node");
            }

            if context
                .app_dir
                .join("yarn.lock")
                .fs_err_try_exists()
                .map_err(DetectError::YarnLock)
                .map_err(RubyBuildpackError::BuildpackDetectionError)?
            {
                plan_builder = plan_builder.requires("yarn");
            }

            if fs_err::read_to_string(lockfile)
                .map_err(DetectError::GemfileLock)
                .map_err(RubyBuildpackError::BuildpackDetectionError)
                .map(needs_java)?
            {
                plan_builder = plan_builder.requires("jdk");
            }
        } else if context
            .app_dir
            .join("Gemfile")
            .fs_err_try_exists()
            .map_err(DetectError::Gemfile)
            .map_err(RubyBuildpackError::BuildpackDetectionError)?
        {
            plan_builder = plan_builder.requires("ruby");
        }

        DetectResultBuilder::pass()
            .build_plan(plan_builder.build())
            .build()
    }

    #[allow(clippy::too_many_lines)]
    fn build(&self, context: BuildContext<Self>) -> libcnb::Result<BuildResult, Self::Error> {
        let started = print::buildpack("Heroku Ruby Buildpack");
        let mut build_output = Print::global().h2("Heroku Ruby Buildpack");

        // ## Set default environment
        let (mut env, store) =
            crate::steps::default_env(&context, &context.platform.env().clone())?;

        // Gather static information about project
        let lockfile = context.app_dir.join("Gemfile.lock");
        let lockfile_contents = fs_err::read_to_string(&lockfile)
            .map_err(|error| RubyBuildpackError::MissingGemfileLock(lockfile, error))?;
        let gemfile_lock = GemfileLock::from_str(&lockfile_contents).expect("Infallible");
        let bundler_version = gemfile_lock.resolve_bundler("2.5.23");
        let ruby_version = gemfile_lock.resolve_ruby("3.3.7");
        tracing::info!(
            // Bundler version the app is asking for i.e. "2.6.7"
            cnb.ruby.bundler.version = bundler_version.to_string(),
            // Where the bundler version came from
            // Either "Gemfile.lock" or "default"
            cnb.ruby.bundler.sourced_from = gemfile_lock.bundler_source(),
            // Ruby version the app is asking for i.e. "3.4.2" for MRI or "2.5.7-jruby-9.2.13.0" for jruby
            cnb.ruby.runtime.version = ruby_version.to_string(),
            // Where the Ruby version came from
            // Either "Gemfile.lock" or "default"
            cnb.ruby.runtime.sourced_from = gemfile_lock.ruby_source()
        );
        // ## Install metrics agent
        build_output = {
            let bullet = build_output.bullet("Metrics agent");
            if lockfile_contents.contains("barnes") {
                layers::metrics_agent_install::handle_metrics_agent_layer(&context, bullet)?.done()
            } else {
                print::sub_bullet(format!(
                    "Skipping install ({barnes} gem not found)",
                    barnes = style::value("barnes")
                ));
                bullet.done()
            }
        };

        // ## Install executable ruby version
        (build_output, env) = {
            let bullet = build_output.bullet(format!(
                "Ruby version {} from {}",
                style::value(ruby_version.to_string()),
                style::value(gemfile_lock.ruby_source())
            ));
            let (bullet, layer_env) = layers::ruby_install_layer::handle(
                &context,
                bullet,
                &layers::ruby_install_layer::Metadata {
                    os_distribution: OsDistribution {
                        name: context.target.distro_name.clone(),
                        version: context.target.distro_version.clone(),
                    },
                    cpu_architecture: context.target.arch.clone(),
                    ruby_version: ruby_version.clone(),
                },
            )?;

            (bullet.done(), layer_env.apply(Scope::Build, &env))
        };

        // ## Setup bundler
        (build_output, env) = {
            let bullet = build_output.bullet(format!(
                "Bundler version {} from {}",
                style::value(bundler_version.to_string()),
                style::value(gemfile_lock.bundler_source())
            ));
            let (bullet, layer_env) = layers::bundle_download_layer::handle(
                &context,
                &env,
                bullet,
                &layers::bundle_download_layer::Metadata {
                    version: bundler_version,
                },
            )?;

            (bullet.done(), layer_env.apply(Scope::Build, &env))
        };

        // ## Bundle install
        env = {
            print::bullet("Bundle install gems");
            let layer_env = layers::bundle_install_layer::handle(
                &context,
                &env,
                &layers::bundle_install_layer::Metadata {
                    os_distribution: OsDistribution {
                        name: context.target.distro_name.clone(),
                        version: context.target.distro_version.clone(),
                    },
                    cpu_architecture: context.target.arch.clone(),
                    ruby_version: ruby_version.clone(),
                    force_bundle_install_key: String::from(
                        crate::layers::bundle_install_layer::FORCE_BUNDLE_INSTALL_CACHE_KEY,
                    ),
                    digest: MetadataDigest::new_env_files(
                        &context.platform,
                        &[
                            &context.app_dir.join("Gemfile"),
                            &context.app_dir.join("Gemfile.lock"),
                        ],
                    )
                    .map_err(|error| match error {
                        commons::metadata_digest::DigestError::CannotReadFile(path, error) => {
                            RubyBuildpackError::BundleInstallDigestError(path, error)
                        }
                    })?,
                },
                &BundleWithout::new("development:test"),
            )?;

            layer_env.apply(Scope::Build, &env)
        };

        env = {
            let user_binstubs = context.uncached_layer(
                layer_name!("user_binstubs"),
                UncachedLayerDefinition {
                    build: true,
                    launch: true,
                },
            )?;
            user_binstubs.write_env(
                LayerEnv::new()
                    .chainable_insert(Scope::All, ModificationBehavior::Delimiter, "PATH", ":")
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Prepend,
                        "PATH",
                        context.app_dir.join("bin"),
                    ),
            )?;
            user_binstubs.read_env()?.apply(Scope::Build, &env)
        };

        // ## Detect gems
        print::bullet("Default process detection");
        let gem_list = gem_list::bundle_list(&env).map_err(RubyBuildpackError::GemListGetError)?;
        let default_process = steps::get_default_process(&context, &gem_list);

        // ## Assets install
        print::bullet("Rake assets install");
        if let Some(rake_detect) = crate::steps::detect_rake_tasks(&gem_list, &context, &env)? {
            crate::steps::rake_assets_install(&context, &env, &rake_detect)?;
        }
        print::all_done(&Some(started));

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

fn needs_java(gemfile_lock: impl AsRef<str>) -> bool {
    let java_regex = regex::Regex::new(r"\(jruby ").expect("clippy");
    java_regex.is_match(gemfile_lock.as_ref())
}

#[derive(Debug)]
pub(crate) enum RubyBuildpackError {
    BuildpackDetectionError(DetectError),
    RakeDetectError(CmdError),
    GemListGetError(CmdError),
    RubyInstallError(RubyInstallError),
    MetricsAgentError(MetricsAgentInstallError),
    MissingGemfileLock(std::path::PathBuf, std::io::Error),
    InAppDirCacheError(CacheError),
    BundleInstallDigestError(std::path::PathBuf, std::io::Error),
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
#[serde(deny_unknown_fields)]
struct BundleWithout(String);

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

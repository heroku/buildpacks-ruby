use commons::cache::CacheError;
use commons::gemfile_lock::GemfileLock;
use commons::metadata_digest::MetadataDigest;
use commons::output::warn_later::WarnGuard;
#[allow(clippy::wildcard_imports)]
use commons::output::{build_log::*, fmt};
use core::str::FromStr;
use fs_err::PathExt;
use fun_run::CmdError;
use layers::{
    bundle_download_layer::{BundleDownloadLayer, BundleDownloadLayerMetadata},
    bundle_install_layer::{BundleInstallLayer, BundleInstallLayerMetadata},
    metrics_agent_install::MetricsAgentInstallError,
    ruby_install_layer::{RubyInstallError, RubyInstallLayer, RubyInstallLayerMetadata},
};
use libcnb::build::{BuildContext, BuildResult, BuildResultBuilder};
use libcnb::data::build_plan::BuildPlanBuilder;
use libcnb::data::launch::LaunchBuilder;
use libcnb::data::layer_name;
use libcnb::detect::{DetectContext, DetectResult, DetectResultBuilder};
use libcnb::generic::{GenericMetadata, GenericPlatform};
use libcnb::layer_env::Scope;
use libcnb::Platform;
use libcnb::{buildpack_main, Buildpack};
use std::io::stdout;

mod gem_list;
mod layers;
mod rake_status;
mod rake_task_detect;
mod steps;
mod target_id;
mod user_errors;

#[cfg(test)]
use libcnb_test as _;

use clap as _;

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
        let mut logger = BuildLog::new(stdout()).buildpack_name("Heroku Ruby Buildpack");
        let warn_later = WarnGuard::new(stdout());

        // ## Set default environment
        let (mut env, store) =
            crate::steps::default_env(&context, &context.platform.env().clone())?;

        // Gather static information about project
        let lockfile = context.app_dir.join("Gemfile.lock");
        let lockfile_contents = fs_err::read_to_string(&lockfile)
            .map_err(|error| RubyBuildpackError::MissingGemfileLock(lockfile, error))?;
        let gemfile_lock = GemfileLock::from_str(&lockfile_contents).expect("Infallible");
        let bundler_version = gemfile_lock.resolve_bundler("2.4.5");
        let ruby_version = gemfile_lock.resolve_ruby("3.1.3");

        // ## Install metrics agent
        (logger, env) = {
            let section = logger.section("Metrics agent");
            if lockfile_contents.contains("barnes") {
                let layer_ref = layers::metrics_agent_install::handle_metrics_agent_layer(
                    &context,
                    section.as_ref(),
                )?;
                (
                    section.end_section(),
                    // TODO: This should likely be removed (also eliminating the need for the `handle_metrics_agent_layer` function to return a `LayerRef`)?
                    // Included the logic here for reference, but it doesn't seem necessary (the layer env isn't explicitly modified as far as I can tell, but perhaps there's some trait API magic happening I'm not familiar with?).
                    layer_ref.read_env()?.apply(Scope::Build, &env),
                )
            } else {
                (
                    section
                        .step(&format!(
                            "Skipping install ({barnes} gem not found)",
                            barnes = fmt::value("barnes")
                        ))
                        .end_section(),
                    env,
                )
            }
        };

        // ## Install executable ruby version
        (logger, env) = {
            let section = logger.section(&format!(
                "Ruby version {} from {}",
                fmt::value(ruby_version.to_string()),
                fmt::value(gemfile_lock.ruby_source())
            ));
            let ruby_layer = context //
                .handle_layer(
                    layer_name!("ruby"),
                    RubyInstallLayer {
                        _in_section: section.as_ref(),
                        metadata: RubyInstallLayerMetadata {
                            distro_name: context.target.distro_name.clone(),
                            distro_version: context.target.distro_version.clone(),
                            cpu_architecture: context.target.arch.clone(),
                            ruby_version: ruby_version.clone(),
                        },
                    },
                )?;
            let env = ruby_layer.env.apply(Scope::Build, &env);
            (section.end_section(), env)
        };

        // ## Setup bundler
        (logger, env) = {
            let section = logger.section(&format!(
                "Bundler version {} from {}",
                fmt::value(bundler_version.to_string()),
                fmt::value(gemfile_lock.bundler_source())
            ));
            let download_bundler_layer = context.handle_layer(
                layer_name!("bundler"),
                BundleDownloadLayer {
                    env: env.clone(),
                    metadata: BundleDownloadLayerMetadata {
                        version: bundler_version,
                    },
                    _section_logger: section.as_ref(),
                },
            )?;
            let env = download_bundler_layer.env.apply(Scope::Build, &env);

            (section.end_section(), env)
        };

        // ## Bundle install
        (logger, env) = {
            let section = logger.section("Bundle install");
            let bundle_install_layer = context.handle_layer(
                layer_name!("gems"),
                BundleInstallLayer {
                    env: env.clone(),
                    without: BundleWithout::new("development:test"),
                    _section_log: section.as_ref(),
                    metadata: BundleInstallLayerMetadata {
                        distro_name: context.target.distro_name.clone(),
                        distro_version: context.target.distro_version.clone(),
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
                },
            )?;
            let env = bundle_install_layer.env.apply(Scope::Build, &env);
            (section.end_section(), env)
        };

        // ## Detect gems
        let (mut logger, gem_list, default_process) = {
            let section = logger.section("Setting default processes");

            let gem_list = gem_list::GemList::from_bundle_list(&env, section.as_ref())
                .map_err(RubyBuildpackError::GemListGetError)?;
            let default_process = steps::get_default_process(section.as_ref(), &context, &gem_list);

            (section.end_section(), gem_list, default_process)
        };

        // ## Assets install
        logger = {
            let section = logger.section("Rake assets install");
            let rake_detect =
                crate::steps::detect_rake_tasks(section.as_ref(), &gem_list, &context, &env)?;

            if let Some(rake_detect) = rake_detect {
                crate::steps::rake_assets_install(section.as_ref(), &context, &env, &rake_detect)?;
            }

            section.end_section()
        };
        logger.finish_logging();
        warn_later.warn_now();

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

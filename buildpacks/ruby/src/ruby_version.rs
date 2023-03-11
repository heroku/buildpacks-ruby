use crate::gemfile_lock::{self, EngineVersion, LockfileRuby};
use commons::env_command::{CommandError, EnvCommand};
use indoc::formatdoc;
use libcnb::build::BuildContext;
use libcnb::data::layer_name;
use libcnb::layer_env::Scope;
use libcnb::Env;
use libherokubuildpack::log as user;
use serde::Deserialize;
use serde::Serialize;
use std::fmt::Display;

use crate::layers::RubyInstallLayer;
use crate::RubyBuildpack;
use crate::RubyBuildpackError;

/// Used when no version is found in the Gemfile.lock
/// Default version is not sticky
const DEFAULT_RUBY_VERSION: &str = "3.1.3";

/// Env key to disable Ruby installation. When set this tells the Ruby
/// buildpack that we want to skip Ruby installation and instead rely on
/// "system ruby" basically whatever is on the PATH that responds to
/// `which ruby` this may come from a buildpack or the host OS.
const HEROKU_USE_SYSTEM_RUBY: &str = "HEROKU_USE_SYSTEM_RUBY";

/// Return a `Ruby` version based on Gemfile.lock contents and `HEROKU_USE_SYSTEM_RUBY`
///
/// ## Errors
///
/// - `HEROKU_USE_SYSTEM_RUBY` is set, but `ruby -v` errors
/// - Ruby engine version is specified, without a Ruby version
pub(crate) fn from_lockfile(lockfile: &str, env: &Env) -> Result<Ruby, RubyVersionError> {
    if let Some(value) = env.get("HEROKU_USE_SYSTEM_RUBY") {
        user::log_warning(
            "Using System Ruby",
            formatdoc! {"
                Found environment variable {HEROKU_USE_SYSTEM_RUBY}={value:?}. When enabled, you are
                responsible for ensuring a Ruby version is already installed from a prior buildpack
                or that your operating system contains a ruby version you can use.

                Behavior that occurs in this buildpack after this warning message are not supported.
                To report a bug or receive support for any behavior beyond this point, you must reproduce
                the problem without using this env var setting.

                This setting is experimental, it may be removed in the future.
            "},
        );

        user::log_info("Detecting system ruby version");

        let command = EnvCommand::new("ruby", &["-v"], env);

        user::log_info(format!("Running: {command}"));

        let key = command
            .stream()
            .map(|output| output.stdout)
            .map(|stdout| String::from_utf8_lossy(&stdout).to_string())
            .map(|output| RubyCacheKey(format!("{output} (system)")))
            .map_err(RubyVersionError::SystemRubyDetectionFailed)?;

        user::log_info(format!("Found Ruby version {key}"));

        Ok(Ruby::System(key))
    } else {
        user::log_info("Detecting ruby version from Gemfile.lock");
        match gemfile_lock::ruby_info(lockfile) {
            LockfileRuby::Version(version) => {
                let version = RubyVersion(version);
                user::log_info(format!("Found Ruby version {version} in Gemfile.lock"));

                Ok(Ruby::Lockfile(version))
            }
            LockfileRuby::VersionWithEngine(version, EngineVersion::JRuby(engine_version)) => {
                let version = RubyVersion(format!("{version}-jruby-{engine_version}"));

                user::log_info(format!("Found JRuby version {version} in Gemfile.lock"));

                Ok(Ruby::Lockfile(version))
            }
            LockfileRuby::EngineMissingRuby(EngineVersion::JRuby(engine)) => {
                Err(RubyVersionError::JRubyMissingRubyVersion(engine))
            }
            LockfileRuby::None => {
                let version = RubyVersion(DEFAULT_RUBY_VERSION.to_string());
                user::log_warning(
                    "No Ruby version detected",
                    formatdoc! {"
                    No version of Ruby was found in `Gemfile.lock`
                    Using default ruby version {version}
                "},
                );

                Ok(Ruby::Default(version))
            }
        }
    }
}

/// Install a given `Ruby` version
///
/// Returns an `Env` that must be applied.
///
/// ## Errors:
///
/// - On internal libcnb handling errors
/// - On `RubyInstalLayer` returned errors
pub(crate) fn download(
    ruby: &Ruby,
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
) -> libcnb::Result<Env, RubyBuildpackError> {
    match &ruby {
        Ruby::Lockfile(version) | Ruby::Default(version) => {
            let ruby_layer = context.handle_layer(
                layer_name!("ruby"),
                RubyInstallLayer {
                    version: version.clone(),
                },
            )?;

            let env = ruby_layer.env.apply(Scope::Build, env);
            Ok(env)
        }
        Ruby::System(_) => {
            user::log_info("Skipping Ruby installation. HEROKU_USE_SYSTEM_RUBY is enabled");
            Ok(env.clone())
        }
    }
}

/// Errors encountered while deriving and validating `RubyVersion`
#[derive(thiserror::Error, Debug)]
pub(crate) enum RubyVersionError {
    #[error("JRuby version {0} specified without Ruby version")]
    JRubyMissingRubyVersion(String),

    #[error("Failed {0}")]
    SystemRubyDetectionFailed(CommandError),
}

/// Holds Ruby version a user declared they want
/// us to install as a string.
///
/// Must not be used when `HEROKU_USE_SYTEM_RUBY` is being
/// used.
#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
pub(crate) struct RubyVersion(pub(crate) String);
impl Display for RubyVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Represents the current state of Ruby as a cache key value.
/// When Ruby version changes, this value must change.
///
/// The internal value will be used as a display for the user.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub(crate) struct RubyCacheKey(pub(crate) String);
impl Display for RubyCacheKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Represents either the ruby version the application desires
/// to install, or the ruby version that is being used due to
/// `HEROKU_USE_SYSTEM_RUBY`.
pub(crate) enum Ruby {
    /// Ruby version information from Gemfile.lock
    Lockfile(RubyVersion),
    /// HEROKU_USE_SYSTEM_RUBY is being used.
    ///
    /// Do not expose a `RubyVersion` since the buildpack is not being
    /// asked to install one.
    ///
    /// Holds a `RubyCacheKey` that represents the current system Ruby
    /// version and can be used for layer cache invalidation.
    System(RubyCacheKey),
    /// No Ruby version information found in the Gemfile.lock
    /// HEROKU_USE_SYSTEM_RUBY is not being used.
    Default(RubyVersion),
}

impl Ruby {
    /// No matter where the Ruby version came from, we must always be able
    /// to check if it changed (for layer cache invalidation).
    pub(crate) fn cache_key(&self) -> RubyCacheKey {
        match self {
            Ruby::Lockfile(RubyVersion(v)) | Ruby::Default(RubyVersion(v)) => {
                RubyCacheKey(v.to_string())
            }
            Ruby::System(key) => key.clone(),
        }
    }
}

impl Display for Ruby {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ruby::Lockfile(RubyVersion(s)) => f.write_str(s),
            Ruby::System(key) => write!(f, "{key}"),
            Ruby::Default(RubyVersion(key)) => f.write_str(key),
        }
    }
}

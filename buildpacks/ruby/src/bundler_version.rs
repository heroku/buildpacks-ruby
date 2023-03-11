use crate::gemfile_lock;
use crate::layers::BundleDownloadLayer;
use crate::{layers::BundleInstallLayer, ruby_version::RubyCacheKey};
use crate::{RubyBuildpack, RubyBuildpackError};
use libcnb::Env;
use libcnb::{build::BuildContext, data::layer_name, layer_env::Scope};
use libherokubuildpack::log as user;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// Used when no version is found in the Gemfile.lock
/// Default version is not sticky
const DEFAULT_BUNDLER_VERSION: &str = "2.4.8";

/// Bundler version info from the `Gemfile.lock`
pub(crate) fn from_lockfile(lockfile: &str) -> Bundler {
    if let Some(version) = gemfile_lock::bundled_with(lockfile) {
        user::log_info(format!(
            "Detected Bundler version {version} from Gemfile.lock"
        ));

        Bundler::Version(BundlerVersion(version))
    } else {
        let version = DEFAULT_BUNDLER_VERSION.to_string();

        Bundler::Default(BundlerVersion(version))
    }
}

/// Downloads the specified version of bundler to use later
/// in order to install dependencies.
pub(crate) fn download(
    bundler: Bundler,
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
) -> libcnb::Result<Env, RubyBuildpackError> {
    let mut env = env.clone();

    match bundler {
        Bundler::Version(version) | Bundler::Default(version) => {
            let download_bundler_layer = context.handle_layer(
                layer_name!("bundler"),
                BundleDownloadLayer {
                    version,
                    env: env.clone(),
                },
            )?;
            env = download_bundler_layer.env.apply(Scope::Build, &env);

            Ok(env)
        }
    }
}

/// Runs `bundle install` on the application
pub(crate) fn install_dependencies(
    context: &BuildContext<RubyBuildpack>,
    without: BundleWithout,
    ruby_version: RubyCacheKey,
    env: &Env,
) -> libcnb::Result<Env, RubyBuildpackError> {
    let bundle_install_layer = context.handle_layer(
        layer_name!("gems"),
        BundleInstallLayer {
            env: env.clone(),
            without,
            ruby_version,
        },
    )?;
    let env = bundle_install_layer.env.apply(Scope::Build, env);

    Ok(env)
}

/// Contains the colon `:` delimited string of environments
/// we don't want to install i.e. `"development:test"`
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct BundleWithout(pub(crate) String);
impl BundleWithout {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Holds Bundler version a user declared they want
/// us to install as a string.
#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
pub(crate) struct BundlerVersion(pub(crate) String);
impl Display for BundlerVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Represents the state of bundler that the application wants
/// to install.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Bundler {
    /// An explicit BUNDLED WITH was found in the Gemfile.lock
    Version(BundlerVersion),

    /// No explicit version found in Gemfile.lock
    Default(BundlerVersion),
}

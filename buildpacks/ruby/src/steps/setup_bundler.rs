use crate::layers::{BundleDownloadLayer, BundleEnvLayer, BundlePathLayer};
use crate::{RubyBuildpack, RubyBuildpackError};
use commons::gemfile_lock::{ResolvedBundlerVersion, ResolvedRubyVersion};
use libcnb::Env;
use libcnb::{build::BuildContext, data::layer_name, layer_env::Scope};

/// Primary interface for `bundle install`
pub(crate) fn setup_bundler(
    ruby_version: ResolvedRubyVersion,
    bundler_version: ResolvedBundlerVersion,
    without_default: String,
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
) -> libcnb::Result<Env, RubyBuildpackError> {
    let mut env = env.clone();

    // Gems will be installed here, sets BUNDLE_PATH env var
    let create_bundle_path_layer = context.handle_layer(
        layer_name!("gems"),
        BundlePathLayer {
            ruby_version: ruby_version.version,
        },
    )?;
    env = create_bundle_path_layer.env.apply(Scope::Build, &env);

    // Configures other `BUNDLE_*` settings not based on a layer path.
    let configure_env_layer = context.handle_layer(
        layer_name!("bundle_configure_env"),
        BundleEnvLayer { without_default },
    )?;
    env = configure_env_layer.env.apply(Scope::Build, &env);

    // Download the specified bundler version
    let download_bundler_layer = context.handle_layer(
        layer_name!("bundler"),
        BundleDownloadLayer {
            version: bundler_version,
            env: env.clone(),
        },
    )?;
    env = download_bundler_layer.env.apply(Scope::Build, &env);

    Ok(env)
}

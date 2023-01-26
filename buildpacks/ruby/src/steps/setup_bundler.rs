use crate::layers::{BundleDownloadLayer, GemsPathLayer};
use crate::{RubyBuildpack, RubyBuildpackError};
use commons::gemfile_lock::{ResolvedBundlerVersion, ResolvedRubyVersion};
use commons::layer::ConfigureEnvLayer;
use libcnb::layer_env::{LayerEnv, ModificationBehavior};
use libcnb::Env;
use libcnb::{build::BuildContext, data::layer_name, layer_env::Scope};

/// Primary interface for `bundle install`
pub(crate) fn setup_bundler(
    ruby_version: ResolvedRubyVersion,
    bundler_version: ResolvedBundlerVersion,
    without_default: &str,
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
) -> libcnb::Result<Env, RubyBuildpackError> {
    let mut env = env.clone();

    // Gems will be installed here, sets BUNDLE_PATH env var
    let create_bundle_path_layer = context.handle_layer(
        layer_name!("gems"),
        GemsPathLayer {
            ruby_version: ruby_version.version,
        },
    )?;
    env = create_bundle_path_layer.env.apply(Scope::Build, &env);

    // Configures other `BUNDLE_*` settings not based on a layer path.
    let configure_env_layer = context.handle_layer(
        layer_name!("bundle_configure_env"),
        ConfigureEnvLayer::new(
            LayerEnv::new()
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Default,
                    "BUNDLE_WITHOUT", // Do not install `development` or `test` groups via bundle install. Additional groups can be specified via user config.
                    without_default,
                )
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Override,
                    "BUNDLE_GEMFILE", // Tells bundler where to find the `Gemfile`
                    context.app_dir.join("Gemfile"),
                )
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Override,
                    "BUNDLE_CLEAN", // After successful `bundle install` bundler will automatically run `bundle clean`
                    "1",
                )
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Override,
                    "BUNDLE_DEPLOYMENT", // Requires the `Gemfile.lock` to be in sync with the current `Gemfile`.
                    "1",
                ),
        ),
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

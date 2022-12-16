use crate::layers::{BundleDownloadLayer, BundleEnvLayer, BundlePathLayer};
use crate::{RubyBuildpack, RubyBuildpackError};
use commons::env_command::EnvCommand;
use commons::gemfile_lock::{ResolvedBundlerVersion, ResolvedRubyVersion};
use libcnb::Env;
use libcnb::{build::BuildContext, data::layer_name, layer_env::Scope};

/// Primary interface for `bundle install`
pub(crate) fn bundle_install(
    ruby_version: ResolvedRubyVersion,
    bundler_version: ResolvedBundlerVersion,
    without_default: String,
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
) -> libcnb::Result<Env, RubyBuildpackError> {
    let mut env = env.clone();
    // ## Setup bundler
    //
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

    // ## Download bundler
    //
    // Download the specified bundler version
    let download_bundler_layer = context.handle_layer(
        layer_name!("bundler"),
        BundleDownloadLayer {
            version: bundler_version,
            env: env.clone(),
        },
    )?;
    env = download_bundler_layer.env.apply(Scope::Build, &env);

    // ## Run `$ bundle install`
    println!("---> Installing gems");
    let command = EnvCommand::new_show_keys(
        "bundle",
        &["install"],
        &env,
        [
            "BUNDLE_BIN",
            "BUNDLE_CLEAN",
            "BUNDLE_DEPLOYMENT",
            "BUNDLE_GEMFILE",
            "BUNDLE_PATH",
            "BUNDLE_WITHOUT",
        ],
    );

    println!("Running: $ {command}");

    command
        .stream()
        .map_err(RubyBuildpackError::BundleInstallCommandError)?;

    Ok(env)
}

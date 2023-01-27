use crate::layers::BundleDownloadLayer;
use crate::{RubyBuildpack, RubyBuildpackError};
use commons::gemfile_lock::ResolvedBundlerVersion;
use libcnb::Env;
use libcnb::{build::BuildContext, data::layer_name, layer_env::Scope};

/// Primary interface for `bundle install`
pub(crate) fn bundler_download(
    bundler_version: ResolvedBundlerVersion,
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
) -> libcnb::Result<Env, RubyBuildpackError> {
    let mut env = env.clone();

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

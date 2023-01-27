use crate::{layers::BundleInstallLayer, BundleWithout, RubyBuildpack, RubyBuildpackError};
use commons::gemfile_lock::ResolvedRubyVersion;
use libcnb::{build::BuildContext, data::layer_name, layer_env::Scope, Env};

pub(crate) fn bundle_install(
    context: &BuildContext<RubyBuildpack>,
    without: BundleWithout,
    ruby_version: ResolvedRubyVersion,
    env: &Env,
) -> libcnb::Result<Env, RubyBuildpackError> {
    // Gems will be installed here, sets BUNDLE_PATH env var
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

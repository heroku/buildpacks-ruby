use libcnb::Env;
use libcnb::{build::BuildContext, data::layer_name, layer_env::Scope};

use crate::layers::BundleInstallConfigureEnvLayer;
use crate::lib::{BundlerVersion, RubyVersion};
use crate::{
    layers::{
        BundleInstallCreatePathLayer, BundleInstallDownloadBundlerLayer, BundleInstallExecuteLayer,
    },
    RubyBuildpack, RubyBuildpackError,
};

pub struct BundleInstall;

impl BundleInstall {
    pub fn call(
        ruby_version: RubyVersion,
        bundler_version: BundlerVersion,
        context: &BuildContext<RubyBuildpack>,
        env: &Env,
    ) -> Result<Env, RubyBuildpackError> {
        let env = env.clone();
        // ## Setup bundler
        let create_bundle_path_layer = context.handle_layer(
            layer_name!("gems"),
            BundleInstallCreatePathLayer {
                ruby_version: ruby_version.version_string(), // ruby_layer.content_metadata.metadata.version,
            },
        )?;
        env = create_bundle_path_layer.env.apply(Scope::Build, &env);

        let create_bundle_path_layer = context.handle_layer(
            layer_name!("bundle_configure_env"),
            BundleInstallConfigureEnvLayer,
        )?;
        env = create_bundle_path_layer.env.apply(Scope::Build, &env);

        // ## Download bundler
        let download_bundler_layer = context.handle_layer(
            layer_name!("bundler"),
            BundleInstallDownloadBundlerLayer {
                version: bundler_version.version_string(), // bundle_info.bundler_version,
                env: env.clone(),
            },
        )?;
        env = download_bundler_layer.env.apply(Scope::Build, &env);

        // ## bundle install
        let execute_bundle_install_layer = context.handle_layer(
            layer_name!("execute_bundle_install"),
            BundleInstallExecuteLayer { env: env.clone() },
        )?;
        env = execute_bundle_install_layer.env.apply(Scope::Build, &env);

        Ok(env)
    }
}

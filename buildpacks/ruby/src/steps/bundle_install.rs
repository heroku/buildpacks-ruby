use crate::layers::{BundleInstallConfigureEnvLayer, BundleInstallDownloadBundlerLayer};
use crate::{layers::BundleInstallCreatePathLayer, RubyBuildpack, RubyBuildpackError};
use commons::env_command::EnvCommand;
use commons::gemfile_lock::{ResolvedBundlerVersion, ResolvedRubyVersion};
use libcnb::Env;
use libcnb::{build::BuildContext, data::layer_name, layer_env::Scope};

pub struct BundleInstall;

impl BundleInstall {
    pub fn call(
        ruby_version: ResolvedRubyVersion,
        bundler_version: ResolvedBundlerVersion,
        context: &BuildContext<RubyBuildpack>,
        env: &Env,
    ) -> libcnb::Result<Env, RubyBuildpackError> {
        let mut env = env.clone();
        // ## Setup bundler
        //
        // Gems will be installed here, sets BUNDLE_PATH env var
        let create_bundle_path_layer = context.handle_layer(
            layer_name!("gems"),
            BundleInstallCreatePathLayer {
                ruby_version: ruby_version.version,
            },
        )?;
        env = create_bundle_path_layer.env.apply(Scope::Build, &env);

        // Configures other `BUNDLE_*` settings not based on a layer path.
        let configure_env_layer = context.handle_layer(
            layer_name!("bundle_configure_env"),
            BundleInstallConfigureEnvLayer,
        )?;
        env = configure_env_layer.env.apply(Scope::Build, &env);

        // ## Download bundler
        //
        // Download the specified bundler version
        let download_bundler_layer = context.handle_layer(
            layer_name!("bundler"),
            BundleInstallDownloadBundlerLayer {
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

        println!("Running: $ {} ", command);

        command
            .stream()
            .map_err(RubyBuildpackError::BundleInstallCommandError)?;

        Ok(env)
    }
}

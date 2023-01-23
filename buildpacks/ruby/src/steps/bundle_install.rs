use crate::RubyBuildpackError;
use commons::env_command::EnvCommand;
use libcnb::Env;
use libherokubuildpack::log as user;

pub(crate) fn bundle_install(env: &Env) -> libcnb::Result<Env, RubyBuildpackError> {
    // ## Run `$ bundle install`
    let command = EnvCommand::new_show_keys(
        "bundle",
        &["install"],
        env,
        [
            "BUNDLE_BIN",
            "BUNDLE_CLEAN",
            "BUNDLE_DEPLOYMENT",
            "BUNDLE_GEMFILE",
            "BUNDLE_PATH",
            "BUNDLE_WITHOUT",
        ],
    );

    user::log_info(format!("$ {command}"));

    command
        .stream()
        .map_err(RubyBuildpackError::BundleInstallCommandError)?;

    Ok(env.clone())
}

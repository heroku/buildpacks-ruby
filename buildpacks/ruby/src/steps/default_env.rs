use crate::{
    layers::{EnvDefaultsLayer, EnvSecretKeyBaseLayer},
    RubyBuildpack, RubyBuildpackError,
};
use libcnb::{
    build::BuildContext,
    data::{layer_name, store::Store},
    layer_env::Scope,
    Env,
};
use rand::Rng;

// Set default environment values
pub(crate) fn default_env(
    context: &BuildContext<RubyBuildpack>,
    platform_env: &Env,
) -> libcnb::Result<(Env, Store), RubyBuildpackError> {
    // Get system env vars
    let mut env = Env::from_current();

    // Apply User env vars
    // TODO reject harmful vars like GEM_PATH
    for (k, v) in platform_env {
        env.insert(k, v);
    }

    let mut metadata = match &context.store {
        Some(store) => store.metadata.clone(),
        None => toml::value::Table::default(),
    };

    let default_secret_key_base = metadata
        .entry("SECRET_KEY_BASE")
        .or_insert_with(|| {
            let mut rng = rand::thread_rng();

            (0..64)
                .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
                .collect::<String>()
                .into()
        })
        .to_string();

    let store = Store { metadata };

    // Setup default environment variables
    let secret_key_base_layer = context //
        .handle_layer(
            layer_name!("secret_key_base"),
            EnvSecretKeyBaseLayer {
                default_value: default_secret_key_base,
            },
        )?;
    env = secret_key_base_layer.env.apply(Scope::Build, &env);

    let env_defaults_layer = context //
        .handle_layer(layer_name!("env_defaults"), EnvDefaultsLayer)?;
    env = env_defaults_layer.env.apply(Scope::Build, &env);

    Ok((env, store))
}

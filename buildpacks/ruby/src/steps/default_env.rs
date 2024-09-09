use crate::{RubyBuildpack, RubyBuildpackError};
use libcnb::layer::UncachedLayerDefinition;
use libcnb::layer_env::{LayerEnv, ModificationBehavior};
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

    let (default_secret_key_base, store) = fetch_secret_key_base_from_store(&context.store);
    let env_defaults_layer = context.uncached_layer(
        layer_name!("env_default"),
        UncachedLayerDefinition {
            build: true,
            launch: true,
        },
    )?;

    let mut layer_env = LayerEnv::new();
    for (key, value) in [
        ("SECRET_KEY_BASE", default_secret_key_base.as_str()),
        ("JRUBY_OPTS", "-Xcompile.invokedynamic=false"),
        ("RACK_ENV", "production"),
        ("RAILS_ENV", "production"),
        ("RAILS_SERVE_STATIC_FILES", "enabled"),
        ("RAILS_LOG_TO_STDOUT", "enabled"),
        ("MALLOC_ARENA_MAX", "2"),
        ("DISABLE_SPRING", "1"),
    ] {
        layer_env =
            layer_env.chainable_insert(Scope::All, ModificationBehavior::Default, key, value);
    }

    env_defaults_layer.write_env(&layer_env)?;

    let env = env_defaults_layer.read_env()?.apply(Scope::Build, &env);

    Ok((env, store))
}

fn fetch_secret_key_base_from_store(store: &Option<Store>) -> (String, Store) {
    let mut store = store.clone().unwrap_or_default();
    let default_secret_key_base = store
        .metadata
        .entry("SECRET_KEY_BASE")
        .or_insert_with(|| {
            let mut rng = rand::thread_rng();

            (0..64)
                .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
                .collect::<String>()
                .into()
        })
        .to_string();

    (default_secret_key_base, store)
}

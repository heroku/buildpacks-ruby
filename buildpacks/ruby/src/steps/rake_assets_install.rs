use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use byte_unit::Byte;
use commons::app_cache_collection::AppCacheCollection;
use commons::app_cache_collection::CacheConfig;
use commons::app_cache_collection::KeepAppPath;
use commons::env_command::CommandError;
use commons::env_command::EnvCommand;
use commons::rake_task_detect::RakeDetect;
use libcnb::build::BuildContext;
use libcnb::Env;
use libherokubuildpack::log as user;

pub(crate) fn rake_assets_install(
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
    rake_detect: &RakeDetect,
) -> Result<(), RubyBuildpackError> {
    let cases = asset_cases(rake_detect);
    match cases {
        AssetCases::None => {
            user::log_info("Skipping 'rake assets:precompile', task not found");
            user::log_info("Help: Ensure `bundle exec rake -P` includes this task");
        }
        AssetCases::PrecompileOnly => {
            user::log_info("Running 'rake assets:precompile', task found");
            user::log_info("Skipping 'rake assets:clean', task not found");
            user::log_info("Help: Ensure `bundle exec rake -P` includes this task");

            run_rake_assets_precompile(env)
                .map_err(RubyBuildpackError::RakeAssetsPrecompileFailed)?;
        }
        AssetCases::PrecompileAndClean => {
            user::log_info("Running 'rake assets:precompile', task found");
            user::log_info("Running 'rake assets:clean', task found");

            let cache_config = [
                CacheConfig {
                    path: context.app_dir.join("public").join("assets"),
                    limit: Byte::from_bytes(byte_unit::n_mib_bytes!(100)),
                    keep_app_path: KeepAppPath::Runtime,
                },
                CacheConfig {
                    path: context.app_dir.join("tmp").join("cache").join("assets"),
                    limit: Byte::from_bytes(byte_unit::n_mib_bytes!(100)),
                    keep_app_path: KeepAppPath::BuildOnly,
                },
            ];

            let cache =
                AppCacheCollection::new_and_load(context, cache_config, |log| user::log_info(log))
                    .map_err(RubyBuildpackError::InAppDirCacheError)?;

            run_rake_assets_precompile_with_clean(env)
                .map_err(RubyBuildpackError::RakeAssetsPrecompileFailed)?;

            cache
                .store()
                .map_err(RubyBuildpackError::InAppDirCacheError)?;
        }
    }

    Ok(())
}

fn run_rake_assets_precompile(env: &Env) -> Result<(), CommandError> {
    let command = EnvCommand::new(
        "bundle",
        &["exec", "rake", "assets:precompile", "--trace"],
        env,
    );
    user::log_info("$ {command}");

    command.stream()?;

    Ok(())
}

fn run_rake_assets_precompile_with_clean(env: &Env) -> Result<(), CommandError> {
    let command = EnvCommand::new(
        "bundle",
        &[
            "exec",
            "rake",
            "assets:precompile",
            "assets:clean",
            "--trace",
        ],
        env,
    );

    user::log_info("$ {command}");

    command.stream()?;

    Ok(())
}

#[derive(Clone, Debug)]
enum AssetCases {
    None,
    PrecompileOnly,
    PrecompileAndClean,
}

fn asset_cases(rake: &RakeDetect) -> AssetCases {
    if !rake.has_task("assets:precompile") {
        AssetCases::None
    } else if rake.has_task("assets:clean") {
        AssetCases::PrecompileAndClean
    } else {
        AssetCases::PrecompileOnly
    }
}

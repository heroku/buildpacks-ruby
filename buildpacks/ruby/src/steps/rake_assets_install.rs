use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use commons::cache::{mib, AppCacheCollection, CacheConfig, KeepPath};
use commons::fun_run::{self, CmdError, CmdMapExt};
use commons::rake_task_detect::RakeDetect;
use libcnb::build::BuildContext;
use libcnb::Env;
use libherokubuildpack::command::CommandExt;
use libherokubuildpack::log as user;
use std::process::Command;

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
                    limit: mib(100),
                    keep_path: KeepPath::Runtime,
                },
                CacheConfig {
                    path: context.app_dir.join("tmp").join("cache").join("assets"),
                    limit: mib(100),
                    keep_path: KeepPath::BuildOnly,
                },
            ];

            let cache =
                AppCacheCollection::new_and_load(context, cache_config, |log| user::log_info(log))
                    .map_err(RubyBuildpackError::InAppDirCacheError)?;

            run_rake_assets_precompile_with_clean(env)
                .map_err(RubyBuildpackError::RakeAssetsPrecompileFailed)?;

            cache
                .save_and_clean()
                .map_err(RubyBuildpackError::InAppDirCacheError)?;
        }
    }

    Ok(())
}

fn run_rake_assets_precompile(env: &Env) -> Result<(), CmdError> {
    Command::new("bundle")
        .args(["exec", "rake", "assets:precompile", "--trace"])
        .env_clear()
        .envs(env)
        .cmd_map(|cmd| {
            let name = fun_run::display(cmd);
            user::log_info(format!("Running  $ {name}"));

            cmd.output_and_write_streams(std::io::stdout(), std::io::stderr())
                .map_err(|error| {
                    fun_run::annotate_which_problem(error, cmd, env.get("PATH").cloned())
                })
                .map_err(|error| fun_run::on_system_error(name.clone(), error))
                .and_then(|output| fun_run::nonzero_streamed(name.clone(), output))
        })?;

    Ok(())
}

fn run_rake_assets_precompile_with_clean(env: &Env) -> Result<(), CmdError> {
    Command::new("bundle")
        .args([
            "exec",
            "rake",
            "assets:precompile",
            "assets:clean",
            "--trace",
        ])
        .env_clear()
        .envs(env)
        .cmd_map(|cmd| {
            let name = fun_run::display(cmd);
            user::log_info(format!("Running  $ {name}"));

            cmd.output_and_write_streams(std::io::stdout(), std::io::stderr())
                .map_err(|error| {
                    fun_run::annotate_which_problem(error, cmd, env.get("PATH").cloned())
                })
                .map_err(|error| fun_run::on_system_error(name.clone(), error))
                .and_then(|output| fun_run::nonzero_streamed(name.clone(), output))
        })?;

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

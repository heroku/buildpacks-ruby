use crate::build_output;
use crate::build_output::section::RunCommand;
use crate::build_output::section::Section;
use crate::rake_task_detect::RakeDetect;
use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use commons::cache::{mib, AppCacheCollection, CacheConfig, KeepPath};
use commons::fun_run::{self, CmdError, CmdMapExt};
use libcnb::build::BuildContext;
use libcnb::Env;
use std::process::Command;

pub(crate) fn rake_assets_install(
    section: &Section,
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
    rake_detect: &RakeDetect,
) -> Result<(), RubyBuildpackError> {
    let cases = asset_cases(rake_detect);
    let rake_assets_precompile = build_output::fmt::value("rake assets:precompile");
    let rake_assets_clean = build_output::fmt::value("rake assets:clean");
    let rake_detect_cmd = build_output::fmt::value("bundle exec rake -P");

    match cases {
        AssetCases::None => {
            section.say_with_details(
                format!("Skipping {rake_assets_precompile}"),
                format!("task not found via {rake_detect_cmd}"),
            );
            section.help("Enable compiling assets by ensuring that task is present when running the detect command locally");
        }
        AssetCases::PrecompileOnly => {
            section.say_with_details(
                "Compiling assets without cache",
                format!("clean task not found via {rake_detect_cmd}"),
            );
            section.help(format!("Enable caching by ensuring {rake_assets_clean} is present when running the detect command locally"));

            run_rake_assets_precompile(section, env)
                .map_err(RubyBuildpackError::RakeAssetsPrecompileFailed)?;
        }
        AssetCases::PrecompileAndClean => {
            section.say_with_details(
                "Compiling assets with cache",
                format!(
                "detected {rake_assets_precompile} and {rake_assets_clean} via {rake_detect_cmd}"
                ),
            );

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

            let cache = {
                let section = section.clone();
                AppCacheCollection::new_and_load(context, cache_config, move |log| section.say(log))
                    .map_err(RubyBuildpackError::InAppDirCacheError)?
            };

            run_rake_assets_precompile_with_clean(section, env)
                .map_err(RubyBuildpackError::RakeAssetsPrecompileFailed)?;

            cache
                .save_and_clean()
                .map_err(RubyBuildpackError::InAppDirCacheError)?;
        }
    }

    Ok(())
}

fn run_rake_assets_precompile(section: &Section, env: &Env) -> Result<(), CmdError> {
    Command::new("bundle")
        .args(["exec", "rake", "assets:precompile", "--trace"])
        .env_clear()
        .envs(env)
        .cmd_map(|cmd| {
            let path_env = env.get("PATH").cloned();
            section
                .run(RunCommand::stream(cmd))
                .map_err(|error| fun_run::map_which_problem(error, cmd, path_env))
        })?;

    Ok(())
}

fn run_rake_assets_precompile_with_clean(section: &Section, env: &Env) -> Result<(), CmdError> {
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
            let path_env = env.get("PATH").cloned();
            section
                .run(RunCommand::stream(cmd))
                .map_err(|error| fun_run::map_which_problem(error, cmd, path_env))
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

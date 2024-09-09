use crate::rake_task_detect::RakeDetect;
use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use bullet_stream::state::SubBullet;
use bullet_stream::{style, Print};
use commons::cache::{mib, AppCache, CacheConfig, CacheError, CacheState, KeepPath, PathState};
use fun_run::{self, CommandWithName};
use libcnb::build::BuildContext;
use libcnb::Env;
use std::io::Stdout;
use std::process::Command;

pub(crate) fn rake_assets_install(
    mut bullet: Print<SubBullet<Stdout>>,
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
    rake_detect: &RakeDetect,
) -> Result<Print<SubBullet<Stdout>>, RubyBuildpackError> {
    let cases = asset_cases(rake_detect);
    let rake_assets_precompile = style::value("rake assets:precompile");
    let rake_assets_clean = style::value("rake assets:clean");
    let rake_detect_cmd = style::value("bundle exec rake -P");
    let help = style::important("HELP");

    match cases {
        AssetCases::None => {
            bullet = bullet.sub_bullet(format!(
                "Skipping {rake_assets_precompile} (task not found via {rake_detect_cmd})"
            ))
            .sub_bullet("{help} Enable compiling assets by ensuring {rake_assets_precompile} is present when running the detect command locally");
        }
        AssetCases::PrecompileOnly => {
            bullet = bullet.sub_bullet(format!("Compiling assets without cache (Clean task not found via {rake_detect_cmd})"))
                .sub_bullet(format!("{help} Enable caching by ensuring {rake_assets_clean} is present when running the detect command locally"));

            let path_env = env.get("PATH").cloned();
            let mut cmd = Command::new("bundle");
            cmd.args(["exec", "rake", "assets:precompile", "--trace"])
                .env_clear()
                .envs(env);
            bullet
                .stream_with(
                    format!("Running {}", style::command(cmd.name())),
                    |stdout, stderr| {
                        cmd.stream_output(stdout, stderr).map_err(|error| {
                            fun_run::map_which_problem(error, &mut cmd, path_env.clone())
                        })
                    },
                )
                .map_err(RubyBuildpackError::RakeAssetsPrecompileFailed)?;
        }
        AssetCases::PrecompileAndClean => {
            bullet = bullet.sub_bullet(format!("Compiling assets with cache (detected {rake_assets_precompile} and {rake_assets_clean} via {rake_detect_cmd})"));

            let caches = [
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
            ]
            .into_iter()
            .map(|config| AppCache::new_and_load(context, config))
            .collect::<Result<Vec<AppCache>, CacheError>>()
            .map_err(RubyBuildpackError::InAppDirCacheError)?;

            for store in &caches {
                let path = store.path().display();
                bullet = bullet.sub_bullet(match store.cache_state() {
                    CacheState::NewEmpty => format!("Creating cache for {path}"),
                    CacheState::ExistsEmpty => format!("Loading (empty) cache for {path}"),
                    CacheState::ExistsWithContents => format!("Loading cache for {path}"),
                });
            }

            let path_env = env.get("PATH").cloned();
            let mut cmd = Command::new("bundle");
            cmd.args([
                "exec",
                "rake",
                "assets:precompile",
                "assets:clean",
                "--trace",
            ])
            .env_clear()
            .envs(env);

            bullet
                .stream_with(
                    format!("Running {}", style::command(cmd.name())),
                    |stdout, stderr| cmd.stream_output(stdout, stderr),
                )
                .map_err(|error| fun_run::map_which_problem(error, &mut cmd, path_env))
                .map_err(RubyBuildpackError::RakeAssetsPrecompileFailed)?;

            for store in &caches {
                let path = store.path().display();

                bullet = bullet.sub_bullet(match store.path_state() {
                    PathState::Empty => format!("Storing cache for (empty) {path}"),
                    PathState::HasFiles => format!("Storing cache for {path}"),
                });

                if let Some(removed) = store
                    .save_and_clean()
                    .map_err(RubyBuildpackError::InAppDirCacheError)?
                {
                    let path = store.path().display();
                    let limit = store.limit();
                    let removed_len = removed.files.len();
                    let removed_size = removed.adjusted_bytes();

                    bullet = bullet.sub_bullet(format!(
                        "Detected cache size exceeded (over {limit} limit by {removed_size}) for {path}"
                    )).sub_bullet(format!(
                        "Removed {removed_len} files from the cache for {path}",
                    ));
                }
            }
        }
    }

    Ok(bullet)
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

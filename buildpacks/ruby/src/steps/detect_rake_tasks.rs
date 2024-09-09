use std::io::Stdout;

use bullet_stream::state::SubBullet;
use bullet_stream::{style, Print};

use crate::gem_list::GemList;
use crate::rake_status::{check_rake_ready, RakeStatus};
use crate::rake_task_detect::RakeDetect;
use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use libcnb::build::BuildContext;
use libcnb::Env;

pub(crate) fn detect_rake_tasks(
    mut bullet: Print<SubBullet<Stdout>>,
    gem_list: &GemList,
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
) -> Result<(Print<SubBullet<Stdout>>, Option<RakeDetect>), RubyBuildpackError> {
    let rake = style::value("rake");
    let gemfile = style::value("Gemfile");
    let rakefile = style::value("Rakefile");
    let help = style::important("HELP");

    match check_rake_ready(
        &context.app_dir,
        gem_list,
        [".sprockets-manifest-*.json", "manifest-*.json"],
    ) {
        RakeStatus::MissingRakeGem => {
            bullet = bullet
                .sub_bullet(format!("Skipping rake tasks ({rake} gem not found)"))
                .sub_bullet(format!(
                    "{help} Add {gem} to your {gemfile} to enable",
                    gem = style::value("gem 'rake'")
                ));

            Ok((bullet, None))
        }
        RakeStatus::MissingRakefile => {
            bullet = bullet
                .sub_bullet(format!("Skipping rake tasks (no {rakefile} found)"))
                .sub_bullet(format!("{help} Add {rakefile} to your project to enable"));

            Ok((bullet, None))
        }
        RakeStatus::SkipManifestFound(paths) => {
            let files = paths
                .iter()
                .map(|path| style::value(path.to_string_lossy()))
                .collect::<Vec<_>>()
                .join(", ");

            bullet = bullet
                .sub_bullet(format!(
                    "Skipping rake tasks (Manifest files found {files})"
                ))
                .sub_bullet(format!("{help} Delete files to enable running rake tasks"));

            Ok((bullet, None))
        }
        RakeStatus::Ready(path) => {
            bullet = bullet.sub_bullet(format!(
                "Detected rake ({rake} gem found, {rakefile} found at {path})",
                path = style::value(path.to_string_lossy())
            ));

            let (bullet, rake_detect) = RakeDetect::from_rake_command(bullet, env, true)
                .map_err(RubyBuildpackError::RakeDetectError)?;

            Ok((bullet, Some(rake_detect)))
        }
    }
}

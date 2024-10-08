use std::io::Stdout;

use bullet_stream::state::SubBullet;
use bullet_stream::{style, Print};
use commons::output::{
    fmt::{self, HELP},
    section_log::log_step,
};

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
    let help = style::important("HELP");
    let rake = fmt::value("rake");
    let gemfile = fmt::value("Gemfile");
    let rakefile = fmt::value("Rakefile");

    match check_rake_ready(
        &context.app_dir,
        gem_list,
        [".sprockets-manifest-*.json", "manifest-*.json"],
    ) {
        RakeStatus::MissingRakeGem => Ok((
            bullet
                .sub_bullet(format!(
                    "Skipping rake tasks ({rake} gem not found in {gemfile})"
                ))
                .sub_bullet(format!(
                    "{help} Add {gem} to your {gemfile} to enable",
                    gem = fmt::value("gem 'rake'")
                )),
            None,
        )),
        RakeStatus::MissingRakefile => Ok((
            bullet
                .sub_bullet(format!("Skipping rake tasks ({rakefile} not found)",))
                .sub_bullet(format!("{help} Add {rakefile} to your project to enable",)),
            None,
        )),
        RakeStatus::SkipManifestFound(paths) => {
            let manifest_files = paths
                .iter()
                .map(|path| fmt::value(path.to_string_lossy()))
                .collect::<Vec<_>>()
                .join(", ");

            Ok((
                bullet
                    .sub_bullet(format!(
                        "Skipping rake tasks (Manifest {files} found {manifest_files})",
                        files = if manifest_files.len() > 1 {
                            "files"
                        } else {
                            "file"
                        }
                    ))
                    .sub_bullet(format!("{help} Delete files to enable running rake tasks")),
                None,
            ))
        }
        RakeStatus::Ready(path) => {
            log_step(format!(
                "Detected rake ({rake} gem found, {rakefile} found at {path})",
                path = fmt::value(path.to_string_lossy())
            ));

            let rake_detect = RakeDetect::from_rake_command(env, true)
                .map_err(RubyBuildpackError::RakeDetectError)?;

            Ok((bullet, Some(rake_detect)))
        }
    }
}

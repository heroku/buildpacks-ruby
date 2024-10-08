use std::io::Stdout;

use bullet_stream::state::SubBullet;
use bullet_stream::Print;
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
    let rake = fmt::value("rake");
    let gemfile = fmt::value("Gemfile");
    let rakefile = fmt::value("Rakefile");

    match check_rake_ready(
        &context.app_dir,
        gem_list,
        [".sprockets-manifest-*.json", "manifest-*.json"],
    ) {
        RakeStatus::MissingRakeGem => {
            log_step(format!(
                "Skipping rake tasks {}",
                fmt::details(format!("no {rake} gem in {gemfile}"))
            ));

            log_step(format!(
                "{HELP} Add {gem} to your {gemfile} to enable",
                gem = fmt::value("gem 'rake'")
            ));

            Ok((bullet, None))
        }
        RakeStatus::MissingRakefile => {
            log_step(format!(
                "Skipping rake tasks {}",
                fmt::details(format!("no {rakefile}"))
            ));
            log_step(format!("{HELP} Add {rakefile} to your project to enable",));

            Ok((bullet, None))
        }
        RakeStatus::SkipManifestFound(paths) => {
            let files = paths
                .iter()
                .map(|path| fmt::value(path.to_string_lossy()))
                .collect::<Vec<_>>()
                .join(", ");

            log_step(format!(
                "Skipping rake tasks {}",
                fmt::details(format!("Manifest files found {files}"))
            ));
            log_step(format!("{HELP} Delete files to enable running rake tasks"));

            Ok((bullet, None))
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

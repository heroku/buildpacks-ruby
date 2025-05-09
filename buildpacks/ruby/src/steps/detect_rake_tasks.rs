use crate::gem_list::GemList;
use crate::rake_status::{check_rake_ready, RakeStatus};
use crate::rake_task_detect::{self, RakeDetect};
use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use bullet_stream::{global::print, style};
use libcnb::build::BuildContext;
use libcnb::Env;

pub(crate) fn detect_rake_tasks(
    gem_list: &GemList,
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
) -> Result<Option<RakeDetect>, RubyBuildpackError> {
    let help = style::important("HELP");
    let rake = style::value("rake");
    let gemfile = style::value("Gemfile");
    let rakefile = style::value("Rakefile");

    let detect = match check_rake_ready(
        &context.app_dir,
        gem_list,
        [".sprockets-manifest-*.json", "manifest-*.json"],
    ) {
        RakeStatus::MissingRakeGem => {
            print::sub_bullet(format!(
                "Skipping rake tasks ({rake} gem not found in {gemfile})"
            ));
            print::sub_bullet(format!(
                "{help} Add {gem} to your {gemfile} to enable",
                gem = style::value("gem 'rake'")
            ));
            None
        }
        RakeStatus::MissingRakefile => {
            print::sub_bullet(format!("Skipping rake tasks ({rakefile} not found)",));
            print::sub_bullet(format!("{help} Add {rakefile} to your project to enable",));
            None
        }
        RakeStatus::SkipManifestFound(paths) => {
            let manifest_files = paths
                .iter()
                .map(|path| style::value(path.to_string_lossy()))
                .collect::<Vec<_>>()
                .join(", ");

            print::sub_bullet(format!(
                "Skipping rake tasks (Manifest {files} found {manifest_files})",
                files = if manifest_files.len() > 1 {
                    "files"
                } else {
                    "file"
                }
            ));
            print::sub_bullet(format!("{help} Delete files to enable running rake tasks"));
            None
        }
        RakeStatus::Ready(path) => {
            print::sub_bullet(format!(
                "Detected rake ({rake} gem found, {rakefile} found at {path})",
                path = style::value(path.to_string_lossy())
            ));
            let rake_detect =
                rake_task_detect::call(env, true).map_err(RubyBuildpackError::RakeDetectError)?;

            Some(rake_detect)
        }
    };
    Ok(detect)
}

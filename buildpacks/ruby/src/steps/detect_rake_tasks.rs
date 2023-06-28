use crate::build_output::section::Section;
use crate::gem_list::GemList;
use crate::rake_status::{check_rake_ready, RakeStatus};
use crate::rake_task_detect::RakeDetect;
use crate::RubyBuildpackError;
use crate::{build_output, RubyBuildpack};
use libcnb::build::BuildContext;
use libcnb::Env;

pub(crate) fn detect_rake_tasks(
    section: &Section,
    gem_list: &GemList,
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
) -> Result<Option<RakeDetect>, RubyBuildpackError> {
    let rake = build_output::fmt::value("rake");
    let gemfile = build_output::fmt::value("Gemfile");
    let rakefile = build_output::fmt::value("Rakefile");

    match check_rake_ready(
        &context.app_dir,
        gem_list,
        [".sprockets-manifest-*.json", "manifest-*.json"],
    ) {
        RakeStatus::MissingRakeGem => {
            section.say_with_details(
                "Cannot run rake tasks",
                format!("no {rake} gem in {gemfile}"),
            );

            let gem = build_output::fmt::value("gem 'rake'");
            section.help(format!("Add {gem} to your {gemfile} to enable"));

            Ok(None)
        }
        RakeStatus::MissingRakefile => {
            section.say_with_details("Cannot run rake tasks", format!("no {rakefile}"));
            section.help(format!("Add {rakefile} to your project to enable"));

            Ok(None)
        }
        RakeStatus::SkipManifestFound(paths) => {
            let files = paths
                .iter()
                .map(|path| build_output::fmt::value(path.to_string_lossy()))
                .collect::<Vec<_>>()
                .join(", ");
            section.say_with_details(
                "Skipping rake tasks",
                format!("Manifest files found {files}"),
            );
            section.help("Delete files to enable running rake tasks");

            Ok(None)
        }
        RakeStatus::Ready(path) => {
            let path = build_output::fmt::value(path.to_string_lossy());
            section.say_with_details(
                "Rake detected",
                format!("{rake} gem found, {rakefile} found at {path}"),
            );

            let rake_detect = RakeDetect::from_rake_command(section, env, true)
                .map_err(RubyBuildpackError::RakeDetectError)?;

            Ok(Some(rake_detect))
        }
    }
}

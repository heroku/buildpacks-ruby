use std::ffi::OsString;

use crate::build_output::{self, Section};
use crate::bundle_list::GemList;
use crate::rake_status::{check_rake_ready, RakeStatus};
use crate::rake_task_detect::RakeDetect;
use crate::RubyBuildpack;
use crate::RubyBuildpackError;
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

            // Add RAILS_ENV=production to encourage people trying to reproduce the issue
            // locally to use the same env environment.
            let highlight_envs = {
                let mut list = Vec::new();
                if gem_list.has("railties") {
                    list.push(OsString::from("RAILS_ENV"));
                }
                list
            };

            let rake_detect = RakeDetect::from_rake_command(section, env, true, &highlight_envs)
                .map_err(RubyBuildpackError::CannotDetectRakeTasks)?;

            Ok(Some(rake_detect))
        }
    }
}

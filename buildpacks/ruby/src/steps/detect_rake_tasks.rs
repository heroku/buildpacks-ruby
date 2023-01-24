use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use commons::gem_list::GemList;
use commons::rake_status::{detect_rake_status, RakeStatus};
use commons::rake_task_detect::RakeDetect;
use libcnb::build::BuildContext;
use libcnb::Env;
use libherokubuildpack::log as user;

pub(crate) fn detect_rake_tasks(
    gem_list: &GemList,
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
) -> Result<Option<RakeDetect>, RubyBuildpackError> {
    match detect_rake_status(
        &context.app_dir,
        gem_list,
        [".sprockets-manifest-*.json", "manifest-*.json"],
    ) {
        RakeStatus::MissingRakeGem => {
            user::log_info("Cannot run rake tasks, no rake gem in Gemfile");
            user::log_info("Add `gem 'rake'` to your Gemfile to enable");

            Ok(None)
        }
        RakeStatus::MissingRakefile => {
            user::log_info("Cannot run rake tasks, no Rakefile");
            user::log_info("Add a `Rakefile` to your project to enable");

            Ok(None)
        }
        RakeStatus::SkipManifestFound(paths) => {
            user::log_info("Skipping rake tasks. Manifest file(s) found");
            user::log_info(format!(
                "To enable, delete files: {}",
                paths
                    .iter()
                    .map(|path| path.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));

            Ok(None)
        }
        RakeStatus::Ready(path) => {
            let path = path.display();
            user::log_info(format!("Rakefile found {path}"));
            user::log_info("Rake gem found");

            user::log_info("Detecting rake tasks via `rake -P`");
            let rake_detect = RakeDetect::from_rake_command(env, true)
                .map_err(RubyBuildpackError::RakeDetectError)?;

            Ok(Some(rake_detect))
        }
    }
}

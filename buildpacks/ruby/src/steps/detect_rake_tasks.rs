use crate::gem_list::GemList;
use crate::rake_status::{check_rake_ready, RakeStatus};
use crate::rake_task_detect::RakeDetect;
use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use commons::output::{fmt, layer_logger::LayerLogger};
use libcnb::build::BuildContext;
use libcnb::Env;

pub(crate) fn detect_rake_tasks(
    logger: &LayerLogger,
    gem_list: &GemList,
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
) -> Result<Option<RakeDetect>, RubyBuildpackError> {
    let help = fmt::help_prefix();
    let rake = fmt::value("rake");
    let gemfile = fmt::value("Gemfile");
    let rakefile = fmt::value("Rakefile");

    match check_rake_ready(
        &context.app_dir,
        gem_list,
        [".sprockets-manifest-*.json", "manifest-*.json"],
    ) {
        RakeStatus::MissingRakeGem => {
            logger.lock().step(format!(
                "Cannot run rake tasks {}",
                fmt::details(format!("no {rake} gem in {gemfile}"))
            ));

            logger.lock().step(format!(
                "{help} Add {gem} to your {gemfile} to enable",
                gem = fmt::value("gem 'rake'")
            ));

            Ok(None)
        }
        RakeStatus::MissingRakefile => {
            logger.lock().step(format!(
                "Cannot run rake tasks {}",
                fmt::details(format!("no {rakefile}"))
            ));
            logger
                .lock()
                .step(format!("{help} Add {rakefile} to your project to enable",));

            Ok(None)
        }
        RakeStatus::SkipManifestFound(paths) => {
            let files = paths
                .iter()
                .map(|path| fmt::value(path.to_string_lossy()))
                .collect::<Vec<_>>()
                .join(", ");

            logger.lock().step(format!(
                "Skipping rake tasks {}",
                fmt::details(format!("Manifest files found {files}"))
            ));
            logger
                .lock()
                .step(format!("{help} Delete files to enable running rake tasks"));

            Ok(None)
        }
        RakeStatus::Ready(path) => {
            logger.lock().step(format!(
                "Rake detected {}",
                fmt::details(format!(
                    "{rake} gem found, {rakefile} found ad {path}",
                    path = fmt::value(path.to_string_lossy())
                ))
            ));

            let rake_detect = RakeDetect::from_rake_command(logger, env, true)
                .map_err(RubyBuildpackError::RakeDetectError)?;

            Ok(Some(rake_detect))
        }
    }
}

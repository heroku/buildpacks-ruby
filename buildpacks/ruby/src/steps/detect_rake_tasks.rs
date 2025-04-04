use crate::gem_list::GemList;
use crate::rake_status::{check_rake_ready, RakeStatus};
use crate::rake_task_detect::{self, RakeDetect};
use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use bullet_stream::state::SubBullet;
use bullet_stream::{style, Print};
use libcnb::build::BuildContext;
use libcnb::Env;
use std::io::Write;

pub(crate) fn detect_rake_tasks<W>(
    bullet: Print<SubBullet<W>>,
    gem_list: &GemList,
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
) -> Result<(Print<SubBullet<W>>, Option<RakeDetect>), RubyBuildpackError>
where
    W: Write + Send + Sync + 'static,
{
    let help = style::important("HELP");
    let rake = style::value("rake");
    let gemfile = style::value("Gemfile");
    let rakefile = style::value("Rakefile");

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
                    gem = style::value("gem 'rake'")
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
                .map(|path| style::value(path.to_string_lossy()))
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
            let (bullet, rake_detect) = rake_task_detect::call(
                bullet.sub_bullet(format!(
                    "Detected rake ({rake} gem found, {rakefile} found at {path})",
                    path = style::value(path.to_string_lossy())
                )),
                env,
                true,
            )
            .map_err(RubyBuildpackError::RakeDetectError)?;

            Ok((bullet, Some(rake_detect)))
        }
    }
}

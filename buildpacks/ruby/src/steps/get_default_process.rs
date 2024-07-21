use crate::gem_list::GemList;
use crate::RubyBuildpack;
use commons::output::{
    fmt,
    section_log::{log_step, SectionLogger},
};
use libcnb::build::BuildContext;
use libcnb::data::launch::Process;
use libcnb::data::launch::ProcessBuilder;
use libcnb::data::process_type;
use std::path::Path;

pub(crate) fn get_default_process(
    _logger: &dyn SectionLogger,
    context: &BuildContext<RubyBuildpack>,
    gem_list: &GemList,
) -> Option<Process> {
    let config_ru = fmt::value("config.ru");
    let rails = fmt::value("rails");
    let rack = fmt::value("rack");
    let railties = fmt::value("railties");
    match detect_web(gem_list, &context.app_dir) {
        WebProcess::Rails => {
            log_step(format!("Detected rails app ({rails} gem found)"));

            Some(default_rails())
        }
        WebProcess::RackWithConfigRU => {
            log_step(format!(
                "Detected rack app ({rack} gem found and {config_ru} at root of application)"
            ));

            Some(default_rack())
        }
        WebProcess::RackMissingConfigRu => {
            log_step(format!(
                "Skipping default web process ({rack} gem found but missing {config_ru} file)"
            ));

            None
        }
        WebProcess::Missing => {
            log_step(format!(
                "Skipping default web process ({rails}, {railties}, and {rack} not found)"
            ));

            None
        }
    }
}

enum WebProcess {
    Rails,
    RackWithConfigRU,
    RackMissingConfigRu,
    Missing,
}

fn detect_web(gem_list: &GemList, app_path: &Path) -> WebProcess {
    if gem_list.has("railties") {
        WebProcess::Rails
    } else if gem_list.has("rack") {
        if app_path.join("config.ru").exists() {
            WebProcess::RackWithConfigRU
        } else {
            WebProcess::RackMissingConfigRu
        }
    } else {
        WebProcess::Missing
    }
}

fn default_rack() -> Process {
    ProcessBuilder::new(process_type!("web"), ["bash"])
        .args([
            "-c",
            &[
                "bundle exec rackup",
                "--port \"${PORT:?Error: PORT env var is not set!}\"",
                "--host \"0.0.0.0\"",
            ]
            .join(" "),
        ])
        .default(true)
        .build()
}

fn default_rails() -> Process {
    ProcessBuilder::new(process_type!("web"), ["bash"])
        .args([
            "-c",
            &[
                "bin/rails server",
                "--port \"${PORT:?Error: PORT env var is not set!}\"",
                "--environment \"$RAILS_ENV\"",
            ]
            .join(" "),
        ])
        .default(true)
        .build()
}

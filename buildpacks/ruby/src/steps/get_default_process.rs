use crate::build_output;
use crate::build_output::section::Section;
use crate::gem_list::GemList;
use crate::RubyBuildpack;
use libcnb::build::BuildContext;
use libcnb::data::launch::Process;
use libcnb::data::launch::ProcessBuilder;
use libcnb::data::process_type;
use std::path::Path;

pub(crate) fn get_default_process(
    section: &Section,
    context: &BuildContext<RubyBuildpack>,
    gem_list: &GemList,
) -> Option<Process> {
    let config_ru = build_output::fmt::value("config.ru");
    let rails = build_output::fmt::value("rails");
    let rack = build_output::fmt::value("rack");
    let railties = build_output::fmt::value("railties");
    match detect_web(gem_list, &context.app_dir) {
        WebProcess::Rails => {
            section.say(format!("Detected rails app ({rails} gem)"));

            Some(default_rails())
        }
        WebProcess::RackWithConfigRU => {
            section.say(format!(
                "Detected rack app ({rack} gem and {config_ru} at root of application)"
            ));

            Some(default_rack())
        }
        WebProcess::RackMissingConfigRu => {
            section.say(format!(
                "Skipping default web process (detected {rack} gem but missing {config_ru} file"
            ));

            None
        }
        WebProcess::Missing => {
            section.say(format!(
                "Skipping default web process (no web gems detected: {rails}, {railties}, {rack}"
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
                "--port \"$PORT\"",
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
                "--port \"$PORT\"",
                "--environment \"$RAILS_ENV\"",
            ]
            .join(" "),
        ])
        .default(true)
        .build()
}

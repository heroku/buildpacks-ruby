use std::path::Path;

use commons::gem_list::GemList;
use libcnb::build::BuildContext;
use libcnb::data::launch::Process;
use libcnb::data::launch::ProcessBuilder;
use libcnb::data::process_type;
use libherokubuildpack::log as user;

use crate::RubyBuildpack;

pub(crate) fn get_default_process(
    context: &BuildContext<RubyBuildpack>,
    gem_list: &GemList,
) -> Option<Process> {
    match detect_web(gem_list, &context.app_dir) {
        WebProcess::Rails => {
            user::log_info("Detected railties gem");
            user::log_info("Setting default web process (rails)");

            Some(default_rails())
        }
        WebProcess::RackWithConfigRU => {
            user::log_info("Detected rack gem");
            user::log_info("Found `config.ru` file at root of application");
            user::log_info("Setting default web process (rackup)");

            Some(default_rack())
        }
        WebProcess::RackMissingConfigRu => {
            user::log_info("Detected rack gem");
            user::log_info("Missing `config.ru` file at root of application");
            user::log_info("Skipping default web process");

            None
        }
        WebProcess::Missing => {
            user::log_info("No web gems found (railties, rack)");
            user::log_info("Skipping default web process");

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
    ProcessBuilder::new(process_type!("web"), "bundle")
        .args(["exec", "rackup", "--port", "$PORT", "--host", "0.0.0.0"])
        .default(true)
        .build()
}

fn default_rails() -> Process {
    ProcessBuilder::new(process_type!("web"), "bin/rails")
        .args(["server", "--port", "$PORT", "--environment", "$RAILS_ENV"])
        .default(true)
        .build()
}

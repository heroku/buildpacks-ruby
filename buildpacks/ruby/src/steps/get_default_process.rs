use crate::gem_list::GemList;
use crate::RubyBuildpack;
use bullet_stream::style;
use bullet_stream::{state::SubBullet, Print};
use libcnb::build::BuildContext;
use libcnb::data::launch::Process;
use libcnb::data::launch::ProcessBuilder;
use libcnb::data::process_type;
use std::io::Write;
use std::path::Path;

pub(crate) fn get_default_process<W>(
    bullet: Print<SubBullet<W>>,
    context: &BuildContext<RubyBuildpack>,
    gem_list: &GemList,
) -> (Print<SubBullet<W>>, Option<Process>)
where
    W: Write + Send + Sync + 'static,
{
    let config_ru = style::value("config.ru");
    let rails = style::value("rails");
    let rack = style::value("rack");
    let railties = style::value("railties");
    match detect_web(gem_list, &context.app_dir) {
        WebProcess::Rails => (
            bullet.sub_bullet(format!("Detected rails app ({rails} gem found)")),
            Some(default_rails()),
        ),
        WebProcess::RackWithConfigRU => (
            bullet.sub_bullet(format!(
                "Detected rack app ({rack} gem found and {config_ru} at root of application)"
            )),
            Some(default_rack()),
        ),
        WebProcess::RackMissingConfigRu => (
            bullet.sub_bullet(format!(
                "Skipping default web process ({rack} gem found but missing {config_ru} file)"
            )),
            None,
        ),
        WebProcess::Missing => (
            bullet.sub_bullet(format!(
                "Skipping default web process ({rails}, {railties}, and {rack} not found)"
            )),
            None,
        ),
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
    ProcessBuilder::new(process_type!("web"), ["bash", "-c"])
        .args([[
            "bundle exec rackup",
            "--host \"[::]\"",
            "--port \"${PORT:?Error: PORT env var is not set!}\"",
        ]
        .join(" ")])
        .default(true)
        .build()
}

fn default_rails() -> Process {
    ProcessBuilder::new(process_type!("web"), ["bash", "-c"])
        .args([[
            "bin/rails server",
            "--binding \"[::]\"",
            "--port \"${PORT:?Error: PORT env var is not set!}\"",
            "--environment \"$RAILS_ENV\"",
        ]
        .join(" ")])
        .default(true)
        .build()
}

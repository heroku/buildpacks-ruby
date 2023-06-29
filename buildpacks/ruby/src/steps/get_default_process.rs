use commons::gem_list::GemList;
use libcnb::build::BuildContext;
use libcnb::data::launch::Process;
use libcnb::data::launch::ProcessBuilder;
use libcnb::data::process_type;
use libherokubuildpack::log as user;
use std::fmt::Display;
use std::path::Path;

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

#[derive(Debug, Clone, Eq, PartialEq)]
enum ShellString {
    Escape(String),
    QuoteEnvVar(String),
}

impl ShellString {
    fn escape(arg: impl Into<String>) -> ShellString {
        ShellString::Escape(arg.into())
    }

    fn quote_env_var(arg: impl Into<String>) -> ShellString {
        ShellString::QuoteEnvVar(arg.into())
    }
}

impl Display for ShellString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShellString::Escape(string) => f.write_str(&shell_words::quote(string)), // single quote only if needed
            ShellString::QuoteEnvVar(string) => write!(f, "\"{string}\""),
        }
    }
}

fn bashify(program: &ShellString, args: impl IntoIterator<Item = ShellString>) -> Process {
    let args = args
        .into_iter()
        .map(|s| s.to_string())
        .collect::<Vec<String>>()
        .join(" ");
    let command = [String::from("exec"), program.to_string(), args].join(" ");

    ProcessBuilder::new(process_type!("web"), ["bash"])
        .args(["-c", &command])
        .default(true)
        .build()
}

fn default_rack() -> Process {
    bashify(
        &ShellString::escape("bundle"),
        [
            ShellString::escape("exec"),
            ShellString::escape("rackup"),
            ShellString::escape("--port"),
            ShellString::quote_env_var("$PORT"),
            ShellString::escape("--host"),
            ShellString::escape("0.0.0.0"),
        ],
    )
}

fn default_rails() -> Process {
    bashify(
        &ShellString::escape("bin/rails"),
        [
            ShellString::escape("server"),
            ShellString::escape("--port"),
            ShellString::quote_env_var("$PORT"),
            ShellString::escape("--environment"),
            ShellString::quote_env_var("$RAILS_ENV"),
        ],
    )
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn shell_quoting() {
        assert_eq!(
            String::from("\"$PORT\""),
            ShellString::quote_env_var("$PORT").to_string()
        );

        assert_eq!(
            String::from("'hello there'"),
            ShellString::escape("hello there").to_string()
        );
    }
}

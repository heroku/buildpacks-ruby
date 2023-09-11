// Enable Clippy lints that are disabled by default.
// https://rust-lang.github.io/rust-clippy/stable/index.html
#![warn(clippy::pedantic)]

use clap::Parser;
use std::ffi::OsStr;
use std::process::ExitStatus;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::{exit, Command},
    thread::sleep,
    time::Duration,
};

static PORT: &str = "PORT";
static DYNO: &str = "DYNO";
static AGENTMON_DEBUG: &str = "AGENTMON_DEBUG";
static HEROKU_METRICS_URL: &str = "HEROKU_METRICS_URL";

const SLEEP_FOR: Duration = Duration::from_secs(1);

/// Agentmon Loop
///
/// Boots agentmon (a statsd server) in a loop
///
/// Example:
///
/// ```shell
/// $ cargo run --bin agentmon_loop -- --path <path/to/agentmon/binary>
/// ```

/// Turn CLI arguments into a Rust struct
#[derive(Parser, Debug)]
struct Args {
    /// Path to the agentmon executable e.g. --path <path/to/agentmon/binary>
    #[arg(short, long)]
    path: PathBuf,
}

fn main() {
    let agentmon = Args::parse().path;
    let agentmon_args = build_args(&std::env::vars().collect::<HashMap<String, String>>())
        .unwrap_or_else(|error| {
            eprintln!("Cannot start agentmon. {error}");
            exit(1)
        });

    match agentmon.try_exists() {
        Ok(true) => loop {
            match run(&agentmon, &agentmon_args) {
                Ok(status) => {
                    eprintln!("Process completed with status={status}, sleeping {SLEEP_FOR:?}");
                }
                Err(error) => {
                    eprintln!(
                        "Process could not run due to error. {error}, sleeping {SLEEP_FOR:?}"
                    );
                }
            };
            sleep(SLEEP_FOR);
        },
        Ok(false) => {
            eprintln!("Path does not exist {path}", path = agentmon.display());
            exit(1);
        }
        Err(error) => {
            eprintln!(
                "Could not access {path}. {error}",
                path = agentmon.display()
            );
            exit(1);
        }
    }
}

/// Print and run executable
///
/// Runs an executable at the given path with args and streams the results.
fn run<I, S>(path: &Path, args: I) -> Result<ExitStatus, std::io::Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new(path);
    cmd.args(args);

    eprintln!("Running: {}", commons::fun_run::display(&mut cmd));

    cmd.status()
}

#[derive(Debug, thiserror::Error, PartialEq)]
enum BuildArgsError {
    #[error("{PORT} environment variable is not set")]
    MissingPort,

    #[error("{HEROKU_METRICS_URL} environment variable is not set")]
    MissingMetricsUrl,

    #[error("One off dyno detected i.e. {DYNO}=\"run.*\"")]
    RunDynoDetected,
}

/// Constructs the arguments for agentmon based on environment variables
///
/// # Errors
///
/// - Environment variables: PORT or `HEROKU_METRICS_URL` are not set
/// - Environment variable DYNO starts with `run.`
fn build_args(env: &HashMap<String, String>) -> Result<Vec<String>, BuildArgsError> {
    let mut args = Vec::new();
    if env.get(DYNO).is_some_and(|value| value.starts_with("run.")) {
        return Err(BuildArgsError::RunDynoDetected);
    }

    if let Some(port) = env.get(PORT) {
        args.push(format!("-statsd-addr=:{port}"));
    } else {
        return Err(BuildArgsError::MissingPort);
    };

    if env.get(AGENTMON_DEBUG).is_some_and(|value| value == "true") {
        args.push("-debug".to_string());
    };

    if let Some(url) = env.get(HEROKU_METRICS_URL) {
        args.push(url.clone());
    } else {
        return Err(BuildArgsError::MissingMetricsUrl);
    };

    Ok(args)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn missing_run_dyno() {
        let result = build_args(&HashMap::from([("DYNO".to_string(), "run.1".to_string())]));

        assert_eq!(result, Err(BuildArgsError::RunDynoDetected));
    }

    #[test]
    fn missing_metrics_url() {
        let result = build_args(&HashMap::from([("PORT".to_string(), "123".to_string())]));

        assert_eq!(result, Err(BuildArgsError::MissingMetricsUrl));
    }

    #[test]
    fn missing_port() {
        let result = build_args(&HashMap::new());

        assert_eq!(result, Err(BuildArgsError::MissingPort));
    }

    #[test]
    fn agentmon_statsd_addr() {
        let env = HashMap::from([
            ("PORT".to_string(), "90210".to_string()),
            (
                "HEROKU_METRICS_URL".to_string(),
                "https://example.com".to_string(),
            ),
        ]);

        let result = build_args(&env);

        assert_eq!(
            result,
            Ok(vec![
                "-statsd-addr=:90210".to_string(),
                "https://example.com".to_string()
            ])
        );
    }

    #[test]
    fn agentmon_debug_args() {
        let env = HashMap::from([
            ("PORT".to_string(), "90210".to_string()),
            (
                "HEROKU_METRICS_URL".to_string(),
                "https://example.com".to_string(),
            ),
            ("AGENTMON_DEBUG".to_string(), "true".to_string()),
        ]);

        let result = build_args(&env);

        assert_eq!(
            result,
            Ok(vec![
                "-statsd-addr=:90210".to_string(),
                "-debug".to_string(),
                "https://example.com".to_string()
            ])
        );
    }
}

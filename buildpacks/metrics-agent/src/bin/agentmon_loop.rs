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
const SLEEP_FOR: Duration = Duration::from_secs(1);

/// Agentmon Loop
///
/// Boots agentmon (a statsd server) in a loop
///
/// Example:
///
/// $ cargo run --bin agentmon_loop -- --path <path/to/agentmon/binary>

/// Turn CLI arguments into a Rust struct
#[derive(Parser, Debug)]
struct Args {
    /// Path to the agentmon executable e.g. --path <path/to/agenmon/binary>
    #[arg(short, long)]
    path: PathBuf,
}

fn main() {
    let agentmon = Args::parse().path;
    if !agentmon.exists() {
        eprintln!("Path does not exist {}", agentmon.display());
        exit(1);
    }

    let agentmon_args = match build_args(std::env::vars().collect::<HashMap<String, String>>()) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Cannot start agentmon: {e}");
            exit(1)
        }
    };

    loop {
        match run(&agentmon, &agentmon_args) {
            Ok(status) => {
                eprintln!("process completed with status=${status}, sleeping {SLEEP_FOR:?}");
            }
            Err(error) => {
                eprintln!("process could not be run due to error: {error}, sleeping {SLEEP_FOR:?}");
            }
        };
        sleep(SLEEP_FOR);
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

    cmd.spawn().and_then(|mut child| child.wait())
}

#[derive(Debug, thiserror::Error, PartialEq)]
enum Error {
    #[error("PORT environment variable is not set")]
    MissingPort,

    #[error("HEROKU_METRICS_URL environment variable is not set")]
    MissingMetricsUrl,

    #[error("One off dyno detected i.e. DYNO=\"run.*\"")]
    RunDynoDetected,
}

/// Constructs the arguments for agentmon based on environment variables
///
/// # Errors
///
/// - PORT is not set
/// - HEROKU_METRICS_URL is not set
/// - DYNO starts with `run.`
fn build_args(env: HashMap<String, String>) -> Result<Vec<String>, Error> {
    let mut args = Vec::new();
    if let Some(true) = env.get("DYNO").map(|value| value.starts_with("run.")) {
        return Err(Error::RunDynoDetected);
    }

    if let Some(port) = env.get("PORT") {
        args.push(format!("statsd-addr=:{port}"));
    } else {
        return Err(Error::MissingPort);
    };

    if let Some(true) = env.get("AGENTMON_DEBUG").map(|value| value == "true") {
        args.push("-debug".to_string());
    };

    if let Some(url) = env.get("HEROKU_METRICS_URL") {
        args.push(url.clone());
    } else {
        return Err(Error::MissingMetricsUrl);
    };

    Ok(args)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn missing_port() {
        let result = build_args(HashMap::new());

        assert_eq!(result, Err(Error::MissingPort));
    }

    #[test]
    fn agentmon_args() {
        let mut env = HashMap::new();
        env.insert("PORT".to_string(), "90210".to_string());
        env.insert(
            "HEROKU_METRICS_URL".to_string(),
            "https://example.com".to_string(),
        );

        let result = build_args(env);

        assert_eq!(
            result,
            Ok(vec![
                "statsd-addr=:90210".to_string(),
                "https://example.com".to_string()
            ])
        );
    }

    #[test]
    fn agentmon_debug_args() {
        let mut env = HashMap::new();
        env.insert("PORT".to_string(), "90210".to_string());
        env.insert(
            "HEROKU_METRICS_URL".to_string(),
            "https://example.com".to_string(),
        );
        env.insert("AGENTMON_DEBUG".to_string(), "true".to_string());

        let result = build_args(env);

        assert_eq!(
            result,
            Ok(vec![
                "statsd-addr=:90210".to_string(),
                "-debug".to_string(),
                "https://example.com".to_string()
            ])
        );
    }
}

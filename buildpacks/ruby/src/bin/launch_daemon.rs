//! Schedules agentmon to run as a background daemon

// Required due to: https://github.com/rust-lang/rust/issues/95513
#![allow(unused_crate_dependencies)]

use clap::Parser;
use std::path::PathBuf;
use std::process::exit;
use std::process::Command;

static AGENTMON_DEBUG: &str = "AGENTMON_DEBUG";

/// CLI argument parser
///
/// ```shell
/// $ cargo run --bin launch_daemon \
///   --log <path/to/log.txt> \
///   --agentmon <path/to/agentmon> \
///   --loop-path <path/to/agentmon_loop>
/// ```
#[derive(Parser, Debug)]
struct Args {
    #[arg(long, value_parser = absolute_path_exists)]
    log: PathBuf,

    #[arg(long, value_parser = absolute_path_exists)]
    agentmon: PathBuf,

    #[arg(long, value_parser = absolute_path_exists)]
    loop_path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
enum ParseAbsoluteError {
    #[error("Cannot determine cannonical path for {0}. {1}")]
    CannotCanonicalize(PathBuf, std::io::Error),

    #[error("Path does not exist {0}")]
    DoesNotExist(PathBuf),

    #[error("Cannot read {0}. {1}")]
    CannotRead(PathBuf, std::io::Error),
}

/// Used to validate a path pased to the CLI exists and is accessible
fn absolute_path_exists(input: &str) -> Result<PathBuf, ParseAbsoluteError> {
    let input = PathBuf::from(input);
    let path = input
        .canonicalize()
        .map_err(|error| ParseAbsoluteError::CannotCanonicalize(input, error))?;

    if path
        .try_exists()
        .map_err(|error| ParseAbsoluteError::CannotRead(path.clone(), error))?
    {
        Ok(path)
    } else {
        Err(ParseAbsoluteError::DoesNotExist(path))
    }
}

fn main() {
    let Args {
        log,
        loop_path,
        agentmon,
    } = Args::parse();

    let mut command = Command::new("start-stop-daemon");
    if let Some(value) = std::env::var_os(AGENTMON_DEBUG) {
        fs_err::write(
            &log,
            format!(
                "Logging enabled via `{AGENTMON_DEBUG}={value:?}`. To disable `unset {AGENTMON_DEBUG}`"
            ),
        )
        .unwrap_or_else(|error| {
            eprintln!(
                "Could not write to log file {}. Reason: {error}",
                log.display()
            );
        });

        command.args(["--output", &log.to_string_lossy()]);
    } else {
        fs_err::write(
            &log,
            format!("To enable logging run with {AGENTMON_DEBUG}=1"),
        )
        .unwrap_or_else(|error| {
            eprintln!(
                "Could not write to log file {}. Reason: {error}",
                log.display()
            );
        });
    }

    command.args([
        "--start",
        "--background",
        "--exec",
        &loop_path.to_string_lossy(),
        "--",
        "--path",
        &agentmon.to_string_lossy(),
    ]);

    command.status().unwrap_or_else(|error| {
        eprintln!(
            "Command failed {}. Details: {error}",
            fun_run::display(&mut command)
        );
        exit(1)
    });
}

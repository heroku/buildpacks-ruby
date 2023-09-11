use clap::Parser;
use std::path::PathBuf;
use std::process::exit;
use std::process::Command;

/// Schedules agentmon to run as a background daemon

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

fn absolute_path_exists(s: &str) -> Result<PathBuf, String> {
    fs_err::canonicalize(PathBuf::from(s))
        .map_err(|e| format!("{s} is not a valid path. Details: {e}"))
        .and_then(|path| match path.try_exists() {
            Ok(true) => Ok(path),
            Ok(false) => Err(format!("path {} does not exist", path.display())),
            Err(e) => Err(format!("problem verifying {} exists {e}", path.display())),
        })
}

fn main() {
    let Args {
        log,
        loop_path,
        agentmon,
    } = Args::parse();

    let mut command = Command::new("start-stop-daemon");
    if std::env::var_os("AGENTMON_DEBUG").is_some() {
        command.args(["--output", &log.to_string_lossy()]);
    } else {
        fs_err::write(&log, "To enable logging run with AGENTMON_DEBUG=1").unwrap_or_else(|error| {
            eprintln!(
                "Could not write to log file {}. Reason: {error}",
                log.display()
            )
        })
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

    command
        .spawn()
        .and_then(|mut child| child.wait())
        .unwrap_or_else(|error| {
            eprintln!(
                "Command failed {}. Details: {error}",
                commons::fun_run::display(&mut command)
            );
            exit(1)
        });
}

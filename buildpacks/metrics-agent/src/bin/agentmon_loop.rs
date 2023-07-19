// Enable Clippy lints that are disabled by default.
// https://rust-lang.github.io/rust-clippy/stable/index.html
#![warn(clippy::pedantic)]

use clap::Parser;
use commons::fun_run::CmdMapExt;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{exit, Command},
    time::Duration,
};

/// Simple program to greet a person
#[derive(Parser, Debug)]
struct Args {
    /// Name of the person to greet
    #[arg(short, long)]
    path: PathBuf,
}

fn main() {
    let sleep_for = std::time::Duration::from_secs(1);
    let heroku_metrics_url = if let Some(url) = std::env::var_os("HEROKU_METRICS_URL") {
        url
    } else {
        eprintln!("Metrics agent exiting: 0 (HEROKU_METRICS_URL is not set)");
        exit(0)
    };

    if let Some(true) =
        std::env::var_os("DYNO").map(|value| value.to_string_lossy().starts_with("run."))
    {
        eprintln!("Metrics agent exiting: 0 (one off dyno detected i.e. DYNO=\"run.*\")");
        exit(0)
    }

    let agentmon = Args::parse().path;
    if !agentmon.exists() {
        eprintln!("Path does not exist {}", agentmon.display());
        exit(1);
    }

    loop {
        run_agentmon(&agentmon, &heroku_metrics_url, &sleep_for);
    }
}

fn run_agentmon(agentmon: &Path, heroku_metrics_url: &OsString, sleep_for: &Duration) {
    if let Some(port) = std::env::var_os("PORT") {
        let statsd_addr = {
            let mut string = OsString::from("statsd-addr=:");
            string.push(&port);
            string
        };

        let result = Command::new(agentmon).cmd_map(|cmd| {
            cmd.arg(&statsd_addr);

            if let Some(true) = std::env::var_os("AGENTMON_DEBUG").map(|value| value == *"true") {
                cmd.arg("-debug");
            };
            cmd.arg(heroku_metrics_url);

            cmd.spawn().and_then(|mut child| child.wait())
        });

        match result {
            Ok(status) => eprintln!("agentmon completed with status=${status}. Restarting"),
            Err(error) => {
                eprintln!("agentmon could not be run due to error: {error}");
                eprintln!("Retrying");
            }
        };
    } else {
        eprintln!("PORT is not set, sleeping {sleep_for:?}");
    }
    std::thread::sleep(*sleep_for);
}

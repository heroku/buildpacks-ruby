use core::str::FromStr;

use libcnb::Env;
use std::process::{Command, ExitStatus};

use std::str::Utf8Error;

pub struct RakeTask {
    stdout: String
    stderr: String,
}

#[derive(thiserror::Error, Debug)]
pub enum RakeDetectError {
    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),

    #[error("Command `bundle exec rake {0}` errored: {0}")]
    CommandError(OsString, std::io::Error),

    #[error("Command `bundle exec rake {0}` exited with non-zero error code {0} stdout:\n{1}\nstderr:\n{2}\n")]
    UnexpectedExitStatus(String, ExitStatus, String, String),

    #[error("Encoding error: {0}")]
    EncodingError(#[from] Utf8Error),
}

struct CommandBuilder {
    task: String,
    command: Option<Command>
}

impl RakeTask {
    pub fn command(task: &str, env: &Env) -> Command {
        let mut command = Command::new("bundle");
        command.args(&["exec", "rake", task, "--trace"]).envs(env);
        command
    }

    pub fn from_command(command: Command) -> Result<Self, RakeDetectError> {
        let output = command
            .output()
            .map_err(|io_err| RakeDetectError::RakeDashpCommandError(command.get_program(), io_err))?;


        let stdout = std::str::from_utf8(&output.stdout).map_err(RakeDetectError::EncodingError)?;
        let stderr = std::str::from_utf8(&output.stderr).map_err(RakeDetectError::EncodingError)?;
        if output.status.success() {
            Ok(RakeTask { stdout: stdout.to_string(), stderr: stderr.to_string() })
        } else {
            Err(RakeDetectError::RakeDashpUnexpectedExitStatus(
                output.status,
                stdout.to_string(),
                stderr.to_string(),
            ))
        }
    }
}

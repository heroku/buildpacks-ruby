use libcnb::Env;
use std::ffi::OsString;
use std::fmt;
use std::io::BufRead;
use std::io::BufReader;

#[allow(unused_imports)]
use std::io::Write;
use std::process::Stdio;
use std::process::{Command, ExitStatus};

use regex::Regex;
use std::fmt::Debug;
use std::fmt::Display;

use std::thread;

/// # Run a command, with an env!
///
/// Reduce the interface of running commands, and introduce defaults (like auto
/// `Err` on non-zero status code).
///
/// By default will return `env_command::EnvCommandError::UnexpectedExitStatusError(NonZeroExitStatusError)`
/// when the command exits with a non-zero exit code. The struct `NonZeroExitStatusError` includes
/// the field `outcome` which is a `EnvCommandResult` that contains status, stdout, and stderr.
///
/// WARNING: Internals can panic in some situations. See `expect()` in code.
///
/// Example:
///
/// ```rust,no_run
/// use crate::env_command::EnvCommand;
///
/// let env = Env::new();
/// let mut command = EnvCommand::new("echo", &["hello world"], &env);
/// let outcome = command.call().unwrap();
///
/// assert_eq!(outcome.stdout.trim(), "hello world".to_string());
/// ```
///
/// By default it will return `Result<Err>` if the command is not successful.
/// To return `Ok` instead use `allow_non_zero_exit()`:
///
/// ```rust,no_run
/// use crate::env_command::EnvCommand;
///
/// let env = Env::new();
/// let mut outcome = EnvCommand::new("iDoNotExist", &["hello world"], &env)
///                   .allow_non_zero_exit()
///                   .call()
///                   .unwrap();
///
/// assert!(!outcome.status.success());
/// assert!(outcome.stderr.contains("command not found: iDoNotExist"))
/// ```
///
///
/// Can run command and capture the output with `call()` or can stream
/// the command output to stdout/stderr with `stream()`.
///
/// The command can advertize itself via `to_string()`:
///
/// ```rust,no_run
/// use crate::env_command::EnvCommand;
///
/// let mut command = EnvCommand::new("echo", &["hello world"], &env);
/// assert_eq!(command.to_string(), "echo \"hello world\"")
/// ```
///
/// The command can advertize itself with accept list env vars via `display_env_keys()`:
///
/// ```rust,no_run
/// use crate::env_command::EnvCommand;
///
/// let mut env = Env::new();
/// env.insert("DOG", "cinco");
///
/// let mut command =  EnvCommand::new("echo", &["hello world"], &env);
/// command.display_env_keys(&["DOG"]);
///
/// assert_eq!(command.to_string(), "DOG=\"cinco\" echo \"hello world\"")
/// ```
#[allow(dead_code)]
pub struct EnvCommand {
    base: OsString,
    args: Vec<OsString>,
    env: Env,
    display_env_keys: Vec<OsString>,
    allow_non_zero_exit: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EnvCommandResult {
    pub stdout: String,
    pub stderr: String,
    pub status: ExitStatus,
}

#[allow(dead_code)]
#[derive(thiserror::Error, Debug)]
pub enum EnvCommandError {
    #[error("Command `{0}` failed with IO error: {1}")]
    IOError(String, std::io::Error),

    #[error("{0}")]
    UnexpectedExitStatusError(NonZeroExitStatusError),
}

#[derive(Debug)]
pub struct NonZeroExitStatusError {
    command: String,
    result: EnvCommandResult,
}

impl Display for NonZeroExitStatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Command {} exited with non-zero error code {} stdout:\n{}\nstderr:\n{}\n",
            self.command, self.result.status, self.result.stdout, self.result.stderr
        )
    }
}

/// Used for implementing `to_string()`
impl fmt::Display for EnvCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let command = self.command();
        let escape_pattern = Regex::new(r"([^A-Za-z0-9_\-.,:/@\n])").unwrap(); // https://github.com/jimmycuadra/rust-shellwords/blob/d23b853a850ceec358a4137d5e520b067ddb7abc/src/lib.rs#L23

        write!(
            f,
            "{}",
            // Env vars
            self.display_env_keys
                .iter()
                .map(|key| {
                    format!(
                        "{}={:?}",
                        key.to_string_lossy(),
                        self.env
                            .get(key.clone())
                            .unwrap_or_else(|| OsString::from(""))
                    )
                })
                // Main command
                .chain(vec![command.get_program().to_string_lossy().to_string()].into_iter())
                // Args
                .chain(
                    command
                        .get_args()
                        .map(std::ffi::OsStr::to_string_lossy)
                        .map(|arg| {
                            if escape_pattern.is_match(&arg) {
                                format!("{:?}", arg)
                            } else {
                                format!("{}", arg)
                            }
                        }),
                )
                .collect::<Vec<String>>()
                .join(" ")
        )
    }
}

impl EnvCommand {
    pub fn non_zero_exit_error_from_outcome(&self, result: EnvCommandResult) -> EnvCommandError {
        EnvCommandError::UnexpectedExitStatusError(NonZeroExitStatusError {
            command: self.to_string(),
            result: result.clone(),
        })
    }

    /// Main entrypoint, builds a struct with defaults and the arguments
    /// given
    #[allow(dead_code)]
    pub fn new(base: &str, args: &[&str], env: &Env) -> Self {
        EnvCommand {
            base: base.into(),
            args: args
                .iter()
                .map(std::convert::Into::into)
                .collect::<Vec<OsString>>(),
            env: env.clone(),
            display_env_keys: Vec::new(),
            allow_non_zero_exit: false,
        }
    }

    pub fn display_env_keys(
        &mut self,
        keys: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> &mut Self {
        self.display_env_keys = keys
            .into_iter()
            .map(std::convert::Into::into)
            .collect::<Vec<OsString>>();
        self
    }

    // Command is not clonable because it can contain things that are not
    // clonable such as file descriptors. Instead we remember the
    // inputs to Command so we can re-create it at will.
    fn command(&self) -> Command {
        let mut command = Command::new(&self.base);
        command.args(&self.args);
        command.envs(&self.env);
        command
    }

    /// Tells the code to not return an `Err` result when a non-zero
    /// status code is received.
    #[allow(dead_code)]
    pub fn allow_non_zero_exit(&mut self) -> &mut Self {
        self.allow_non_zero_exit = true;
        self
    }

    // Runs the command and streams contents to STDOUT/STDERR
    #[allow(dead_code)]
    pub fn stream(&self) -> Result<EnvCommandResult, EnvCommandError> {
        let mut child = self
            .command()
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|io_err| EnvCommandError::IOError(self.to_string(), io_err))
            .unwrap();

        let child_stdout = child
            .stdout
            .take()
            .expect("Internal error, could not take stdout");
        let child_stderr = child
            .stderr
            .take()
            .expect("Internal error, could not take stderr");

        let (stdout_tx, stdout_rx) = std::sync::mpsc::channel();
        let (stderr_tx, stderr_rx) = std::sync::mpsc::channel();

        let stdout_thread = thread::spawn(move || {
            let stdout_lines = BufReader::new(child_stdout).lines();
            for line in stdout_lines {
                let line = line.unwrap();
                println!("{}", line);
                stdout_tx.send(line).unwrap();
            }
        });

        let stderr_thread = thread::spawn(move || {
            let stderr_lines = BufReader::new(child_stderr).lines();
            for line in stderr_lines {
                let line = line.unwrap();
                eprintln!("{}", line);
                stderr_tx.send(line).unwrap();
            }
        });

        let status = child
            .wait()
            .expect("Internal error, failed to wait on child");

        stdout_thread.join().unwrap();
        stderr_thread.join().unwrap();

        let stdout = stdout_rx.into_iter().collect::<Vec<String>>().join("");
        let stderr = stderr_rx.into_iter().collect::<Vec<String>>().join("");

        let result = EnvCommandResult {
            stdout,
            stderr,
            status,
        };

        if status.success() || self.allow_non_zero_exit {
            Ok(result)
        } else {
            Err(EnvCommandError::UnexpectedExitStatusError(
                NonZeroExitStatusError {
                    command: self.to_string(),
                    result,
                },
            ))
        }
    }

    // Runs the shell command silenty
    #[allow(dead_code)]
    pub fn call(&self) -> Result<EnvCommandResult, EnvCommandError> {
        let output = self
            .command()
            .output()
            .map_err(|io_err| EnvCommandError::IOError(self.to_string(), io_err))?;

        let stdout = std::str::from_utf8(&output.stdout)
            .expect("Internal encoding error")
            .to_string();
        let stderr = std::str::from_utf8(&output.stderr)
            .expect("Internal encoding error")
            .to_string();

        let status = output.status;
        let result = EnvCommandResult {
            stdout,
            stderr,
            status,
        };
        if status.success() || self.allow_non_zero_exit {
            Ok(result)
        } else {
            Err(EnvCommandError::UnexpectedExitStatusError(
                NonZeroExitStatusError {
                    command: self.to_string(),
                    result,
                },
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_string() {
        let command = EnvCommand::new("echo", &["hello world"], &Env::new());
        assert_eq!(command.to_string(), r#"echo "hello world""#);
    }

    #[test]
    fn test_to_string_with_env_keys() {
        let mut env = Env::new();
        env.insert("PATH", "foo");

        let mut command = EnvCommand::new("echo", &["hello world"], &env);
        command.display_env_keys(&["PATH"]);
        assert_eq!(r#"PATH="foo" echo "hello world""#, command.to_string());
    }

    #[test]
    fn runs_command_and_captures_stdout() {
        // Platform dependent
        let command = EnvCommand::new("echo", &["hello world"], &Env::new());
        let outcome = command.call().unwrap();

        assert_eq!(outcome.stdout.trim(), "hello world".to_string());
    }

    #[test]
    fn runs_command_and_captures_stdout_while_streaming_to_stdout_stderr() {
        // Platform dependent
        let command = EnvCommand::new("echo", &["hello world"], &Env::new());
        let outcome = command.stream().unwrap();

        assert_eq!(outcome.stdout.trim(), "hello world".to_string());
    }

    #[test]
    fn test_command_to_str_with_env_keys_one_exists() {
        let mut env = Env::new();
        env.insert("TRANSPORT", "perihelion");

        let mut command = EnvCommand::new("bundle", &["install", "--path", "lol"], &env);
        command.display_env_keys(&["TRANSPORT"]);

        assert_eq!(
            "TRANSPORT=\"perihelion\" bundle install --path lol",
            command.to_string()
        );
    }

    #[test]
    fn test_command_to_str_with_env_keys_one_missing() {
        let env = Env::new();

        let mut command = EnvCommand::new("bundle", &["install", "--path", "lol"], &env);
        command.display_env_keys(&["TRANSPORT"]);

        assert_eq!(
            "TRANSPORT=\"\" bundle install --path lol",
            command.to_string()
        );
    }

    #[test]
    fn test_command_to_str_with_env_keys_two_exist() {
        let mut env = Env::new();
        env.insert("TRANSPORT", "perihelion");
        env.insert("SHOW", "the rise and fall of sanctuary moon");

        let mut command = EnvCommand::new("bundle", &["install", "--path", "lol"], &env);
        command.display_env_keys(&["TRANSPORT", "SHOW"]);

        assert_eq!("TRANSPORT=\"perihelion\" SHOW=\"the rise and fall of sanctuary moon\" bundle install --path lol", command.to_string());
    }

    #[test]
    fn test_command_to_str_with_env_keys_two_with_one_empty() {
        let mut env = Env::new();
        env.insert("TRANSPORT", "perihelion");

        let mut command = EnvCommand::new("bundle", &["install", "--path", "lol"], &env);
        command.display_env_keys(&["TRANSPORT", "SHOW"]);

        assert_eq!(
            "TRANSPORT=\"perihelion\" SHOW=\"\" bundle install --path lol",
            command.to_string()
        );
    }
}

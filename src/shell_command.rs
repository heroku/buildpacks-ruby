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
use std::thread;

/// # Run a command, on the shell!
///
/// ```rust,no_run
/// use crate::shell_command::ShellCommand;
///
/// let env = Env::new()
/// let mut command = ShellCommand::new_with_args("echo", &["hello world"]);
/// let outcome = command.call(env).unwrap();
///
/// assert_eq!(outcome.stdout.trim(), "hello world".to_string());
/// ```
///
/// By default it will return `Result<Err>` if the command is not successful.
/// To return `Ok` instead use `allow_non_zero_exit()`:
///
/// ```rust,no_run
/// use crate::shell_command::ShellCommand;
///
/// let mut outcome = ShellCommand::new_with_args("iDoNotExist", &["hello world"])
///                   .allow_non_zero_exit()
///                   .call(env)
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
/// use crate::shell_command::ShellCommand;
///
/// let mut command = ShellCommand::new_with_args("echo", &["hello world"]);
/// assert_eq!(command.to_string(), "echo \"hello world\"")
/// ```
///
/// The command can advertize itself with accept list env vars via `to_string_with_env_keys()`:
///
/// ```rust,no_run
/// use crate::shell_command::ShellCommand;
///
/// let mut env = Env::new();
/// env.insert("DOG", "cinco");
///
/// let mut command =  ShellCommand::new_with_args("echo", &["hello world"]);
/// assert_eq!(command.to_string_with_env_keys(&env, &["DOG"]), "DOG=\"cinco\" echo \"hello world\"")
/// ```
#[allow(dead_code)]
pub struct ShellCommand {
    command: Command,
    allow_non_zero_exit: bool,
}

#[allow(dead_code)]
pub struct ShellCommandOutcome {
    pub stdout: String,
    pub stderr: String,
    pub status: ExitStatus,
}

#[allow(dead_code)]
#[derive(thiserror::Error, Debug)]
pub enum ShellCommandError {
    #[error("Command `{0}` failed with IO error: {1}")]
    IOError(String, std::io::Error),

    #[error("Command `{0}` exited with non-zero error code {1} stdout:\n{1}\nstderr:\n{2}\n")]
    UnexpectedExitStatus(String, ExitStatus, String, String),
}

impl fmt::Display for ShellCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let escape_pattern = Regex::new(r"([^A-Za-z0-9_\-.,:/@\n])").unwrap(); // https://github.com/jimmycuadra/rust-shellwords/blob/d23b853a850ceec358a4137d5e520b067ddb7abc/src/lib.rs#L23
        write!(
            f,
            "{} {}",
            self.command.get_program().to_string_lossy(),
            self.command
                .get_args()
                .map(std::ffi::OsStr::to_string_lossy)
                .map(|arg| {
                    if escape_pattern.is_match(&arg) {
                        format!("{:?}", arg)
                    } else {
                        format!("{}", arg)
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
}

impl ShellCommand {
    #[allow(dead_code)]
    pub fn to_string_with_env_keys(
        &self,
        env: &Env,
        keys: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> String {
        format!(
            "{} {}",
            keys.into_iter()
                .map(std::convert::Into::into)
                .map(|key| {
                    format!(
                        "{}={:?} ",
                        key.to_string_lossy(),
                        env.get(key.clone()).unwrap_or_else(|| OsString::from(""))
                    )
                })
                .collect::<String>()
                .trim(),
            self
        )
    }

    #[allow(dead_code)]
    pub fn new_with_args(base: &str, args: &[&str]) -> Self {
        let mut command = Command::new(base);
        command.args(args);
        ShellCommand {
            command,
            allow_non_zero_exit: false,
        }
    }

    #[allow(dead_code)]
    pub fn allow_non_zero_exit(&mut self) -> &mut Self {
        self.allow_non_zero_exit = true;
        self
    }

    #[allow(dead_code)]
    // Runs the command and streams contents to STDOUT/STDERR
    pub fn stream(&mut self, env: &Env) -> Result<ShellCommandOutcome, ShellCommandError> {
        let mut child = self
            .command
            .envs(env)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|io_err| ShellCommandError::IOError(self.to_string(), io_err))
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

        if status.success() || self.allow_non_zero_exit {
            Ok(ShellCommandOutcome {
                stdout,
                stderr,
                status,
            })
        } else {
            Err(ShellCommandError::UnexpectedExitStatus(
                self.to_string(),
                status,
                stdout,
                stderr,
            ))
        }
    }

    #[allow(dead_code)]
    // Runs the shell command silenty
    pub fn call(&mut self, env: &Env) -> Result<ShellCommandOutcome, ShellCommandError> {
        let output = self
            .command
            .envs(env)
            .output()
            .map_err(|io_err| ShellCommandError::IOError(self.to_string(), io_err))?;

        let stdout = std::str::from_utf8(&output.stdout)
            .expect("Internal encoding error")
            .to_string();
        let stderr = std::str::from_utf8(&output.stderr)
            .expect("Internal encoding error")
            .to_string();
        if output.status.success() || self.allow_non_zero_exit {
            Ok(ShellCommandOutcome {
                stdout,
                stderr,
                status: output.status,
            })
        } else {
            Err(ShellCommandError::UnexpectedExitStatus(
                self.to_string(),
                output.status,
                stdout,
                stderr,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_string() {
        let command = ShellCommand::new_with_args("echo", &["hello world"]);
        assert_eq!(command.to_string(), r#"echo "hello world""#);
    }

    #[test]
    fn test_to_string_with_env_keys() {
        let mut env = Env::new();
        env.insert("PATH", "foo");

        let command = ShellCommand::new_with_args("echo", &["hello world"]);
        assert_eq!(
            command.to_string_with_env_keys(&env, &["PATH"]),
            r#"PATH="foo" echo "hello world""#
        );
    }

    #[test]
    fn runs_command_and_captures_stdout() {
        // Platform dependent
        let mut command = ShellCommand::new_with_args("echo", &["hello world"]);
        let outcome = command.call(&Env::new()).unwrap();

        assert_eq!(outcome.stdout.trim(), "hello world".to_string());
    }

    #[test]
    fn runs_command_and_captures_stdout_while_streaming_to_stdout_stderr() {
        // Platform dependent
        let mut command = ShellCommand::new_with_args("echo", &["hello world"]);
        let outcome = command.stream(&Env::new()).unwrap();

        assert_eq!(outcome.stdout.trim(), "hello world".to_string());
    }

    #[test]
    fn test_command_to_str_with_env_keys_one_exists() {
        let mut env = Env::new();
        env.insert("TRANSPORT", "perihelion");

        let command = ShellCommand::new_with_args("bundle", &["install", "--path", "lol"]);

        let out = command.to_string_with_env_keys(&env, &["TRANSPORT"]);
        assert_eq!("TRANSPORT=\"perihelion\" bundle install --path lol", out);
    }

    #[test]
    fn test_command_to_str_with_env_keys_one_missing() {
        let env = Env::new();

        let command = ShellCommand::new_with_args("bundle", &["install", "--path", "lol"]);

        let out = command.to_string_with_env_keys(&env, &["TRANSPORT"]);
        assert_eq!("TRANSPORT=\"\" bundle install --path lol", out);
    }

    #[test]
    fn test_command_to_str_with_env_keys_two_exist() {
        let mut env = Env::new();
        env.insert("TRANSPORT", "perihelion");
        env.insert("SHOW", "the rise and fall of sanctuary moon");

        let command = ShellCommand::new_with_args("bundle", &["install", "--path", "lol"]);

        let out = command.to_string_with_env_keys(&env, &["TRANSPORT", "SHOW"]);
        assert_eq!("TRANSPORT=\"perihelion\" SHOW=\"the rise and fall of sanctuary moon\" bundle install --path lol", out);
    }

    #[test]
    fn test_command_to_str_with_env_keys_two_with_one_empty() {
        let mut env = Env::new();
        env.insert("TRANSPORT", "perihelion");

        let command = ShellCommand::new_with_args("bundle", &["install", "--path", "lol"]);

        let out = command.to_string_with_env_keys(&env, &["TRANSPORT", "SHOW"]);
        assert_eq!(
            "TRANSPORT=\"perihelion\" SHOW=\"\" bundle install --path lol",
            out
        );
    }
}

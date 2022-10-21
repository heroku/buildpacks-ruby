use libcnb::Env;
use std::ffi::OsString;
use std::fmt;
use std::io::BufRead;
use std::io::BufReader;

use std::os::unix::prelude::ExitStatusExt;
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
/// the field `result` which is a `EnvCommandResult` that contains status, stdout, and stderr.
///
/// Example:
///
/// ```rust,no_run
/// use heroku_ruby_buildpack::env_command::EnvCommand;
/// use libcnb::Env;
/// lakjsdflkajsdflkjasdfkljasdf
///
/// let env = Env::new();
/// let mut command = EnvCommand::new("echo", &["hello world"], &env);
/// let outcome = command.call().unwrap();
///
/// assert_eq!(outcome.stdout.trim(), "hello world".to_string());
/// ```
///
/// Can run command and capture the output with `call()` or can stream
/// the command output to stdout/stderr with `stream()`.
///
/// ## Customized behavior on non-zero exit
///
/// By default it will return `Result<Err>` if the command is not successful.
/// To return `Ok` instead use `on_non_zero_exit()` to overwrite the behavior.
///
/// ## Display
///
/// The command can advertize itself via `to_string()`:
///
/// ```rust,no_run
/// use heroku_ruby_buildpack::env_command::EnvCommand;
/// use libcnb::Env;
///
///
/// let mut command = EnvCommand::new("echo", &["hello world"], &Env::new());
/// assert_eq!(command.to_string(), "echo \"hello world\"")
/// ```
///
/// ## Display with env keys
///
/// In some cases it is useful to also show environment variables when displaying
/// a command. For example `RAILS_ENV` can have a great impact when running `rake assets:precompile`
/// on a rails app. To
///
/// The command can advertize itself with accept list env vars via `show_env_keys()`:
///
/// ```rust,no_run
/// use heroku_ruby_buildpack::env_command::EnvCommand;
/// use libcnb::Env;
///
/// let mut env = Env::new();
/// env.insert("DOG", "cinco");
///
/// let mut command =  EnvCommand::new("echo", &["hello world"], &env);
/// command.show_env_keys(&["DOG"]);
///
/// assert_eq!(command.to_string(), "DOG=\"cinco\" echo \"hello world\"")
/// ```
#[allow(dead_code)]
pub struct EnvCommand {
    base: OsString,
    args: Vec<OsString>,
    env: Env,
    show_env_keys: Vec<OsString>,
    on_non_zero_exit:
        Box<dyn Fn(NonZeroExitStatusError) -> Result<EnvCommandResult, NonZeroExitStatusError>>,
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
    #[error("{0}")]
    UnexpectedExitStatusError(NonZeroExitStatusError),
}

#[derive(Debug)]
pub struct NonZeroExitStatusError {
    pub command: String,
    pub result: EnvCommandResult,
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

impl EnvCommand {
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
            show_env_keys: Vec::new(),
            on_non_zero_exit: Box::new(Err),
        }
    }

    /// Attach environment variables to the struct for use when
    /// `Display` is called on it.
    ///
    /// ```rust,no_run
    /// use heroku_ruby_buildpack::env_command::EnvCommand;
    /// use libcnb::Env;
    ///
    /// let mut env = Env::new();
    /// env.insert("DOG", "cinco");
    ///
    /// let mut command =  EnvCommand::new("echo", &["hello world"], &env);
    /// command.show_env_keys(&["DOG"]);
    ///
    /// assert_eq!(command.to_string(), "DOG=\"cinco\" echo \"hello world\"")
    /// ```
    ///
    /// Does not change the behavior of the executing command.
    pub fn show_env_keys(
        &mut self,
        keys: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> &mut Self {
        self.show_env_keys = keys
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

    /// Runs the command silently and capture STDOUT & STDERR
    ///
    /// ```rust,no_run
    /// use heroku_ruby_buildpack::env_command::EnvCommand;
    /// use libcnb::Env;
    ///
    /// let env = Env::new();
    /// let mut command = EnvCommand::new("echo", &["hello world"], &env);
    /// let outcome = command.call().unwrap();
    ///
    /// assert_eq!(outcome.stdout.trim(), "hello world".to_string());
    /// ```
    #[allow(dead_code)]
    pub fn call(&self) -> Result<EnvCommandResult, EnvCommandError> {
        self.command()
            .output()
            .map(|output| EnvCommandResult {
                stdout: std::str::from_utf8(&output.stdout)
                    .expect("Internal encoding error")
                    .to_string(),
                stderr: std::str::from_utf8(&output.stderr)
                    .expect("Internal encoding error")
                    .to_string(),
                status: output.status,
            })
            .or_else(|error| {
                Ok(EnvCommandResult {
                    stdout: String::new(),
                    stderr: format!("{}", error),
                    status: ExitStatus::from_raw(error.raw_os_error().unwrap_or(-1)),
                })
            })
            .and_then(|result| {
                if result.status.success() {
                    Ok(result)
                } else {
                    self.handle_non_zero_exit_error(NonZeroExitStatusError {
                        command: self.to_string(),
                        result,
                    })
                }
            })
    }

    /// Runs the command and streams contents to STDOUT/STDERR
    ///
    /// ```rust,no_run
    /// use heroku_ruby_buildpack::env_command::EnvCommand;
    /// use libcnb::Env;
    ///
    /// let env = Env::new();
    /// let mut command = EnvCommand::new("echo", &["hello world"], &env);
    /// let outcome = command.stream().unwrap();
    ///
    /// assert_eq!(outcome.stdout.trim(), "hello world".to_string());
    /// ```
    #[allow(dead_code)]
    pub fn stream(&self) -> Result<EnvCommandResult, EnvCommandError> {
        self.command()
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map(|mut child| EnvCommand::stream_child(&mut child))
            .or_else(|error| {
                Ok(EnvCommandResult {
                    stdout: String::new(),
                    stderr: format!("{}", error),
                    status: ExitStatus::from_raw(error.raw_os_error().unwrap_or(-1)),
                })
            })
            .and_then(|result| {
                if result.status.success() {
                    Ok(result)
                } else {
                    self.handle_non_zero_exit_error(NonZeroExitStatusError {
                        command: self.to_string(),
                        result,
                    })
                }
            })
    }

    /// By default non-zero-exit codes return an error.
    /// use this method to implement custom behavior.
    ///
    /// ```rust,no_run
    /// use heroku_ruby_buildpack::env_command::EnvCommand;
    /// use libcnb::Env;
    ///
    /// let env = Env::new();
    /// let mut outcome = EnvCommand::new("iDoNotExist", &["hello world"], &env)
    ///                   .on_non_zero_exit(|error| Ok(error.result) )
    ///                   .call()
    ///                   .unwrap();
    ///
    /// assert!(!outcome.status.success());
    /// assert!(outcome.stderr.contains("command not found: iDoNotExist"))
    /// ```
    pub fn on_non_zero_exit(
        &mut self,
        fun: impl Fn(NonZeroExitStatusError) -> Result<EnvCommandResult, NonZeroExitStatusError>
            + 'static,
    ) -> &mut Self {
        self.on_non_zero_exit = Box::new(fun);
        self
    }

    /// Internal helper for streaming a child process to stdout/stderr
    /// while also collecting the results of the process.
    fn stream_child(child: &mut std::process::Child) -> EnvCommandResult {
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

        let stdout = stdout_rx.into_iter().collect::<String>();
        let stderr = stderr_rx.into_iter().collect::<String>();

        EnvCommandResult {
            stdout,
            stderr,
            status,
        }
    }

    /// The user can specify an custom function to be called when
    /// a non-zero exit status is encountered.
    ///
    /// This is a helper function to invoke that behavior.
    fn handle_non_zero_exit_error(
        &self,
        error: NonZeroExitStatusError,
    ) -> Result<EnvCommandResult, EnvCommandError> {
        (self.on_non_zero_exit)(error).map_err(EnvCommandError::UnexpectedExitStatusError)
    }

    pub fn to_string(
        command: &Command,
        show_env_keys: impl Iterator<Item = impl Into<OsString>>,
        env: &Env,
    ) -> String {
        let escape_pattern = Regex::new(r"([^A-Za-z0-9_\-.,:/@\n])").unwrap(); // https://github.com/jimmycuadra/rust-shellwords/blob/d23b853a850ceec358a4137d5e520b067ddb7abc/src/lib.rs#L23

        // Env keys
        show_env_keys
            .map(|key| {
                let key = key.into();
                format!(
                    "{}={:?}",
                    key.to_string_lossy(),
                    env.get(key.clone()).unwrap_or_else(|| OsString::from(""))
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
    }
}

// Used for implementing `to_string()`
impl fmt::Display for EnvCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            EnvCommand::to_string(&self.command(), self.show_env_keys.iter(), &self.env)
        )
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
        command.show_env_keys(["PATH"]);
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
    fn runs_non_zero_exit_error() {
        // Platform dependent
        let mut command = EnvCommand::new("exit", &["123"], &Env::new());
        let outcome = command.call();

        assert!(outcome.is_err());

        // Ignore the error
        command.on_non_zero_exit(|err| Ok(err.result));
        command.call().unwrap();
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
        command.show_env_keys(["TRANSPORT"]);

        assert_eq!(
            "TRANSPORT=\"perihelion\" bundle install --path lol",
            command.to_string()
        );
    }

    #[test]
    fn test_command_to_str_with_env_keys_one_missing() {
        let env = Env::new();

        let mut command = EnvCommand::new("bundle", &["install", "--path", "lol"], &env);
        command.show_env_keys(["TRANSPORT"]);

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
        command.show_env_keys(["TRANSPORT", "SHOW"]);

        assert_eq!("TRANSPORT=\"perihelion\" SHOW=\"the rise and fall of sanctuary moon\" bundle install --path lol", command.to_string());
    }

    #[test]
    fn test_command_to_str_with_env_keys_two_with_one_empty() {
        let mut env = Env::new();
        env.insert("TRANSPORT", "perihelion");

        let mut command = EnvCommand::new("bundle", &["install", "--path", "lol"], &env);
        command.show_env_keys(["TRANSPORT", "SHOW"]);

        assert_eq!(
            "TRANSPORT=\"perihelion\" SHOW=\"\" bundle install --path lol",
            command.to_string()
        );
    }
}

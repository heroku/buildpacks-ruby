use libherokubuildpack::command::CommandExt;
use regex::Regex;
use std::borrow::Borrow;
use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::os::unix::prelude::ExitStatusExt;
use std::process::Output;
use std::process::{Command, ExitStatus};

/// # Run a command, with an env!
///
/// Reduce the interface of running commands, and introduce defaults (like auto
/// `Err` on non-zero status code).
///
/// By default will return `env_command::CommandError::UnexpectedExitStatusError(NonZeroExitStatusError)`
/// when the command exits with a non-zero exit code. The struct `NonZeroExitStatusError` includes
/// the field `result` which is a `Output` that contains status, stdout, and stderr.
///
/// Example:
///
/// ```rust,no_run
/// use commons::env_command::{EnvCommand, OutputEx};
/// use libcnb::Env;
///
/// let env = Env::new();
/// let mut command = EnvCommand::new("echo", &["hello world"], &env);
/// let outcome = command.output().unwrap();
///
/// assert_eq!(outcome.stdout_lossy().trim(), "hello world".to_string());
/// ```
///
/// Can run command and capture the output with `output()` or can stream
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
/// use commons::env_command::EnvCommand;
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
/// use commons::env_command::EnvCommand;
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
pub struct EnvCommand {
    base: OsString,
    args: Vec<OsString>,
    env: HashMap<OsString, OsString>,
    show_env_keys: Vec<OsString>,
    on_non_zero_exit: Box<dyn Fn(NonZeroExitStatusError) -> Result<Output, NonZeroExitStatusError>>,
}

/// Convienece traite to extend ```Output```
///
/// Gives ```Output``` functions for returning stdout and stderr as lossy strings.
pub trait OutputEx {
    fn stdout_lossy(&self) -> Cow<'_, str>;
    fn stderr_lossy(&self) -> Cow<'_, str>;
}

impl OutputEx for Output {
    fn stdout_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.stdout)
    }

    fn stderr_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.stderr)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum CommandError {
    #[error("{0}")]
    UnexpectedExitStatusError(NonZeroExitStatusError),
}

#[derive(Debug)]
pub struct NonZeroExitStatusError {
    pub command: String,
    pub result: Output,
    already_streamed: bool,
}

impl Display for NonZeroExitStatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut output = format!(
            "Command '{}' exited with non-zero status code '{}'\n",
            self.command, self.result.status
        );
        if !self.already_streamed {
            output.push_str(&format!(
                "stdout: {}\nstderr{}\n",
                self.result.stdout_lossy(),
                self.result.stderr_lossy()
            ));
        };
        write!(f, "{output}")
    }
}

impl EnvCommand {
    /// Main entrypoint, builds a struct with defaults and the arguments
    /// given
    pub fn new<T: IntoIterator<Item = (K, V)>, K: Into<OsString>, V: Into<OsString>>(
        base: &str,
        args: &[&str],
        env: T,
    ) -> Self {
        EnvCommand {
            base: base.into(),
            args: args
                .iter()
                .map(std::convert::Into::into)
                .collect::<Vec<OsString>>(),
            env: env
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect::<HashMap<OsString, OsString>>(),
            show_env_keys: Vec::new(),
            on_non_zero_exit: Box::new(Err),
        }
    }

    /// New with keys defined from the beginning
    pub fn new_show_keys<T: IntoIterator<Item = (K, V)>, K: Into<OsString>, V: Into<OsString>>(
        base: &str,
        args: &[&str],
        env: T,
        show_env_keys: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> Self {
        EnvCommand {
            base: base.into(),
            args: args
                .iter()
                .map(std::convert::Into::into)
                .collect::<Vec<OsString>>(),
            env: env
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect::<HashMap<OsString, OsString>>(),
            show_env_keys: show_env_keys
                .into_iter()
                .map(std::convert::Into::into)
                .collect::<Vec<OsString>>(),
            on_non_zero_exit: Box::new(Err),
        }
    }

    /// Attach environment variables to the struct for use when
    /// `Display` is called on it.
    ///
    /// ```rust,no_run
    /// use commons::env_command::EnvCommand;
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

    // Command is not cloneable because it can contain things that are not
    // cloneable such as file descriptors. Instead we remember the
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
    /// use commons::env_command::{EnvCommand, OutputEx};
    /// use libcnb::Env;
    ///
    /// let env = Env::new();
    /// let mut command = EnvCommand::new("echo", &["hello world"], &env);
    /// let outcome = command.output().unwrap();
    ///
    /// assert_eq!(outcome.stdout_lossy().trim(), "hello world".to_string());
    /// ```
    ///
    /// # Errors
    ///
    /// - If the exit status of running the command is non-zero
    pub fn output(&self) -> Result<Output, CommandError> {
        self.command()
            .output()
            .or_else(|error| {
                Ok(Output {
                    stdout: String::new().into_bytes(),
                    stderr: format!("{error}").into_bytes(),
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
                        already_streamed: false,
                    })
                }
            })
    }

    /// Runs the command and streams contents to STDOUT/STDERR
    ///
    /// ```rust,no_run
    /// use commons::env_command::{EnvCommand, OutputEx};
    /// use libcnb::Env;
    ///
    /// let env = Env::new();
    /// let mut command = EnvCommand::new("echo", &["hello world"], &env);
    /// let outcome = command.stream().unwrap();
    ///
    /// assert_eq!(outcome.stdout_lossy().trim(), "hello world".to_string());
    /// ```
    ///
    /// # Errors
    ///
    /// - Err when the status code is non-zero
    pub fn stream(&self) -> Result<Output, CommandError> {
        self.command()
            .output_and_write_streams(std::io::stdout(), std::io::stderr())
            .or_else(|error| {
                Ok(Output {
                    stdout: String::new().into_bytes(),
                    stderr: format!("{error}").into_bytes(),
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
                        already_streamed: true,
                    })
                }
            })
    }

    /// By default non-zero-exit codes return an error.
    /// use this method to implement custom behavior.
    ///
    /// ```rust,no_run
    /// use commons::env_command::{EnvCommand, OutputEx};
    /// use libcnb::Env;
    ///
    /// let env = Env::new();
    /// let mut outcome = EnvCommand::new("iDoNotExist", &["hello world"], &env)
    ///                   .on_non_zero_exit(|error| Ok(error.result) )
    ///                   .output()
    ///                   .unwrap();
    ///
    /// assert!(!outcome.status.success());
    /// assert!(outcome.stderr_lossy().contains("command not found: iDoNotExist"))
    /// ```
    pub fn on_non_zero_exit(
        &mut self,
        fun: impl Fn(NonZeroExitStatusError) -> Result<Output, NonZeroExitStatusError> + 'static,
    ) -> &mut Self {
        self.on_non_zero_exit = Box::new(fun);
        self
    }

    /// The user can specify an custom function to be called when
    /// a non-zero exit status is encountered.
    ///
    /// This is a helper function to invoke that behavior.
    fn handle_non_zero_exit_error(
        &self,
        error: NonZeroExitStatusError,
    ) -> Result<Output, CommandError> {
        (self.on_non_zero_exit)(error).map_err(CommandError::UnexpectedExitStatusError)
    }

    pub fn to_string<T: Borrow<HashMap<OsString, OsString>>>(
        command: &Command,
        show_env_keys: impl Iterator<Item = impl Into<OsString>>,
        env: T,
    ) -> String {
        let escape_pattern = Regex::new(r"([^A-Za-z0-9_\-.,:/@\n])") // https://github.com/jimmycuadra/rust-shellwords/blob/d23b853a850ceec358a4137d5e520b067ddb7abc/src/lib.rs#L23
            .expect("Internal error: Bad Regex"); // Checked via clippy lint https://rust-lang.github.io/rust-clippy/master/index.html#invalid_regex
        let env = env.borrow();

        // Env keys
        show_env_keys
            .map(|key| {
                let key = key.into();
                format!(
                    "{}={:?}",
                    key.to_string_lossy(),
                    env.get(&key).cloned().unwrap_or_else(|| OsString::from(""))
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
                            format!("{arg:?}")
                        } else {
                            format!("{arg}")
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
    use libcnb::Env;

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
        let outcome = command.output().unwrap();

        assert_eq!(outcome.stdout_lossy().trim(), "hello world".to_string());
    }

    #[test]
    fn runs_non_zero_exit_error() {
        // Platform dependent
        let mut command = EnvCommand::new("exit", &["123"], &Env::new());
        let outcome = command.output();

        assert!(outcome.is_err());

        // Ignore the error
        command.on_non_zero_exit(|err| Ok(err.result));
        command.output().unwrap();
    }

    #[test]
    fn runs_command_and_captures_stdout_while_streaming_to_stdout_stderr() {
        // Platform dependent
        let command = EnvCommand::new("echo", &["hello world"], &Env::new());
        let outcome = command.stream().unwrap();

        assert_eq!(outcome.stdout_lossy().trim(), "hello world".to_string());
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

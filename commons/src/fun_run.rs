use lazy_static::lazy_static;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fmt::{Debug, Display};
use std::process::{Command, Output};
use which_problem::Which;

#[cfg(test)]
use libherokubuildpack as _;

/// The `fun_run` module is designed to make running commands more fun for you
/// and your users.
///
/// Fun runs are easy to understand when they work, and easier to debug when
/// they fail.
///
/// Fun runs make it easy to:
///
/// - Advertise the command being run before execution
/// - Customize how commands are displayed
/// - Return error messages with the command name.
/// - Turn non-zero status results into an error
/// - Embed stdout and stderr into errors (when not streamed)
/// - Store stdout and stderr without displaying them (when streamed)
///
/// Even better:
///
/// - Composable by design. Use what you want. Ignore what you don't.
/// - Plays well with standard library types by default.
///
/// And of course:
///
/// - Fun(ctional)
///
/// While the pieces can be composed functionally the real magic comes when you start mixing in the helper structs `NamedOutput` and `CmdError`.
/// Together these will return a Result type that contains the associated name of the command just called: `Result<NamedOutput, CmdError>`.
///
/// Trait extensions:
///
///   - `Result<T,E>::cmd_map()` provided by `CmdMapExt`
///   - `Result<Output,std::io::Error>::with_name` provided by the `ResultNameExt`.
///
/// Use these along with other fun methods to compose the command run of your dreams.
///
/// Example:
///
/// ```no_run
/// use commons::fun_run::{self, CmdMapExt, ResultNameExt};
/// use libherokubuildpack::command::CommandExt;
/// use std::process::Command;
/// use libcnb::Env;
///
/// let env = Env::new();
///
/// Command::new("bundle")
///     .args(["install"])
///     .envs(&env)
///     .cmd_map(|cmd| {
///         let name = fun_run::display(cmd);
///         eprintln!("\nRunning command:\n$ {name}");
///
///         cmd.output_and_write_streams(std::io::stdout(), std::io::stderr())
///             .with_name(name) // Converts Result<Output, std::io::Error> to Result<NamedOutput, CmdError>
///             .and_then(fun_run::NamedOutput::nonzero_streamed) // Converts `Ok` to `Err` if `NamedOutput` status is not zero
///     }).unwrap();
/// ```

/// Allows for a functional-style flow when running a `Command` via
/// providing `cmd_map`
pub trait CmdMapExt<O, F>
where
    F: Fn(&mut Command) -> O,
{
    fn cmd_map(&mut self, f: F) -> O;
}

impl<O, F> CmdMapExt<O, F> for Command
where
    F: Fn(&mut Command) -> O,
{
    /// Acts like `Iterator.map` on a `Command`
    ///
    /// Yields its self and returns whatever output the block returns.
    fn cmd_map(&mut self, f: F) -> O {
        f(self)
    }
}

lazy_static! {
    // https://github.com/jimmycuadra/rust-shellwords/blob/d23b853a850ceec358a4137d5e520b067ddb7abc/src/lib.rs#L23
    static ref QUOTE_ARG_RE: regex::Regex = regex::Regex::new(r"([^A-Za-z0-9_\-.,:/@\n])").expect("Internal error:");
}

/// Converts a command and its arguments into a user readable string
///
/// Example
///
/// ```rust
/// use std::process::Command;
/// use commons::fun_run;
///
/// let name = fun_run::display(Command::new("bundle").arg("install"));
/// assert_eq!(String::from("bundle install"), name);
/// ```
#[must_use]
pub fn display(command: &mut Command) -> String {
    vec![command.get_program().to_string_lossy().to_string()]
        .into_iter()
        .chain(
            command
                .get_args()
                .map(std::ffi::OsStr::to_string_lossy)
                .map(|arg| {
                    if QUOTE_ARG_RE.is_match(&arg) {
                        format!("{arg:?}")
                    } else {
                        format!("{arg}")
                    }
                }),
        )
        .collect::<Vec<String>>()
        .join(" ")
}

/// Converts a command, arguments, and specified environment variables to user readable string
///
///
/// Example
///
/// ```rust
/// use std::process::Command;
/// use commons::fun_run;
/// use libcnb::Env;
///
/// let mut env = Env::new();
/// env.insert("RAILS_ENV", "production");

///
/// let mut command = Command::new("bundle");
/// command.arg("install").envs(&env);
///
/// let name = fun_run::display_with_env_keys(&mut command, &env, ["RAILS_ENV"]);
/// assert_eq!(String::from(r#"RAILS_ENV="production" bundle install"#), name);
/// ```
#[must_use]
pub fn display_with_env_keys<E, K, V, I, O>(cmd: &mut Command, env: E, keys: I) -> String
where
    E: IntoIterator<Item = (K, V)>,
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
    I: IntoIterator<Item = O>,
    O: AsRef<OsStr>,
{
    let env_hash = env
        .into_iter()
        .map(|(k, v)| (k.as_ref().to_owned(), v.as_ref().to_owned()))
        .collect::<HashMap<OsString, OsString>>();

    keys.into_iter()
        .map(|key| {
            let key = key.as_ref();
            format!(
                "{}={:?}",
                key.to_string_lossy(),
                env_hash
                    .get(key)
                    .cloned()
                    .unwrap_or_else(|| OsString::from(""))
            )
        })
        .chain([display(cmd)])
        .collect::<Vec<String>>()
        .join(" ")
}

/// Adds diagnostic information to a `CmdError` using `which_problem` if error is `std::io::Error`
///
/// This feature is experimental
pub fn map_which_problem(
    error: CmdError,
    cmd: &mut Command,
    path_env: Option<OsString>,
) -> CmdError {
    match error {
        CmdError::SystemError(name, error) => {
            CmdError::SystemError(name, annotate_which_problem(error, cmd, path_env))
        }
        CmdError::NonZeroExitNotStreamed(_) | CmdError::NonZeroExitAlreadyStreamed(_) => error,
    }
}

/// Adds diagnostic information to an `std::io::Error` using `which_problem`
///
/// This feature is experimental
#[must_use]
pub fn annotate_which_problem(
    error: std::io::Error,
    cmd: &mut Command,
    path_env: Option<OsString>,
) -> std::io::Error {
    let program = cmd.get_program().to_os_string();
    let current_working_dir = cmd.get_current_dir().map(std::path::Path::to_path_buf);
    let problem = Which {
        cwd: current_working_dir,
        program,
        path_env,
        ..Which::default()
    }
    .diagnose();

    let annotation = match problem {
        Ok(details) => format!("\nSystem diagnostic information:\n\n{details}"),
        Err(error) => format!("\nInternal error while gathering dianostic information:\n\n{error}"),
    };

    annotate_io_error(error, annotation)
}

/// Returns an IO error that displays the given annotation starting on
/// the next line.
///
/// Internal API used by `annotate_which_problem`
#[must_use]
fn annotate_io_error(source: std::io::Error, annotation: String) -> std::io::Error {
    crate::err::IoErrorAnnotation::new(source, annotation).into_io_error()
}

/// Who says (`Command`) errors can't be fun?
///
/// Fun run errors include all the info a user needs to debug, like
/// the name of the command that failed and any outputs (like error messages
/// in stderr).
///
/// Fun run errors don't overwhelm end users, so by default if stderr is already
/// streamed the output won't be duplicated.
///
/// Enjoy if you want, skip if you don't. Fun run errors are not mandatory.
///
/// Error output formatting is unstable
#[derive(Debug, thiserror::Error)]
#[allow(clippy::module_name_repetitions)]
pub enum CmdError {
    #[error("Could not run command command {0:?}. Details: {1}")]
    SystemError(String, std::io::Error),

    #[error("Command failed: {0:?}\nexit status: {}\nstdout: {}\nstderr: {}", .0.output.status.code().unwrap_or_else(|| 1),  display_out_or_empty(&.0.output.stdout), display_out_or_empty(&.0.output.stderr))]
    NonZeroExitNotStreamed(NamedOutput),

    #[error("Command failed: {0:?}\nexit status: {}\nstdout: <see above>\nstderr: <see above>", .0.output.status.code().unwrap_or_else(|| 1))]
    NonZeroExitAlreadyStreamed(NamedOutput),
}

impl CmdError {
    /// Returns a display representation of the command that failed
    ///
    /// Example:
    ///
    /// ```no_run
    /// use commons::fun_run::{self, CmdMapExt};
    /// use std::process::Command;
    ///
    /// let result = Command::new("cat")
    ///     .arg("mouse.txt")
    ///     .cmd_map(fun_run::quick::output);
    ///
    /// match result {
    ///     Ok(_) => todo!(),
    ///     Err(error) => assert_eq!(error.name().to_string(), "cat mouse.txt")
    /// }
    /// ```
    #[must_use]
    pub fn name(&self) -> std::borrow::Cow<'_, str> {
        match self {
            CmdError::SystemError(name, _) => name.into(),
            CmdError::NonZeroExitNotStreamed(out) | CmdError::NonZeroExitAlreadyStreamed(out) => {
                out.name.as_str().into()
            }
        }
    }
}

impl TryFrom<CmdError> for NamedOutput {
    type Error = CmdError;

    fn try_from(value: CmdError) -> Result<Self, Self::Error> {
        match value {
            CmdError::SystemError(_, _) => Err(value),
            CmdError::NonZeroExitNotStreamed(named)
            | CmdError::NonZeroExitAlreadyStreamed(named) => Ok(named),
        }
    }
}

/// Converts a `std::io::Error` into a `CmdError` which includes the formatted command name
#[must_use]
pub fn on_system_error(name: String, error: std::io::Error) -> CmdError {
    CmdError::SystemError(name, error)
}

fn display_out_or_empty(contents: &[u8]) -> String {
    let contents = String::from_utf8_lossy(contents);
    if contents.trim().is_empty() {
        "<empty>".to_string()
    } else {
        contents.to_string()
    }
}

/// Converts an `Output` into an error when status is non-zero
///
/// When calling a `Command` and streaming the output to stdout/stderr
/// it can be jarring to have the contents emitted again in the error. When this
/// error is displayed those outputs will not be repeated.
///
/// Use when the `Output` comes from a source that was already streamed.
///
/// To to include the results of stdout/stderr in the display of the error
/// use `nonzero_captured` instead.
///
/// # Errors
///
/// Returns Err when the `Output` status is non-zero
pub fn nonzero_streamed(name: String, output: impl Into<Output>) -> Result<NamedOutput, CmdError> {
    let output = output.into();
    if output.status.success() {
        Ok(NamedOutput { name, output })
    } else {
        Err(CmdError::NonZeroExitAlreadyStreamed(NamedOutput {
            name,
            output,
        }))
    }
}

/// Converts an `Output` into an error when status is non-zero
///
/// Use when the `Output` comes from a source that was not streamed
/// to stdout/stderr so it will be included in the error display by default.
///
/// To avoid double printing stdout/stderr when streaming use `nonzero_streamed`
///
/// # Errors
///
/// Returns Err when the `Output` status is non-zero
pub fn nonzero_captured(name: String, output: impl Into<Output>) -> Result<NamedOutput, CmdError> {
    let output = output.into();
    if output.status.success() {
        Ok(NamedOutput { name, output })
    } else {
        Err(CmdError::NonZeroExitNotStreamed(NamedOutput {
            name,
            output,
        }))
    }
}

/// Holds a the `Output` of a command's execution along with it's "name"
///
/// When paired with `CmdError` a `Result<NamedOutput, CmdError>` will retain the
/// "name" of the command regardless of succss or failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedOutput {
    pub name: String,
    pub output: Output,
}

/// Easilly convert command output into a result with names
///
/// Associated function name is experimental and may change
pub trait ResultNameExt {
    /// # Errors
    ///
    /// Returns a `CmdError::SystemError` if the original Result was `Err`.
    fn with_name(self, name: impl AsRef<str>) -> Result<NamedOutput, CmdError>;
}

/// Convert the value of `Command::output()` into `Result<Output, std::io::Error>`
impl ResultNameExt for Result<Output, std::io::Error> {
    /// # Errors
    ///
    /// Returns a `CmdError::SystemError` if the original Result was `Err`.
    fn with_name(self, name: impl AsRef<str>) -> Result<NamedOutput, CmdError> {
        let name = name.as_ref();
        self.map_err(|io_error| CmdError::SystemError(name.to_string(), io_error))
            .map(|output| NamedOutput {
                name: name.to_string(),
                output,
            })
    }
}

impl NamedOutput {
    /// # Errors
    ///
    /// Returns an error if the status is not zero
    pub fn nonzero_captured(self) -> Result<NamedOutput, CmdError> {
        nonzero_captured(self.name, self.output)
    }

    /// # Errors
    ///
    /// Returns an error if the status is not zero
    pub fn nonzero_streamed(self) -> Result<NamedOutput, CmdError> {
        nonzero_streamed(self.name, self.output)
    }
}

impl From<NamedOutput> for Output {
    fn from(value: NamedOutput) -> Self {
        value.output
    }
}

/// Experimental: Annotate commands with other commands when they fail
///
/// API subject to change.
///
/// ```no_run
/// use std::process::Command;
/// use commons::fun_run::{self, CmdMapExt, CmdErrorDiagnostics};
/// use libcnb::Env;
///
/// let env = Env::new();
///
/// Command::new("bundle")
///     .arg("list")
///     .env_clear()
///     .envs(&env)
///     .cmd_map(|cmd| {
///         let name = fun_run::display(cmd);
///
///         cmd.output()
///            .map_err(|error| fun_run::on_system_error(name.clone(), error))
///            .and_then(|output| fun_run::nonzero_captured(name.clone(), output))
///            .map_err(|error| {
///                 CmdErrorDiagnostics::new(error)
///                     .run_and_insert(Command::new("bundle").arg("env").env_clear().envs(&env))
///                     .run_and_insert(Command::new("gem").arg("env").env_clear().envs(&env))
///            })
///     });
/// ```
/// Experimental interface, subject to change
pub type CmdErrorDiagnostics = ErrorDiagnostics<CmdError>;

/// Experimental interface, subject to change
#[derive(Debug)]
pub struct ErrorDiagnostics<E: Display + Debug> {
    pub error: E,
    pub diagnostics: Vec<Result<NamedOutput, CmdError>>,
}

impl<E: Display + Debug> ErrorDiagnostics<E> {
    #[must_use]
    pub fn new(error: E) -> Self {
        let diagnostics = Vec::new();
        Self { error, diagnostics }
    }

    #[must_use]
    pub fn run_and_insert(mut self, cmd: &mut Command) -> Self {
        let name = display(cmd);

        let result = cmd
            .output()
            .with_name(name)
            .and_then(NamedOutput::nonzero_captured);

        self.diagnostics.push(result);
        self
    }
}

impl<E: Display + Debug> Display for ErrorDiagnostics<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for diagnostic in &self.diagnostics {
            writeln!(f, "{}\n\n", self.error)?;

            f.write_str("System diagnostic information:\n\n")?;
            match &diagnostic {
                Ok(named) => {
                    let name = &named.name;
                    let output = &named.output;
                    let status = output.status;
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);

                    writeln!(
                        f,
                        "Command: {name}\nstatus: {status}\nstdout: {stdout}\n stderr: {stderr}"
                    )?;
                }
                Err(error) => writeln!(f, "{error}")?,
            }
        }

        Ok(())
    }
}

/// Experimental module for common operations with opinonated defaults
///
/// API subject to change
pub mod quick {
    use super::{display, CmdError, Command, NamedOutput, ResultNameExt};
    use libherokubuildpack::command::CommandExt;

    /// # Errors
    ///
    /// Returns an error if the system errors or if
    /// the command returns a non-zero exit code.
    pub fn stream(command: &mut Command) -> Result<NamedOutput, CmdError> {
        command
            .output_and_write_streams(std::io::stdout(), std::io::stderr())
            .with_name(display(command))
            .and_then(NamedOutput::nonzero_streamed)
    }

    /// # Errors
    ///
    /// Returns an error if the system errors or if
    /// the command returns a non-zero exit code.
    pub fn output(command: &mut Command) -> Result<NamedOutput, CmdError> {
        command
            .output()
            .with_name(display(command))
            .and_then(NamedOutput::nonzero_captured)
    }
}

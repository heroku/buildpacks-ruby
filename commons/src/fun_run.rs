use lazy_static::lazy_static;
use libherokubuildpack::command::CommandExt;
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::process::ExitStatusExt;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Output;
use which_problem::Which;

#[cfg(test)]
use libherokubuildpack as _;

use crate::fun_run;

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
/// While the pieces can be composed functionally the real magic comes when you start mixing in the helper structs `NamedCommand`, `NamedOutput` and `CmdError`.
/// Together these will return a Result type that contains the associated name of the command just called: `Result<NamedOutput, CmdError>`.
///
/// Example:
///
/// ```no_run
/// use commons::fun_run::CommandWithName;
/// use std::process::Command;
/// use libcnb::Env;
///
/// let env = Env::new();
///
/// let result = Command::new("bundle")
///     .args(["install"])
///     .envs(&env)
///     .stream_output(std::io::stdout(), std::io::stderr());
///
/// match result {
///     Ok(output) => {
///         assert_eq!("bundle install", &output.name())
///     },
///     Err(varient) => {
///         assert_eq!("bundle install", &varient.name())
///     }
/// }
/// ```
///
/// Change names as you see fit:
///
/// ```no_run
/// use commons::fun_run::CommandWithName;
/// use std::process::Command;
/// use libcnb::Env;
///
/// let env = Env::new();
///
/// let result = Command::new("gem")
///     .args(["install", "bundler", "-v", "2.4.1.7"])
///     .envs(&env)
///     // Overwrites default command name which would include extra arguments
///     .named("gem install")
///     .stream_output(std::io::stdout(), std::io::stderr());
///
/// match result {
///     Ok(output) => {
///         assert_eq!("bundle install", &output.name())
///     },
///     Err(varient) => {
///         assert_eq!("bundle install", &varient.name())
///     }
/// }
/// ```
///
/// Or include env vars:
///
/// ```no_run
/// use commons::fun_run::{self, CommandWithName};
/// use std::process::Command;
/// use libcnb::Env;
///
/// let env = Env::new();
///
/// let result = Command::new("gem")
///     .args(["install", "bundler", "-v", "2.4.1.7"])
///     .envs(&env)
///     // Overwrites default command name
///     .named_fn(|cmd| {
///         // Annotate command with GEM_HOME env var
///         fun_run::display_with_env_keys(cmd, &env, ["GEM_HOME"])
///     })
///     .stream_output(std::io::stdout(), std::io::stderr());
///
/// match result {
///     Ok(output) => {
///         assert_eq!("GEM_HOME=\"/usr/bin/local/.gems\" gem install bundler -v 2.4.1.7", &output.name())
///     },
///     Err(varient) => {
///         assert_eq!("GEM_HOME=\"/usr/bin/local/.gems\" gem install bundler -v 2.4.1.7", &varient.name())
///     }
/// }
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

pub trait CommandWithName {
    fn name(&mut self) -> String;
    fn mut_cmd(&mut self) -> &mut Command;

    fn named(&mut self, s: impl AsRef<str>) -> NamedCommand<'_> {
        let name = s.as_ref().to_string();
        let command = self.mut_cmd();
        NamedCommand { name, command }
    }

    #[allow(clippy::needless_lifetimes)]
    fn named_fn<'a>(&'a mut self, f: impl FnOnce(&mut Command) -> String) -> NamedCommand<'a> {
        let cmd = self.mut_cmd();
        let name = f(cmd);
        self.named(name)
    }

    /// Runs the command without streaming
    ///
    /// # Errors
    ///
    /// Returns `CmdError::SystemError` if the system is unable to run the command.
    /// Returns `CmdError::NonZeroExitNotStreamed` if the exit code is not zero.
    fn named_output(&mut self) -> Result<NamedOutput, CmdError> {
        let name = self.name();
        self.mut_cmd()
            .output()
            .with_name(name)
            .and_then(NamedOutput::nonzero_captured)
    }

    /// Runs the command and streams to the given writers
    ///
    /// # Errors
    ///
    /// Returns `CmdError::SystemError` if the system is unable to run the command
    /// Returns `CmdError::NonZeroExitAlreadyStreamed` if the exit code is not zero.
    fn stream_output<OW, EW>(
        &mut self,
        stdout_write: OW,
        stderr_write: EW,
    ) -> Result<NamedOutput, CmdError>
    where
        OW: Write + Send,
        EW: Write + Send,
    {
        let name = &self.name();
        self.mut_cmd()
            .output_and_write_streams(stdout_write, stderr_write)
            .with_name(name)
            .and_then(NamedOutput::nonzero_streamed)
    }
}

impl CommandWithName for Command {
    fn name(&mut self) -> String {
        fun_run::display(self)
    }

    fn mut_cmd(&mut self) -> &mut Command {
        self
    }
}

/// It's a command, with a name
pub struct NamedCommand<'a> {
    name: String,
    command: &'a mut Command,
}

impl CommandWithName for NamedCommand<'_> {
    fn name(&mut self) -> String {
        self.name.to_string()
    }

    fn mut_cmd(&mut self) -> &mut Command {
        self.command
    }
}

/// Holds a the `Output` of a command's execution along with it's "name"
///
/// When paired with `CmdError` a `Result<NamedOutput, CmdError>` will retain the
/// "name" of the command regardless of succss or failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedOutput {
    name: String,
    output: Output,
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

    #[must_use]
    pub fn status(&self) -> &ExitStatus {
        &self.output.status
    }

    #[must_use]
    pub fn stdout_lossy(&self) -> String {
        String::from_utf8_lossy(&self.output.stdout).to_string()
    }

    #[must_use]
    pub fn stderr_lossy(&self) -> String {
        String::from_utf8_lossy(&self.output.stderr).to_string()
    }

    #[must_use]
    pub fn name(&self) -> String {
        self.name.clone()
    }
}

impl AsRef<Output> for NamedOutput {
    fn as_ref(&self) -> &Output {
        &self.output
    }
}

impl From<NamedOutput> for Output {
    fn from(value: NamedOutput) -> Self {
        value.output
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
    K: Into<OsString>,
    V: Into<OsString>,
    I: IntoIterator<Item = O>,
    O: Into<OsString>,
{
    let env = env
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect::<std::collections::HashMap<OsString, OsString>>();

    keys.into_iter()
        .map(|key| {
            let key = key.into();
            format!(
                "{}={:?}",
                key.to_string_lossy(),
                env.get(&key).cloned().unwrap_or_else(|| OsString::from(""))
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
    #[error("Could not run command `{0}`. {1}")]
    SystemError(String, std::io::Error),

    #[error("Command failed: `{cmd}`\nexit status: {status}\nstdout: {stdout}\nstderr: {stderr}", cmd = .0.name, status = .0.output.status.code().unwrap_or_else(|| 1),  stdout = display_out_or_empty(&.0.output.stdout), stderr = display_out_or_empty(&.0.output.stderr))]
    NonZeroExitNotStreamed(NamedOutput),

    #[error("Command failed: `{cmd}`\nexit status: {status}\nstdout: <see above>\nstderr: <see above>", cmd = .0.name, status = .0.output.status.code().unwrap_or_else(|| 1))]
    NonZeroExitAlreadyStreamed(NamedOutput),
}

impl CmdError {
    /// Returns a display representation of the command that failed
    ///
    /// Example:
    ///
    /// ```no_run
    /// use commons::fun_run::{self, CmdMapExt, ResultNameExt};
    /// use std::process::Command;
    ///
    /// let result = Command::new("cat")
    ///     .arg("mouse.txt")
    ///     .cmd_map(|cmd| cmd.output().with_name(fun_run::display(cmd)));
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

impl From<CmdError> for NamedOutput {
    fn from(value: CmdError) -> Self {
        match value {
            CmdError::SystemError(name, error) => NamedOutput {
                name,
                output: Output {
                    status: ExitStatus::from_raw(error.raw_os_error().unwrap_or(-1)),
                    stdout: Vec::new(),
                    stderr: error.to_string().into_bytes(),
                },
            },
            CmdError::NonZeroExitNotStreamed(named)
            | CmdError::NonZeroExitAlreadyStreamed(named) => named,
        }
    }
}

fn display_out_or_empty(contents: &[u8]) -> String {
    let contents = String::from_utf8_lossy(contents);
    if contents.trim().is_empty() {
        "<empty>".to_string()
    } else {
        contents.to_string()
    }
}

/// Converts a `std::io::Error` into a `CmdError` which includes the formatted command name
#[must_use]
pub fn on_system_error(name: String, error: std::io::Error) -> CmdError {
    CmdError::SystemError(name, error)
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

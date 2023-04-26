use lazy_static::lazy_static;
use std::ffi::OsString;
use std::process::Command;
use std::process::Output;
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
/// The main interface is the `name_and_then` method provided by the `CommandNameAndThen` extension.
/// Use this along with other fun methods to compose the command run of your dreams.
///
/// Example:
///
/// ```no_run
/// use commons::fun_run::{ self, CommandNameAndThen};
/// use libherokubuildpack::command::CommandExt;
/// use std::process::Command;
/// use libcnb::Env;
///
/// let env = Env::new();
///
/// Command::new("bundle")
///     .args(["install"])
///     .envs(&env)
///     .name_and_then(fun_run::display, |name, cmd| {
///         eprintln!("\nRunning command:\n$ {name}");
///
///         cmd.output_and_write_streams(std::io::stdout(), std::io::stderr())
///             .map_err(|error| fun_run::on_system_error(name.clone(), error))
///             .and_then(|output| fun_run::nonzero_streamed(name.clone(), output))
///     }).unwrap();
/// ```

/// Format a command in a single location use it for display or errors
pub trait CommandNameAndThen<N, G, O, E>
where
    N: Fn(&mut Command) -> String,
    G: FnOnce(String, &mut Command) -> Result<O, E>,
{
    /// Takes two functions the first produces a name
    /// and is then passed to the section function along with
    /// an `&mut Command`
    ///
    /// The second function is intended to return
    /// `Result<Output, std::io::Error>` or `Result<Output, CmdError>`, but can
    /// return any result.
    ///
    /// # Errors
    ///
    /// Returns the result of the second function directly.
    ///
    fn name_and_then(&mut self, get_name: N, f: G) -> Result<O, E>;
}

impl<N, G, O, E> CommandNameAndThen<N, G, O, E> for Command
where
    N: Fn(&mut Command) -> String,
    G: FnOnce(String, &mut Command) -> Result<O, E>,
{
    fn name_and_then(&mut self, get_name: N, f: G) -> Result<O, E> {
        let name = get_name(self);
        f(name, self)
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

    #[error("Command failed {0:?}.\nstatus: {}\nstdout: {}\nstderr: {}", .1.status,  String::from_utf8_lossy(&.1.stdout), String::from_utf8_lossy(&.1.stderr))]
    NonZeroExitNotStreamed(String, Output),

    #[error("Command failed {0:?}.\nstatus: {}\nstdout: see above\nstderr: see above", .1.status)]
    NonZeroExitAlreadyStreamed(String, Output),
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
pub fn nonzero_streamed(name: String, output: Output) -> Result<Output, CmdError> {
    if output.status.success() {
        Ok(output)
    } else {
        Err(CmdError::NonZeroExitAlreadyStreamed(name, output))
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
pub fn nonzero_captured(name: String, output: Output) -> Result<Output, CmdError> {
    if output.status.success() {
        Ok(output)
    } else {
        Err(CmdError::NonZeroExitNotStreamed(name, output))
    }
}

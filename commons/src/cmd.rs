use lazy_static::lazy_static;
use std::ffi::OsStr;
use std::fmt::Display;
use std::os::unix::process::ExitStatusExt;
use std::process::Command;
use std::process::Output;
use std::{ffi::OsString, process::ExitStatus};

/// Display `Command` and deliver consistent errors
///
///
/// Return an error if non-zero output:
///
/// ```rust
/// # use std::process::Command;
/// use commons::cmd;
///
/// # fn fun(command: &mut Command) {
///     command.output()
///         .map_err(cmd::os_command_error)
///         .and_then(|output| cmd::check_non_zero(output, cmd::OutputState::IncludeInError))
///         .unwrap();
/// # }
/// ```
///
/// Wrap `OutputError` with configurable command name via `CmdError::new`:
///
/// ```rust
/// # use std::process::Command;
/// use commons::{cmd, cmd::CmdError};
///
/// # fn fun(command: &mut Command) {
///     let cmd_str = command.get_program().to_string_lossy().to_string();
///
///     command.output()
///         .map_err(cmd::os_command_error)
///         .and_then(|output| cmd::check_non_zero(output, cmd::OutputState::IncludeInError))
///         .map_err(|error| CmdError::new(cmd_str, error))
///         .unwrap();
/// # }
///
/// ```
///
/// Use `display` and `display_with_keys` to format
/// commands for user output:
///
/// ```rust
/// # let mut env = std::collections::HashMap::new();
/// # env.insert("BUNDLE_PATH", "/layers/gems");
/// use std::process::Command;
/// use commons::cmd;
///
/// let mut command = Command::new("bundle");
/// command.args(["install"]);
///
/// let cmd_str = cmd::display(&command);
/// assert_eq!("Running $ bundle install", &format!("Running $ {cmd_str}"));
///
/// let cmd_str = cmd::display_with_keys(&command, &env, ["BUNDLE_PATH"]);
/// assert_eq!(r#"Running $ BUNDLE_PATH="/layers/gems" bundle install"#, &format!("Running $ {cmd_str}"));
/// ```
///
/// All together:
///
/// ```rust
/// use libherokubuildpack::command::CommandExt;
/// use commons::cmd::CmdError;
/// use commons::cmd;
///
/// fn bundle_install(env: &libcnb::Env) -> Result<(), CmdError> {
///     // ## Run `$ bundle install`
///
///     let mut command = cmd::create("bundle", &["install"], env);
///     let display = cmd::display_with_keys(
///         &command,
///         env,
///         [
///             "BUNDLE_BIN",
///             "BUNDLE_CLEAN",
///             "BUNDLE_DEPLOYMENT",
///             "BUNDLE_GEMFILE",
///             "BUNDLE_PATH",
///             "BUNDLE_WITHOUT",
///         ],
///     );
///
///     println!("\nRunning command:\n$ {display}");
///
///     command.output_and_write_streams(std::io::stdout(), std::io::stderr())
///         .map_err(cmd::os_command_error)
///         .and_then(|output| cmd::check_non_zero(output, cmd::OutputState::AlreadyStreamed))
///         .map_err(|error| cmd::CmdError::new(display, error))?;
///
///     Ok(())
/// }
///```

/// Indicate the state of the current `Output` value
///
/// Depending on how a command is executed it may
/// have already streamed any errors to the user's display.
///
/// We want to know this so we don't double render error
/// information
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputState {
    /// Indicates stdout and stderr are already available
    /// to the user, to not include again in the
    /// error `Display`
    AlreadyStreamed,

    /// Indicates stdout and stderr have NOT been
    /// streamed to the user. When an error
    /// is built using this value, add the
    /// stdout and stderr contents to it's `Display`
    IncludeInError,
}

lazy_static! {
    // https://github.com/jimmycuadra/rust-shellwords/blob/d23b853a850ceec358a4137d5e520b067ddb7abc/src/lib.rs#L23
    static ref QUOTE_ARG_RE: regex::Regex = regex::Regex::new(r"([^A-Za-z0-9_\-.,:/@\n])").expect("Internal error:");
}

/// Converts a command and it's arguments into a user readable string
///
/// ```rust
/// use std::process::Command;
/// use commons::cmd;
///
/// let mut command = Command::new("echo");
/// command.args(&["hello world"]);
///
/// let command_str = cmd::display(&command);
/// assert_eq!(r#"echo "hello world""#, &command_str);
/// ```
pub fn display(command: &Command) -> String {
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
/// ```rust
/// use commons::cmd;
/// let mut env = libcnb::Env::new();
/// env.insert("PATH", "i_can_show_you");
///
/// let mut command = cmd::create("echo", &["the world"], &env);
/// let command_str = cmd::display_with_keys(&command, &env, ["PATH"]);
/// assert_eq!(r#"PATH="i_can_show_you" echo "the world""#, &command_str);
/// ```
pub fn display_with_keys<E, K, V, I, O>(command: &Command, env: E, show_keys: I) -> String
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

    show_keys
        .into_iter()
        .map(|key| {
            let key = key.into();
            format!(
                "{}={:?}",
                key.to_string_lossy(),
                env.get(&key).cloned().unwrap_or_else(|| OsString::from(""))
            )
        })
        .chain([display(command)])
        .collect::<Vec<String>>()
        .join(" ")
}

/// Build a command from args and env vars
///
/// This mostly exists so developers don't forget to apply
/// environment variables to their program by accident.
pub fn create<I, K, V>(program: &str, args: &[&str], env: I) -> Command
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    let mut command = Command::new(program);
    command.args(args);
    command.envs(env);
    command
}

/// Convert an os command error into formatted error
///
/// When running commands the operating system may return
/// a `std::io::Error` for example if it cannot find
/// the executable i.e. "No such file or directory" error.
///
/// The `OutputError::OsError` is formatted consistently with
/// other user errors.
#[must_use]
pub fn os_command_error(error: std::io::Error) -> OutputError {
    let code = error.raw_os_error().unwrap_or(-1);
    OutputError::OsError {
        status: ExitStatus::from_raw(code),
        source: error,
    }
}

/// Checks if an `Output` is non-zero to return an error
///
/// # Errors
///
/// If the output status is non-zero it will return an `OutputError`
/// based on the variant of `OutputContents` received.
pub fn check_non_zero(output: Output, output_state: OutputState) -> Result<Output, OutputError> {
    if output.status.success() {
        Ok(output)
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let status = output.status;

        Err(OutputError::NonZeroExit {
            stdout,
            stderr,
            status,
            output_state,
        })
    }
}

/// Contains error details that can be derived
/// from a command's `Output` value or it's
/// error state.
///
/// Notably none of these values contain the
/// a representation of command that was just
/// run. This is intentional.
///
/// The variant from this enum are intended to be paired
/// with a command to form a `CmdError`.
#[derive(Debug)]
pub enum OutputError {
    OsError {
        status: ExitStatus,
        source: std::io::Error,
    },
    NonZeroExit {
        status: ExitStatus,
        stdout: String,
        stderr: String,
        output_state: OutputState,
    },
}

impl Display for OutputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputError::OsError { status, source } => {
                writeln!(f, "status: {status}\nerror: {source}")
            }
            OutputError::NonZeroExit {
                status,
                stdout,
                stderr,
                output_state
            } => {
                match output_state {
                    OutputState::AlreadyStreamed =>
                writeln!(f, "status: {status}\nstdout: contents streamed above\nstderr: contents streamed above"),
                    OutputState::IncludeInError =>
                writeln!(f, "status: {status}\nstdout: {stdout}\nstderr: {stderr}"),
                }
            }
        }
    }
}

/// Combines an `OutputError` with a user representation
/// for the command just run.
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct CmdError {
    pub source: OutputError,
    pub command_string: String,
}

impl CmdError {
    #[must_use]
    pub fn new(command_string: String, error: OutputError) -> Self {
        Self {
            source: error,
            command_string,
        }
    }
}

impl std::fmt::Display for CmdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let CmdError {
            command_string,
            source,
        } = self;
        writeln!(f, "command failed: {command_string}")?;
        writeln!(f, "{source}")?;
        Ok(())
    }
}

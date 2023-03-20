use lazy_static::lazy_static;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fmt::Display;
use std::process::Command;
use std::process::Output;
use which_problem::Which;

/// Build a command from args and env vars
///
/// This mostly exists so developers don't forget to apply
/// environment variables to their program by accident.
pub fn plain<I, K, V>(program: &str, args: &[&str], env: I) -> Command
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

/// Cute commands have names!
///
/// Holds a named command. Name should be representative of
/// the command being run.
#[derive(Debug)]
pub struct CuteCmd<'a> {
    pub cmd: &'a mut Command,
    pub name: String,
}
impl Display for CuteCmd<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

impl<'a> CuteCmd<'a> {
    #[must_use]
    pub fn new_with(
        cmd: &'a mut Command,
        name_fun: impl FnOnce(&mut Command) -> String,
    ) -> CuteCmd<'a> {
        let name = name_fun(cmd);
        Self { cmd, name }
    }

    /// Hey cutie! Yields it's own name
    ///
    ///
    /// Useful for logging before running a command
    #[must_use]
    pub fn hello(self, f: impl FnOnce(&str)) -> CuteCmd<'a> {
        Self::yield_name(self, f)
    }

    /// Hey cutie! Yields it's own name
    ///
    /// Useful for logging before running a command
    #[must_use]
    pub fn yield_name(self, f: impl FnOnce(&str)) -> CuteCmd<'a> {
        f(&self.name);
        self
    }

    /// Run the command and stream output to current stdout/stderr
    #[must_use]
    pub fn stream(self) -> CuteCmdStatus<'a> {
        use libherokubuildpack::command::CommandExt;

        self.capture_output(|c| {
            Content::AlreadyStreamed(
                c.output_and_write_streams(std::io::stdout(), std::io::stderr()),
            )
        })
    }

    /// Run the command WITHOUT streaming the output to current stdout/stderr
    #[must_use]
    pub fn run_quietly(self) -> CuteCmdStatus<'a> {
        self.capture_output(|c| Content::NotStreamed(c.output()))
    }

    /// Bring your own `Command` execution. Run whatever code you want.
    ///
    /// `stream` and `run_quietly` not cute enough for ya? This method allows you to call any functionality
    /// that you want as long as it returns an `Output` value we can use.
    ///
    /// Wrap the return `Result<Output, std::io::Error>` in either `Contents::AlreadyStreamed`
    /// or `Contents::NotStreamed`. This will let us know later if stdout/stderr need to be
    /// in the error message or not (when non-zero).
    #[must_use]
    pub fn capture_output(self, f: impl FnOnce(&mut Command) -> Content) -> CuteCmdStatus<'a> {
        let CuteCmd { cmd, name } = self;
        let contents = f(cmd);

        match contents {
            Content::AlreadyStreamed(Ok(output)) | Content::NotStreamed(Ok(output))
                if output.status.success() =>
            {
                CuteCmdStatus::Ok(CuteCmdContent { cmd, name, output })
            }
            Content::AlreadyStreamed(Ok(output)) => {
                CuteCmdStatus::NonZeroExitAlreadyStreamed(CuteCmdContent { cmd, name, output })
            }
            Content::NotStreamed(Ok(output)) => {
                CuteCmdStatus::NonZeroExitNotStreamed(CuteCmdContent { cmd, name, output })
            }
            Content::AlreadyStreamed(Err(error)) | Content::NotStreamed(Err(error)) => {
                CuteCmdStatus::SystemError(CuteCmd { cmd, name }, error)
            }
        }
    }
}

/// Even a status can be cute!
///
/// After a command runs it will return a status.
/// Use this struct to implement your own logic or
/// call `to_result` to return the built in `CuteCmdError`
#[allow(clippy::module_name_repetitions)]
pub enum CuteCmdStatus<'a> {
    SystemError(CuteCmd<'a>, std::io::Error),
    NonZeroExitNotStreamed(CuteCmdContent<'a>),
    NonZeroExitAlreadyStreamed(CuteCmdContent<'a>),
    Ok(CuteCmdContent<'a>),
}

/// Cute errors are better
///
/// Cute errors include all the info a user needs to debug, like
/// the name of the command that failed and any outputs (like error messages in stderr).
///
/// Cute errors don't overwhelm end users, so by default if stderr is already streamed
/// the output won't be duplicated.
///
/// Cute errors aren't required. You don't have to use this error if you don't want, it's
/// provided because cute libraries have cute defaults.
#[derive(Debug, thiserror::Error)]
#[allow(clippy::module_name_repetitions)]
pub enum CuteCmdError {
    #[error("Could not run command command {0:?}. Details: {1}")]
    SystemError(String, std::io::Error),

    #[error("Command failed {0:?}.\nstatus: {}\nstdout: {}\nstderr: {}", .1.status,  String::from_utf8_lossy(&.1.stdout), String::from_utf8_lossy(&.1.stderr))]
    NonZeroExitNotStreamed(String, Output),

    #[error("Command failed {0:?}.\nstatus: {}\nstdout: see above\nstderr: see above", .1.status)]
    NonZeroExitAlreadyStreamed(String, Output),
}

impl<'a> CuteCmdStatus<'a> {
    /// Convert into a Result
    ///
    /// # Errors
    ///
    /// - A system error prevented the command from running `CuteCmdError::SystemError`
    /// - The command ran but exit status was non-zero `CuteCmdError::NonZeroExitNotStreamed` or `CuteCmdError::NonZeroExitAlreadyStreamed`
    pub fn to_result(self) -> Result<CuteCmdContent<'a>, CuteCmdError> {
        match self {
            CuteCmdStatus::SystemError(cute, error) => {
                Err(CuteCmdError::SystemError(cute.name, error))
            }
            CuteCmdStatus::NonZeroExitNotStreamed(CuteCmdContent {
                cmd: _,
                name,
                output,
            }) => Err(CuteCmdError::NonZeroExitNotStreamed(name, output)),
            CuteCmdStatus::NonZeroExitAlreadyStreamed(CuteCmdContent {
                cmd: _,
                name,
                output,
            }) => Err(CuteCmdError::NonZeroExitAlreadyStreamed(name, output)),
            CuteCmdStatus::Ok(CuteCmdContent { cmd, name, output }) => {
                Ok(CuteCmdContent { cmd, name, output })
            }
        }
    }

    /// When your `to_result` needs to be just a smidge bit cuter
    ///
    /// # Errors
    ///
    /// See [`to_result`]
    pub fn uwu(self) -> Result<CuteCmdContent<'a>, CuteCmdError> {
        Self::to_result(self)
    }
}

/// Holds everything in a `CuteCmd`, plus content!
///
/// Nothing is guaranteed about the status of the `Output`
/// in this struct (i.e. it could be from a successful or failed command)
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct CuteCmdContent<'a> {
    pub cmd: &'a mut Command,
    pub name: String,
    pub output: Output,
}

/// Cute commands are considerate of content
///
/// Used to wrap the result of a a `Command` call so we can build a
/// `CuteCmdStatus`.
#[derive(Debug)]
pub enum Content {
    /// Indicates that stderr and stdout are
    /// already visible above. Do not include them
    /// again in any errors that may come from this output
    AlreadyStreamed(Result<Output, std::io::Error>),

    /// Indicates that stderr and stdout have
    /// not been streamed and should be visible in the error
    NotStreamed(Result<Output, std::io::Error>),
}

lazy_static! {
    // https://github.com/jimmycuadra/rust-shellwords/blob/d23b853a850ceec358a4137d5e520b067ddb7abc/src/lib.rs#L23
    static ref QUOTE_ARG_RE: regex::Regex = regex::Regex::new(r"([^A-Za-z0-9_\-.,:/@\n])").expect("Internal error:");
}

/// Converts a command and it's arguments into a user readable string
///
fn display(command: &Command) -> String {
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
fn display_with_keys<E, K, V, I, O>(command: &Command, env: E, show_keys: I) -> String
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

/// Adds diagnostic information to an `std::io::Error` using `which_problem`
#[must_use]
pub fn annotate_which_problem(
    source: std::io::Error,
    program: OsString,
    path_env: Option<OsString>,
) -> std::io::Error {
    let problem = Which {
        program,
        path_env,
        ..Which::default()
    }
    .diagnose();

    let annotation = match problem {
        Ok(details) => format!("\nSystem diagnostic information:\n\n{details}"),
        Err(error) => format!("\nInternal error while gathering dianostic information:\n\n{error}"),
    };

    annotate_io_error(source, annotation)
}

/// Returns an IO error that displays the given annotation starting on
/// the next line.
#[must_use]
fn annotate_io_error(source: std::io::Error, annotation: String) -> std::io::Error {
    crate::err::IoErrorAnnotation::new(source, annotation).into_io_error()
}

/// Gives a reasonably cute name when given a `Command`
///
/// ```rust
/// use std::process::Command;
/// use commons::cute_cmd;
///
/// let mut command = Command::new("echo");
/// command.args(&["hello world"]);
///
/// let command_str = cute_cmd::cute_name(&mut command);
/// assert_eq!(r#"echo "hello world""#, &command_str);
/// ```
pub fn cute_name(cmd: &mut Command) -> String {
    display(cmd)
}

/// What's sweeter than shortbread? A `Command` name with environment variables!
///
/// ```rust
/// use commons::cute_cmd;
/// let mut env = libcnb::Env::new();
/// env.insert("PATH", "i_can_show_you");
///
/// let mut command = cute_cmd::plain("echo", &["the world"], &env);
/// let command_str = cute_cmd::cute_name_with_env(&mut command, ["PATH"]);
/// assert_eq!(r#"PATH="i_can_show_you" echo "the world""#, &command_str);
/// ```
pub fn cute_name_with_env(
    cmd: &mut Command,
    keys: impl IntoIterator<Item = impl Into<OsString>>,
) -> String {
    let env = cmd
        .get_envs()
        .filter_map(|(k, v)| v.map(|value| (k.to_os_string(), value.to_os_string())))
        .collect::<Vec<(OsString, OsString)>>();

    display_with_keys(cmd, env, keys)
}

// Seemingly un-needed. I'm asserting that Command behaves like I think
// it will
#[cfg(test)]
mod command_tests {
    use super::*;

    #[test]
    fn assert_envs_behavior_for_command() {
        let mut cmd = Command::new("yolo");
        cmd.envs([("key", "value")]);

        let found = cmd.get_envs().find(|(key, _)| key == &OsStr::new("key"));
        assert!(found.is_some())
    }

    #[test]
    fn assert_env_clear() {
        // let mut cmd = Command::new("env");

        // println!("{:#?}", cmd);
        // let out = cmd.output().unwrap();
        // let before = String::from_utf8_lossy(&out.stdout);

        // cmd.env_clear();

        // println!("{:#?}", cmd.);

        // let out = cmd.output().unwrap();
        // let after = String::from_utf8_lossy(&out.stdout);

        // assert_eq!(before, after);
    }

    #[test]
    fn assert_env_remove_sets_a_get_env_to_none() {
        std::env::set_var("YOLO", "from_system");

        let mut cmd = Command::new("env");

        cmd.env_remove("YOLO");

        let out = cmd.output().unwrap();
        let system_out = String::from_utf8_lossy(&out.stdout);
        println!("{system_out}");
        assert!(system_out.contains("YOLO=from_system"));

        cmd.env("YOLO", "from_method");

        let out = cmd.output().unwrap();
        let method_out = String::from_utf8_lossy(&out.stdout);
        println!("{method_out}");
        assert!(method_out.contains("YOLO=from_method"));

        let found = cmd.get_envs().find(|(k, _)| k == &OsStr::new("YOLO"));
        assert_eq!(
            Some((OsStr::new("YOLO"), Some(OsStr::new("from_method")))),
            found
        );

        cmd.env_remove("YOLO");

        let found = cmd.get_envs().find(|(k, _)| k == &OsStr::new("YOLO"));
        assert_eq!(Some((OsStr::new("YOLO"), None)), found);

        let out = cmd.output().unwrap();
        let env_remove_out = String::from_utf8_lossy(&out.stdout);
        println!("{env_remove_out}");
        assert!(!env_remove_out.contains("YOLO"));
    }
}

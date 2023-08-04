use crate::fun_run::{self, CmdError};
use libherokubuildpack::write::line_mapped;
use std::time::Instant;
use time::BuildpackDuration;

pub use section::{RunCommand, Section};

/// Build with style
///
/// ```rust,no_run
/// use commons::build_output::{self, RunCommand};
/// use std::process::Command;
///
/// // Announce your buildpack and time it
/// let timer = build_output::buildpack_name("Buildpack name");
/// // Do stuff here
/// timer.done();
///
/// // Create section with a topic
/// let section = build_output::section("Ruby version");
///
/// // Output stuff in that section
/// section.step("Installing");
/// section.step_with_details("Installing", "important stuff");
///
/// // Live stream a progress timer in that section
/// let mut timer = section.step_with_inline_timer("Installing with progress");
/// // Do stuff here
/// timer.done();
///
/// // Decorate and format your output
/// let version = build_output::fmt::value("3.1.2");
/// section.step(format!("Installing {version}"));
///
/// // Run a command in that section with a variety of formatting options
/// // Stream the output to the user:
/// section
///     .run(RunCommand::stream(
///         Command::new("echo").args(&["hello world"]),
///     ))
///     .unwrap();
///
/// // Run a command after announcing it. Show a progress timer but don't stream the output :
/// section
///     .run(RunCommand::inline_progress(
///         Command::new("echo").args(&["hello world"]),
///     ))
///     .unwrap();
///
///
/// // Run a command with no output:
/// section
///     .run(RunCommand::silent(
///         Command::new("echo").args(&["hello world"]),
///     ))
///     .unwrap();
///
/// // Control the display of the command being run:
/// section
///     .run(RunCommand::stream(
///         Command::new("bash").args(&["-c", "exec", "echo \"hello world\""]),
///     ).with_name("echo \"hello world\""))
///     .unwrap();
///```

mod time {
    use super::fmt;
    use super::print::PrintControl;
    use std::thread::{self, JoinHandle};
    use std::time::Duration;
    use std::time::Instant;

    /// Time the entire buildpack execution
    pub struct BuildpackDuration {
        pub(crate) start: Instant,
    }

    impl BuildpackDuration {
        /// Emit timing details with done block
        pub fn done_timed(self) -> PrintControl {
            let time = human(&self.start.elapsed());
            let details = fmt::details(format!("finished in {time}"));

            PrintControl::new()
                .print_inline(format!("- Done {details}"))
                .with_separator(" ")
                .on_drop_print("\n")
        }

        /// Emit done block without timing details
        #[allow(clippy::unused_self)]
        pub fn done(self) -> PrintControl {
            PrintControl::new()
                .print_inline("- Done")
                .with_separator(" ")
                .on_drop_print("\n")
        }

        /// Finish without announcing anything
        #[allow(clippy::unused_self)]
        pub fn done_silently(self) {}
    }

    /// Handles outputing inline progress based on timing
    ///
    /// i.e.   `- Installing ... (5.733s)`
    ///
    /// In this example the dashes roughly equate to seconds.
    /// The moving output in the build indicates we're waiting for something
    pub struct LiveTimingInline {
        start: Instant,
        stop_dots: std::sync::mpsc::Sender<usize>,
        join_dots: Option<JoinHandle<()>>,
    }

    impl Default for LiveTimingInline {
        fn default() -> Self {
            Self::new()
        }
    }

    impl LiveTimingInline {
        #[must_use]
        pub fn new() -> Self {
            let (stop_dots, receiver) = std::sync::mpsc::channel();

            let join_dots = thread::spawn(move || {
                PrintControl::new().print_inline(fmt::colorize(fmt::DEFAULT_DIM, " ."));

                loop {
                    let msg = receiver.recv_timeout(Duration::from_secs(1));

                    PrintControl::new().print_inline(fmt::colorize(fmt::DEFAULT_DIM, "."));
                    if msg.is_ok() {
                        PrintControl::new().print_inline(fmt::colorize(fmt::DEFAULT_DIM, ". "));
                        break;
                    }
                }
            });

            Self {
                stop_dots,
                join_dots: Some(join_dots),
                start: Instant::now(),
            }
        }

        fn stop_dots(&mut self) {
            if let Some(handle) = self.join_dots.take() {
                self.stop_dots.send(1).expect("Thread is not dead");
                handle.join().expect("Thread is joinable");
            }
        }

        pub fn done(&mut self) -> PrintControl {
            self.stop_dots();
            let contents = fmt::details(human(&self.start.elapsed()));

            PrintControl::new()
                .print_inline(contents)
                .with_separator(" ")
                .on_drop_print("\n")
        }
    }

    // Returns the part of a duration only in miliseconds
    pub(crate) fn milliseconds(duration: &Duration) -> u32 {
        duration.subsec_millis()
    }

    pub(crate) fn seconds(duration: &Duration) -> u64 {
        duration.as_secs() % 60
    }

    pub(crate) fn minutes(duration: &Duration) -> u64 {
        (duration.as_secs() / 60) % 60
    }

    pub(crate) fn hours(duration: &Duration) -> u64 {
        (duration.as_secs() / 3600) % 60
    }

    pub(crate) fn human(duration: &Duration) -> String {
        let hours = hours(duration);
        let minutes = minutes(duration);
        let seconds = seconds(duration);
        let miliseconds = milliseconds(duration);

        if hours > 0 {
            format!("{hours}h {minutes}m {seconds}s")
        } else if minutes > 0 {
            format!("{minutes}m {seconds}s")
        } else if seconds > 0 || miliseconds > 100 {
            // 0.1
            format!("{seconds}.{miliseconds:0>3}s")
        } else {
            String::from("< 0.1s")
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn test_millis_and_seconds() {
            let duration = Duration::from_millis(1024);
            assert_eq!(24, milliseconds(&duration));
            assert_eq!(1, seconds(&duration));
        }

        #[test]
        fn test_display_duration() {
            let duration = Duration::from_millis(99);
            assert_eq!("< 0.1s", human(&duration).as_str());

            let duration = Duration::from_millis(1024);
            assert_eq!("1.024s", human(&duration).as_str());

            let duration = std::time::Duration::from_millis(60 * 1024);
            assert_eq!("1m 1s", human(&duration).as_str());

            let duration = std::time::Duration::from_millis(3600 * 1024);
            assert_eq!("1h 1m 26s", human(&duration).as_str());
        }
    }
}

/// All work is done inside of a section. Advertize a section topic
pub fn section(topic: impl AsRef<str>) -> section::Section {
    let topic = fmt::section(String::from(topic.as_ref()));
    println!("{topic}");

    section::Section { topic }
}

/// Top level buildpack header
///
/// Should only use once per buildpack
#[must_use]
pub fn buildpack_name(buildpack: impl AsRef<str>) -> BuildpackDuration {
    let header = fmt::header(buildpack.as_ref());
    println!("{header}");
    println!();

    let start = Instant::now();
    BuildpackDuration { start }
}

mod section {
    use super::{fmt, fun_run, line_mapped, time, time::LiveTimingInline, CmdError, Instant};
    use crate::{
        build_output::print::PrintControl,
        fun_run::{NamedOutput, ResultNameExt},
    };
    use libherokubuildpack::command::CommandExt;
    use std::process::Command;

    #[derive(Debug, Clone, Eq, PartialEq)]
    pub struct Section {
        pub(crate) topic: String,
    }

    impl Section {
        /// Emit contents to the buid output with indentation
        pub fn step(&self, contents: impl AsRef<str>) -> PrintControl {
            PrintControl::new()
                .print_inline(contents)
                .with_separator(" ")
                .on_drop_print("\n")
        }

        pub fn step_with_details(
            &self,
            contents: impl AsRef<str>,
            details: impl AsRef<str>,
        ) -> PrintControl {
            let contents = contents.as_ref();
            let details = fmt::details(details.as_ref());

            let contents = fmt::step(format!("{contents} {details}"));

            PrintControl::new()
                .print_inline(contents)
                .with_separator(" ")
                .on_drop_print("\n")
        }

        /// Emit an inline indented help section with a "- ! Help: {contents}" prefix auto added
        pub fn help(&self, contents: impl AsRef<str>) -> PrintControl {
            let contents = fmt::step(fmt::help(contents));

            PrintControl::new()
                .print_inline(contents)
                .with_separator(" ")
                .on_drop_print("\n")
        }

        /// Start a time and emit a reson for it
        ///
        /// The timer will emit an inline progress meter until `LiveTimingInline::done` is called
        /// on it.
        #[must_use]
        pub fn step_with_inline_timer(&self, reason: impl AsRef<str>) -> time::LiveTimingInline {
            let reason = reason.as_ref();
            PrintControl::new().print_inline(fmt::step(reason));
            time::LiveTimingInline::new()
        }

        /// Run a command with the given configuration and name
        ///
        /// # Errors
        ///
        /// Returns an error if the command status is non-zero or if the
        /// system cannot run the command.
        pub fn run(&self, run_command: RunCommand) -> Result<NamedOutput, CmdError> {
            match run_command.output {
                OutputConfig::Stream | OutputConfig::StreamNoTiming => {
                    Self::stream_command(self, run_command)
                }
                OutputConfig::Silent => Self::silent_command(self, run_command),
                OutputConfig::InlineProgress => Self::inline_progress_command(self, run_command),
            }
        }

        /// Run a command and output nothing to the screen
        fn silent_command(
            _section: &Section,
            run_command: RunCommand,
        ) -> Result<NamedOutput, CmdError> {
            let RunCommand {
                command,
                name,
                output: _config,
            } = run_command;

            command
                .output()
                .with_name(name)
                .and_then(NamedOutput::nonzero_captured)
        }

        /// Run a command. Output command name, but don't stream the contents
        fn inline_progress_command(
            _section: &Section,
            run_command: RunCommand,
        ) -> Result<NamedOutput, CmdError> {
            let RunCommand {
                command,
                name,
                output: _config,
            } = run_command;
            let name = fmt::command(name);

            PrintControl::new().print_inline(fmt::step(format!("Running {name}")));

            let mut start = LiveTimingInline::new();
            let output = command.output();
            let result = output
                .with_name(name)
                .and_then(NamedOutput::nonzero_captured);

            start.done();

            result
        }

        /// Run a command. Output command name, and stream the contents
        fn stream_command(
            section: &Section,
            run_command: RunCommand,
        ) -> Result<NamedOutput, CmdError> {
            let RunCommand {
                command,
                name,
                output: config,
            } = run_command;
            let name = fmt::command(name);

            section.step(format!("Running {name}"));
            println!(); // Weird output from prior stream adds indentation that's unwanted

            let start = Instant::now();
            let result = command
                .output_and_write_streams(
                    line_mapped(std::io::stdout(), fmt::cmd_stream_format),
                    line_mapped(std::io::stderr(), fmt::cmd_stream_format),
                )
                .with_name(name)
                .and_then(NamedOutput::nonzero_streamed);

            println!(); // Weird output from prior stream adds indentation that's unwanted

            let duration = start.elapsed();
            let time = fmt::details(time::human(&duration));
            match config {
                OutputConfig::Stream => {
                    section.step(format!("Done {time}")).done();
                }
                OutputConfig::StreamNoTiming => section.step("Done {time}").done(),
                OutputConfig::Silent | OutputConfig::InlineProgress => unreachable!(),
            }

            result
        }
    }

    /// Specify how you want a command to be run by `Section::run`
    pub struct RunCommand<'a> {
        command: &'a mut Command,
        name: String,
        output: OutputConfig,
    }

    impl<'a> RunCommand<'a> {
        /// Generate a new `RunCommand` with a different name
        #[must_use]
        pub fn with_name(self, name: impl AsRef<str>) -> Self {
            let name = name.as_ref().to_string();
            let RunCommand {
                command,
                name: _,
                output,
            } = self;

            Self {
                command,
                name,
                output,
            }
        }

        /// Announce and stream the output of a command
        pub fn stream(command: &'a mut Command) -> Self {
            let name = fun_run::display(command);
            Self {
                command,
                name,
                output: OutputConfig::Stream,
            }
        }

        /// Announce and stream the output of a command without timing information at the end
        pub fn stream_without_timing(command: &'a mut Command) -> Self {
            let name = fun_run::display(command);
            Self {
                command,
                name,
                output: OutputConfig::StreamNoTiming,
            }
        }

        /// Do not announce or stream output of a command
        pub fn silent(command: &'a mut Command) -> Self {
            let name = fun_run::display(command);
            Self {
                command,
                name,
                output: OutputConfig::Silent,
            }
        }

        /// Announce a command inline. Do not stream it's output. Emit inline progress timer.
        pub fn inline_progress(command: &'a mut Command) -> Self {
            let name = fun_run::display(command);
            Self {
                command,
                name,
                output: OutputConfig::InlineProgress,
            }
        }
    }

    enum OutputConfig {
        Stream,
        StreamNoTiming,
        Silent,
        InlineProgress,
    }
}

pub mod fmt {
    use std::process::Output;

    use crate::fun_run::{CmdError, NamedOutput};

    pub(crate) const RED: &str = "\x1B[0;31m";
    pub(crate) const YELLOW: &str = "\x1B[0;33m";
    pub(crate) const CYAN: &str = "\x1B[0;36m";

    pub(crate) const BOLD_CYAN: &str = "\x1B[1;36m";
    pub(crate) const BOLD_PURPLE: &str = "\x1B[1;35m"; // magenta

    pub(crate) const DEFAULT_DIM: &str = "\x1B[2;1m"; // Default color but softer/less vibrant
    pub(crate) const RESET: &str = "\x1B[0m";

    #[cfg(test)]
    pub(crate) const NOCOLOR: &str = "\x1B[1;39m"; // Differentiate between color clear and explicit no color https://github.com/heroku/buildpacks-ruby/pull/155#discussion_r1260029915
    #[cfg(test)]
    pub(crate) const ALL_CODES: [&str; 7] = [
        RED,
        YELLOW,
        CYAN,
        BOLD_CYAN,
        BOLD_PURPLE,
        DEFAULT_DIM,
        RESET,
    ];

    pub(crate) const HEROKU_COLOR: &str = BOLD_PURPLE;
    pub(crate) const VALUE_COLOR: &str = YELLOW;
    pub(crate) const COMMAND_COLOR: &str = BOLD_CYAN;
    pub(crate) const URL_COLOR: &str = CYAN;
    pub(crate) const IMPORTANT_COLOR: &str = CYAN;
    pub(crate) const ERROR_COLOR: &str = RED;

    #[allow(dead_code)]
    pub(crate) const WARNING_COLOR: &str = YELLOW;

    const SECTION_PREFIX: &str = "- ";
    const STEP_PREFIX: &str = "  - ";
    const CMD_INDENT: &str = "      ";

    #[must_use]
    pub fn url(contents: impl AsRef<str>) -> String {
        colorize(URL_COLOR, contents)
    }

    /// Used to decorate a command being run i.e. `bundle install`
    #[must_use]
    pub fn command(contents: impl AsRef<str>) -> String {
        value(colorize(COMMAND_COLOR, contents.as_ref()))
    }

    /// Used to decorate a derived or user configured value
    #[must_use]
    pub fn value(contents: impl AsRef<str>) -> String {
        let contents = colorize(VALUE_COLOR, contents.as_ref());
        format!("`{contents}`")
    }

    /// Used to decorate additional information
    #[must_use]
    pub fn details(contents: impl AsRef<str>) -> String {
        let contents = contents.as_ref();
        format!("({contents})")
    }

    /// Used to decorate a buildpack
    #[must_use]
    pub(crate) fn header(contents: impl AsRef<str>) -> String {
        let contents = contents.as_ref();
        colorize(HEROKU_COLOR, format!("\n# {contents}"))
    }

    #[must_use]
    pub fn section(topic: impl AsRef<str>) -> String {
        let topic = topic.as_ref();
        format!("{SECTION_PREFIX}{topic}")
    }

    #[must_use]
    pub fn step(contents: impl AsRef<str>) -> String {
        let contents = contents.as_ref();
        format!("{STEP_PREFIX}{contents}")
    }

    /// Used with libherokubuildpack linemapped command output
    ///
    #[must_use]
    pub fn cmd_stream_format(mut input: Vec<u8>) -> Vec<u8> {
        let mut result: Vec<u8> = CMD_INDENT.into();
        result.append(&mut input);
        result
    }

    /// Like `cmd_stream_format` but for static intput
    #[must_use]
    pub fn cmd_output_format(contents: impl AsRef<str>) -> String {
        let contents = contents
            .as_ref()
            .split('\n')
            .map(|part| {
                let tmp: Vec<u8> = cmd_stream_format(part.into());
                String::from_utf8_lossy(&tmp).into_owned()
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Emulate above
        format!("\n{contents}\n")
    }

    #[must_use]
    pub(crate) fn help(contents: impl AsRef<str>) -> String {
        let contents = contents.as_ref();
        colorize(IMPORTANT_COLOR, bangify(format!("Help: {contents}")))
    }

    #[must_use]
    pub fn cmd_diagnostics(diagnostics: &Vec<Result<NamedOutput, CmdError>>) -> String {
        let mut parts = Vec::new();
        if !diagnostics.is_empty() {
            parts.push(section("System diagnostic information"));
        }

        for diagnostic in diagnostics {
            let name = command(match diagnostic {
                Ok(output) => output.name.clone(),
                Err(error) => error.name().to_string(),
            });

            let status = value(
                match diagnostic {
                    Ok(output) => output.status().code().unwrap_or(1),
                    Err(_) => 1,
                }
                .to_string(),
            );

            let details = details(format!("exit status {status}"));
            parts.push(step(colorize(
                IMPORTANT_COLOR,
                bangify(format!("Diagnostic command {name} {details}")),
            )));

            match diagnostic {
                Ok(named_output) => parts.push(output_stdout_stderr_as_steps(&named_output.output)),
                Err(e) => match e {
                    CmdError::SystemError(_, e) => parts.push(e.to_string()),
                    CmdError::NonZeroExitNotStreamed(o)
                    | CmdError::NonZeroExitAlreadyStreamed(o) => {
                        parts.push(output_stdout_stderr_as_steps(&o.output));
                    }
                },
            }
        }

        parts.join("\n")
    }

    fn output_stdout_stderr_as_steps(output: &Output) -> String {
        let mut parts = Vec::new();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if stdout.trim().is_empty() {
            parts.push(step("stdout: <empty>"));
        } else {
            parts.push(step("stdout:"));
            parts.push(cmd_output_format(stdout));
        }

        if stderr.trim().is_empty() {
            parts.push(step("stderr: <empty>"));
        } else {
            parts.push(step("stderr:"));
            parts.push(cmd_output_format(stderr));
        }
        parts.join("\n")
    }

    fn exit_status(output: &Output) -> String {
        format!(
            "exit status: {}",
            value(output.status.code().unwrap_or(1).to_string())
        )
    }

    #[must_use]
    pub fn cmd_error(error: &CmdError) -> String {
        let name = command(error.name());
        let section = section(colorize(
            ERROR_COLOR,
            bangify(format!(
                "Command failed {name} {}",
                details("details below")
            )),
        ));

        let mut parts = vec![section];
        match error {
            CmdError::SystemError(_, error) => {
                parts.push(step("System error:"));
                parts.push(cmd_output_format(error.to_string()));
            }
            CmdError::NonZeroExitNotStreamed(named_output) => {
                parts.push(step(exit_status(&named_output.output)));
                parts.push(output_stdout_stderr_as_steps(&named_output.output));
            }
            CmdError::NonZeroExitAlreadyStreamed(named_output) => {
                parts.push(step(exit_status(&named_output.output)));

                parts.push(step("stdout: <streamed above>"));
                parts.push(step("stderr: <streamed above>"));
            }
        }

        parts.join("\n")
    }

    /// Helper method that adds a bang i.e. `!` before strings
    pub(crate) fn bangify(body: impl AsRef<str>) -> String {
        prepend_each_line("!", " ", body)
    }

    // Ensures every line starts with `prepend`
    pub(crate) fn prepend_each_line(
        prepend: impl AsRef<str>,
        separator: impl AsRef<str>,
        contents: impl AsRef<str>,
    ) -> String {
        let body = contents.as_ref();
        let prepend = prepend.as_ref();
        let separator = separator.as_ref();

        if body.trim().is_empty() {
            format!("{prepend}\n")
        } else {
            body.split('\n')
                .map(|section| {
                    if section.trim().is_empty() {
                        prepend.to_string()
                    } else {
                        format!("{prepend}{separator}{section}")
                    }
                })
                .collect::<Vec<String>>()
                .join("\n")
        }
    }

    /// Colorizes a body while preserving existing color/reset combinations and clearing before newlines
    ///
    /// Colors with newlines are a problem since the contents stream to git which prepends `remote:` before the `libcnb_test`
    /// if we don't clear, then we will colorize output that isn't ours.
    ///
    /// Explicitly uncolored output is handled by treating `\x1b[1;39m` (NOCOLOR) as a distinct case from `\x1b[0m`
    pub(crate) fn colorize(color: &str, body: impl AsRef<str>) -> String {
        body.as_ref()
            .split('\n')
            // If sub contents are colorized it will contain SUBCOLOR ... RESET. After the reset,
            // ensure we change back to the current color
            .map(|line| line.replace(RESET, &format!("{RESET}{color}"))) // Handles nested color
            // Set the main color for each line and reset after so we don't colorize `remote:` by accident
            .map(|line| format!("{color}{line}{RESET}"))
            // The above logic causes redundant colors and resets, clean them up
            .map(|line| line.replace(&format!("{RESET}{color}{RESET}"), RESET))
            .map(|line| line.replace(&format!("{color}{color}"), color)) // Reduce useless color
            .collect::<Vec<String>>()
            .join("\n")
    }

    #[cfg(test)]
    pub(crate) fn strip_control_codes(contents: impl AsRef<str>) -> String {
        let mut contents = contents.as_ref().to_string();
        for code in ALL_CODES {
            contents = contents.replace(code, "");
        }
        contents
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn test_bangify() {
            let actual = bangify("");
            assert_eq!("!\n", actual);

            let actual = bangify("\n");
            assert_eq!("!\n", actual);
        }

        #[test]
        fn handles_explicitly_removed_colors() {
            let nested = colorize(NOCOLOR, "nested");

            let out = colorize(RED, format!("hello {nested} color"));
            let expected = format!("{RED}hello {NOCOLOR}nested{RESET}{RED} color{RESET}");

            assert_eq!(expected, out);
        }

        #[test]
        fn handles_nested_colors() {
            let nested = colorize(CYAN, "nested");

            let out = colorize(RED, format!("hello {nested} color"));
            let expected = format!("{RED}hello {CYAN}nested{RESET}{RED} color{RESET}");

            assert_eq!(expected, out);
        }

        #[test]
        fn splits_newlines() {
            let actual = colorize(RED, "hello\nworld");
            let expected = format!("{RED}hello{RESET}\n{RED}world{RESET}");

            assert_eq!(expected, actual);
        }

        #[test]
        fn simple_case() {
            let actual = colorize(RED, "hello world");
            assert_eq!(format!("{RED}hello world{RESET}"), actual);
        }
    }
}

pub mod attn {
    use super::fmt::{ERROR_COLOR, IMPORTANT_COLOR, WARNING_COLOR};
    use crate::build_output::fmt::{self, bangify, colorize};
    use itertools::Itertools;
    use std::fmt::Display;

    /// Holds info about a url
    #[derive(Debug, Clone, PartialEq)]
    pub enum Url {
        Label { label: String, url: String },
        MoreInfo(String),
    }

    impl Display for Url {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Url::Label { label, url } => {
                    writeln!(f, "{label}: {}", fmt::url(url))
                }
                Url::MoreInfo(url) => writeln!(
                    f,
                    "For more information, refer to the following documentation:\n{}",
                    fmt::url(url)
                ),
            }
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub enum Detail {
        Raw(String),
    }

    impl Display for Detail {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Detail::Raw(details) => write!(f, "{details}"),
            }
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub enum Body {
        Raw(String),
    }

    impl Display for Body {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Body::Raw(string) => write!(f, "{string}"),
            }
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub enum Part {
        Body(Body),
        Url(Url),
        Detail(Detail),
    }

    impl Display for Part {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Part::Body(body) => write!(f, "{body}"),
                Part::Url(url) => write!(f, "{url}"),
                Part::Detail(details) => write!(f, "{details}"),
            }
        }
    }

    /// Build the contents of an Announcement
    pub struct Announcement {
        name: String,
        color: String,
        inner: Vec<Part>,
    }

    impl Display for Announcement {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let mut parts = self
                .inner
                .iter()
                .tuple_windows::<(_, _)>()
                .map(|(now, next)| {
                    let part = self.format_part(now);
                    // If both last and next lines share a prefix then add a prefix
                    // to the newline separator
                    let sep = match (now, next) {
                        (Part::Detail(_), _) | (_, Part::Detail(_)) => "\n".to_string(),
                        _ => colorize(&self.color, bangify("\n")),
                    };

                    format!("{part}{sep}")
                })
                .collect::<Vec<_>>();

            if let Some(part) = self.inner.last() {
                parts.push(self.format_part(part));
            }

            write!(f, "{}", parts.join(""))
        }
    }

    impl Announcement {
        fn format_part(&self, part: &Part) -> String {
            let part = match part {
                Part::Body(body) => colorize(&self.color, bangify(body.to_string().trim())),
                Part::Url(url) => colorize(&self.color, bangify(url.to_string().trim())),
                Part::Detail(details) => details.to_string().trim().to_string(),
            };
            format!("{part}\n")
        }

        #[must_use]
        pub fn error() -> Self {
            Self {
                name: "ERROR".to_string(),
                color: ERROR_COLOR.to_string(),
                inner: Vec::new(),
            }
        }

        #[must_use]
        pub fn warning() -> Self {
            Self {
                name: "WARNING".to_string(),
                color: WARNING_COLOR.to_string(),
                inner: Vec::new(),
            }
        }

        #[must_use]
        pub fn important() -> Self {
            Self {
                name: "IMPORTANT".to_string(),
                color: IMPORTANT_COLOR.to_string(),
                inner: Vec::new(),
            }
        }

        pub fn header(&mut self, header: impl AsRef<str>) -> &mut Self {
            let header = format!("{}: {}", self.name, header.as_ref());
            self.inner.push(Part::Body(Body::Raw(header)));
            self
        }

        pub fn body(&mut self, body: impl AsRef<str>) -> &mut Self {
            self.inner
                .push(Part::Body(Body::Raw(body.as_ref().to_string())));
            self
        }

        pub fn url(&mut self, url: Url) -> &mut Self {
            self.inner.push(Part::Url(url));
            self
        }

        /// I don't love having this in here It's technically not part of the announcement
        /// from a philosophical standpoint, however it makes a nice chained API
        pub fn detail(&mut self, detail: Detail) -> &mut Self {
            self.inner.push(Part::Detail(detail));
            self
        }

        pub fn print(&mut self) {
            println!();
            println!("{self}");
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;
        use crate::build_output::attn;
        use crate::build_output::fmt::strip_control_codes;
        use crate::fun_run::{self, CmdMapExt, NamedOutput, ResultNameExt};
        use indoc::formatdoc;

        #[test]
        fn test_error_output_with_url_and_detailsvisual() {
            let actual = Announcement::error()
                .header(
                    "Error installing Ruby"
                )
                .body(formatdoc! {"
                    Could not install the detected Ruby version. Ensure that you're using a supported
                    ruby version and try again.
                "})
                .url(Url::MoreInfo(
                    "https://devcenter.heroku.com/articles/ruby-support#ruby-versions".to_string(),
                ))
                .detail(
                    Detail::Raw(formatdoc!{"
                        Could not create file: You do not have sufficient permissions to access this file: /path/to/file
                    "})
                )
                .to_string();

            let expected = formatdoc! {
               "! ERROR: Error installing Ruby
                !
                ! Could not install the detected Ruby version. Ensure that you're using a supported
                ! ruby version and try again.
                !
                ! For more information, refer to the following documentation:
                ! https://devcenter.heroku.com/articles/ruby-support#ruby-versions

                Could not create file: You do not have sufficient permissions to access this file: /path/to/file
            "};

            assert_eq!(expected, strip_control_codes(actual));
        }

        #[test]
        fn cmd_error_output() {
            let result = std::process::Command::new("cat")
                .arg("does_not_exist")
                .cmd_map(|cmd| {
                    cmd.output()
                        .with_name(fun_run::display(cmd))
                        .and_then(NamedOutput::nonzero_captured)
                });

            match result {
                Ok(out) => panic!("Command should have failed {out:?}"),
                Err(error) => {
                    let error_detail = attn::Detail::Raw(fmt::cmd_error(&error));
                    let expected = formatdoc! {"
                        - ! Command failed `cat does_not_exist` (details below)
                          - exit status: `1`
                          - stdout: <empty>
                          - stderr:

                              cat: does_not_exist: No such file or directory
                        "};
                    assert_eq!(
                        expected.trim(),
                        strip_control_codes(error_detail.clone().to_string().trim())
                    );

                    let actual = Announcement::error()
                        .header("Failed to compile assets")
                        .body("oops")
                        .detail(error_detail)
                        .to_string();

                    let expected = formatdoc! {"
                        ! ERROR: Failed to compile assets
                        !
                        ! oops

                        - ! Command failed `cat does_not_exist` (details below)
                          - exit status: `1`
                          - stdout: <empty>
                          - stderr:

                              cat: does_not_exist: No such file or directory
                    "};

                    assert_eq!(expected.trim(), strip_control_codes(actual.trim()));
                }
            }
        }
    }
}

mod print {
    use std::io::Write;

    /// Allows for the caller to control printing beahvior
    ///
    /// Problem: Sometimes you know that you want to print a message right now
    /// you also know that after the message you'll want a newline. But if you
    /// print a newline at this second, then you'll prevent composing any other
    /// output from writing to the same line.
    ///
    /// Solution: Yield control of the newline back to the caller and default
    /// to printing a finalized newline character. Default to printing a newline
    /// on drop so if the caller doesn't assume control a newline is guaranteed
    /// be added.
    ///
    /// Problem: You want to yield control to allow multiple elements to be
    /// printed to the same line, you don't know if there's going to be anything
    /// else printed, but if they are, you want a space between them. You also
    /// don't want trailing whitespace in your output in the event that it's you're
    /// done printing and want a newline.
    ///
    /// Solution: Save the separator now, write it to a
    #[derive(Default)]
    #[allow(clippy::module_name_repetitions)]
    pub struct PrintControl {
        separator: Option<String>,
        on_drop_print: Option<String>,
    }

    impl PrintControl {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn with_separator(mut self, separator: impl AsRef<str>) -> Self {
            self.separator = Some(separator.as_ref().to_string());
            self
        }

        /// Change the value to print when struct is dropped
        pub fn on_drop_print(mut self, value: impl AsRef<str>) -> Self {
            self.on_drop_print = Some(value.as_ref().to_string());
            self
        }

        /// Prints the message inline with prior separator (if there was one)
        pub fn print_inline(self, contents: impl AsRef<str>) -> Self {
            self.print_with_last_separator(contents);
            self
        }

        /// Finish printing, will output `on_drop_print` if there is one
        pub fn done(self) {
            drop(self);
        }

        fn print_with_last_separator(&self, contents: impl AsRef<str>) -> &Self {
            if let Some(sep) = &self.separator {
                Self::inline_print_flush(sep);
            }
            Self::inline_print_flush(contents);
            self
        }

        // Helper for printing without newlines that auto-flushes stdout
        fn inline_print_flush(contents: impl AsRef<str>) {
            let contents = contents.as_ref();
            print!("{contents}");
            std::io::stdout().flush().expect("Stdout is writable");
        }
    }

    impl Drop for PrintControl {
        fn drop(&mut self) {
            if let Some(contents) = &self.on_drop_print {
                PrintControl::inline_print_flush(contents);
            }
        }
    }
}

use crate::fun_run::{self, CmdError};
use libherokubuildpack::write::{line_mapped, mappers::add_prefix};
use std::io::Write;
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
/// section.say("Installing");
/// section.say_with_details("Installing", "important stuff");
///
/// // Live stream a progress timer in that section
/// let mut timer = section.say_with_inline_timer("Installing with progress");
/// // Do stuff here
/// timer.done();
///
/// // Decorate and format your output
/// let version = build_output::fmt::value("3.1.2");
/// section.say(format!("Installing {version}"));
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
///     .run(RunCommand::quiet(
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
    use super::{fmt, raw_inline_print};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;
    use std::time::Instant;

    /// Time the entire buildpack execution
    pub struct BuildpackDuration {
        pub(crate) start: Instant,
    }

    impl BuildpackDuration {
        /// Emit timing details with done block
        pub fn done_timed(self) {
            let time = human(&self.start.elapsed());
            let details = fmt::details(format!("finished in {time}"));
            println!("- Done {details}");
        }

        /// Emit done block without timing details
        #[allow(clippy::unused_self)]
        pub fn done(self) {
            println!("- Done");
        }

        /// Finish without announcing anything
        #[allow(clippy::unused_self)]
        pub fn done_silently(self) {}
    }

    /// Handles outputing inline progress based on timing
    ///
    /// i.e.   `- Installing [------] (5.733s)`
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
                raw_inline_print(fmt::colorize(fmt::DEFAULT_DIM, " ["));

                loop {
                    let msg = receiver.recv_timeout(Duration::from_secs(1));
                    raw_inline_print(fmt::colorize(fmt::DEFAULT_DIM, "-"));

                    if msg.is_ok() {
                        raw_inline_print(fmt::colorize(fmt::DEFAULT_DIM, "] "));
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

        pub fn done(&mut self) {
            self.stop_dots();
            let time = fmt::details(human(&self.start.elapsed()));

            println!("{time}");
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

// Helper for printing without newlines that auto-flushes stdout
fn raw_inline_print(contents: impl AsRef<str>) {
    let contents = contents.as_ref();
    print!("{contents}");
    std::io::stdout().flush().expect("Stdout is writable");
}

/// All work is done inside of a section. Advertize a section topic
pub fn section(topic: impl AsRef<str>) -> section::Section {
    let topic = String::from(topic.as_ref());
    println!("- {topic}");

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
    use super::{
        add_prefix, fmt, fun_run, line_mapped, raw_inline_print, time, time::LiveTimingInline,
        CmdError, Instant,
    };
    use libherokubuildpack::command::CommandExt;
    use std::process::{Command, Output};

    const CMD_INDENT: &str = "      ";
    const SECTION_INDENT: &str = "  ";
    const SECTION_PREFIX: &str = "  - ";

    #[derive(Debug, Clone, Eq, PartialEq)]
    pub struct Section {
        pub(crate) topic: String,
    }

    impl Section {
        /// Emit contents to the buid output with indentation
        pub fn say(&self, contents: impl AsRef<str>) {
            let contents = contents.as_ref();
            println!("{SECTION_PREFIX}{contents}");
        }

        pub fn say_with_details(&self, contents: impl AsRef<str>, details: impl AsRef<str>) {
            let contents = contents.as_ref();
            let details = fmt::details(details.as_ref());

            println!("{SECTION_PREFIX}{contents} {details}");
        }

        /// Emit an indented help section with a "! Help:" prefix auto added
        pub fn help(&self, contents: impl AsRef<str>) {
            let contents = fmt::help(contents);

            println!("{SECTION_INDENT}{contents}");
        }

        /// Start a time and emit a reson for it
        ///
        /// The timer will emit an inline progress meter until `LiveTimingInline::done` is called
        /// on it.
        #[must_use]
        pub fn say_with_inline_timer(&self, reason: impl AsRef<str>) -> time::LiveTimingInline {
            let reason = reason.as_ref();
            raw_inline_print(format!("{SECTION_PREFIX}{reason}"));

            time::LiveTimingInline::new()
        }

        /// Run a command with the given configuration and name
        ///
        /// # Errors
        ///
        /// Returns an error if the command status is non-zero or if the
        /// system cannot run the command.
        pub fn run(&self, run_command: RunCommand) -> Result<Output, CmdError> {
            match run_command.output {
                OutputConfig::Stream | OutputConfig::StreamNoTiming => {
                    Self::stream_command(self, run_command)
                }
                OutputConfig::Quiet => Self::silent_command(self, run_command),
                OutputConfig::InlineProgress => Self::inline_progress_command(self, run_command),
            }
        }

        /// If someone wants to build their own command invocation and wants to match styles with this
        /// command runner they'll need access to the prefix for consistent execution.
        #[must_use]
        pub fn cmd_stream_prefix() -> String {
            String::from(CMD_INDENT)
        }

        /// Run a command and output nothing to the screen
        fn silent_command(_section: &Section, run_command: RunCommand) -> Result<Output, CmdError> {
            let RunCommand {
                command,
                name,
                output: _,
            } = run_command;

            command
                .output()
                .map_err(|error| fun_run::on_system_error(name.clone(), error))
                .and_then(|output| fun_run::nonzero_captured(name, output))
        }

        /// Run a command. Output command name, but don't stream the contents
        fn inline_progress_command(
            _section: &Section,
            run_command: RunCommand,
        ) -> Result<Output, CmdError> {
            let RunCommand {
                command,
                name,
                output: _,
            } = run_command;
            let name = fmt::command(name);

            raw_inline_print(format!("{SECTION_PREFIX}Running {name}"));

            let mut start = LiveTimingInline::new();
            let output = command.output();
            let result = output
                .map_err(|error| fun_run::on_system_error(name.clone(), error))
                .and_then(|output| fun_run::nonzero_captured(name, output));

            start.done();

            result
        }

        /// Run a command. Output command name, and stream the contents
        fn stream_command(section: &Section, run_command: RunCommand) -> Result<Output, CmdError> {
            let RunCommand {
                command,
                name,
                output,
            } = run_command;
            let name = fmt::command(name);

            section.say(format!("Running {name}"));
            println!(); // Weird output from prior stream adds indentation that's unwanted

            let start = Instant::now();
            let result = command
                .output_and_write_streams(
                    line_mapped(std::io::stdout(), add_prefix(CMD_INDENT)),
                    line_mapped(std::io::stderr(), add_prefix(CMD_INDENT)),
                )
                .map_err(|error| fun_run::on_system_error(name.clone(), error))
                .and_then(|output| fun_run::nonzero_streamed(name, output));

            println!(); // Weird output from prior stream adds indentation that's unwanted

            let duration = start.elapsed();
            let time = fmt::details(time::human(&duration));
            match output {
                OutputConfig::Stream => {
                    section.say(format!("Done {time}"));
                }
                OutputConfig::StreamNoTiming => section.say("Done {time}"),
                OutputConfig::Quiet | OutputConfig::InlineProgress => unreachable!(),
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
        pub fn quiet(command: &'a mut Command) -> Self {
            let name = fun_run::display(command);
            Self {
                command,
                name,
                output: OutputConfig::Quiet,
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
        Quiet,
        InlineProgress,
    }
}

pub mod fmt {
    use indoc::formatdoc;
    use std::fmt::Display;

    pub(crate) const RED: &str = "\x1B[31m";
    pub(crate) const YELLOW: &str = "\x1B[33m";
    pub(crate) const CYAN: &str = "\x1B[36m";
    // pub(crate) const PURPLE: &str = "\x1B[35m"; // magenta

    pub(crate) const BOLD_CYAN: &str = "\x1B[1;36m";
    pub(crate) const BOLD_PURPLE: &str = "\x1B[1;35m"; // magenta

    pub(crate) const DEFAULT_DIM: &str = "\x1B[2;1m"; // Default color but softer/less vibrant
    pub(crate) const RESET: &str = "\x1B[0m";
    pub(crate) const NOCOLOR: &str = "\x1B[0m\x1B[0m"; //differentiate between color clear and explicit no color
    pub(crate) const NOCOLOR_TMP: &str = "🙈🙈🙈"; // Used together with NOCOLOR to act as a placeholder

    pub(crate) const HEROKU_COLOR: &str = BOLD_PURPLE;
    pub(crate) const VALUE_COLOR: &str = YELLOW;
    pub(crate) const COMMAND_COLOR: &str = BOLD_CYAN;
    pub(crate) const URL_COLOR: &str = CYAN;
    pub(crate) const IMPORTANT_COLOR: &str = CYAN;
    pub(crate) const ERROR_COLOR: &str = RED;
    pub(crate) const WARNING_COLOR: &str = YELLOW;

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

    /// Used to standardize error/warning/important information
    pub(crate) fn look_at_me(
        color: &str,
        noun: impl AsRef<str>,
        header: impl AsRef<str>,
        body: impl AsRef<str>,
        url: &Option<String>,
    ) -> String {
        let noun = noun.as_ref();
        let header = header.as_ref();
        let body = help_url(body, url);
        colorize(
            color,
            bangify(formatdoc! {"
                {noun} {header}

                {body}
            "}),
        )
    }

    #[must_use]
    pub(crate) fn help(contents: impl AsRef<str>) -> String {
        let contents = contents.as_ref();
        colorize(IMPORTANT_COLOR, bangify(format!("Help: {contents}")))
    }

    /// Holds the contents of an error
    ///
    /// Designed so that additional optional fields may be added later without
    /// breaking compatability
    #[derive(Debug, Clone, Default)]
    pub struct ErrorInfo {
        header: String,
        body: String,
        url: Option<String>,
        debug_details: Option<String>,
    }

    impl ErrorInfo {
        pub fn header_body_details(
            header: impl AsRef<str>,
            body: impl AsRef<str>,
            details: impl Display,
        ) -> Self {
            Self {
                header: header.as_ref().to_string(),
                body: body.as_ref().to_string(),
                debug_details: Some(details.to_string()),
                ..Default::default()
            }
        }

        pub fn print(&self) {
            println!("{self}");
        }
    }

    impl Display for ErrorInfo {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(error(self).as_str())
        }
    }

    /// Need feedback on this interface
    ///
    /// Should it take fields or a struct
    /// Struct is more robust to change but extra boilerplate
    /// If the struct is always needed, this should perhaps be an associated function
    #[must_use]
    pub fn error(info: &ErrorInfo) -> String {
        let ErrorInfo {
            header,
            body,
            url,
            debug_details,
        } = info;

        let body = look_at_me(ERROR_COLOR, "ERROR:", header, body, url);
        if let Some(details) = debug_details {
            format!("{body}\n\nDebug information: {details}")
        } else {
            body
        }
    }

    /// Need feedback on this interface
    ///
    /// Should it have a dedicated struct like `ErrorInfo` or be a function?
    /// Do we want to bundle warnings together and emit them at the end? (I think so, but it's out of current scope)
    pub fn warning(header: impl AsRef<str>, body: impl AsRef<str>, url: &Option<String>) -> String {
        let header = header.as_ref();
        let body = body.as_ref();

        look_at_me(WARNING_COLOR, "WARNING:", header, body, url)
    }

    /// Need feedback on this interface
    ///
    /// Same questions as `warning`
    pub fn important(
        header: impl AsRef<str>,
        body: impl AsRef<str>,
        url: &Option<String>,
    ) -> String {
        let header = header.as_ref();
        let body = body.as_ref();

        look_at_me(IMPORTANT_COLOR, "", header, body, url)
    }

    fn help_url(body: impl AsRef<str>, url: &Option<String>) -> String {
        let body = body.as_ref();

        if let Some(url) = url {
            let url = colorize(URL_COLOR, url);

            formatdoc! {"
            {body}

            For more information, refer to the following documentation:
            {url}
        "}
        } else {
            body.to_string()
        }
    }

    /// Helper method that adds a bang i.e. `!` before strings
    fn bangify(body: impl AsRef<str>) -> String {
        body.as_ref()
            .split('\n')
            .map(|section| format!("! {section}"))
            .collect::<Vec<String>>()
            .join("\n")
    }

    /// Colorizes a body while preserving existing color/reset combinations and clearing before newlines
    ///
    /// Colors with newlines are a problem since the contents stream to git which prepends `remote:` before the `libcnb_test`
    /// if we don't clear, then we will colorize output that isn't ours.
    ///
    /// Explicitly uncolored output is handled by a hacky process of treating two color clears as a special case
    pub(crate) fn colorize(color: &str, body: impl AsRef<str>) -> String {
        body.as_ref()
            .split('\n')
            .map(|section| section.replace(NOCOLOR, NOCOLOR_TMP)) // Explicit no-color hack so it's not cleaned up by accident
            .map(|section| section.replace(RESET, &format!("{RESET}{color}"))) // Handles nested color
            .map(|section| format!("{color}{section}{RESET}")) // Clear after every newline
            .map(|section| section.replace(&format!("{RESET}{color}{RESET}"), RESET)) // Reduce useless color
            .map(|section| section.replace(&format!("{color}{color}"), color)) // Reduce useless color
            .map(|section| section.replace(NOCOLOR_TMP, NOCOLOR)) // Explicit no-color repair
            .collect::<Vec<String>>()
            .join("\n")
    }

    #[cfg(test)]
    mod test {
        use super::*;

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

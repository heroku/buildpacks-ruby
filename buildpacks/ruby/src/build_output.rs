use commons::fun_run;
use libherokubuildpack::command::CommandExt;
use libherokubuildpack::write::line_mapped;
use libherokubuildpack::write::mappers::add_prefix;
use std::io::Write;
use std::{
    process::{Command, Output},
    time::{Duration, Instant},
};

mod time {
    use super::*;

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

fn print_inline(contents: impl AsRef<str>) {
    let contents = contents.as_ref();
    print!("{contents}");
    std::io::stdout().flush().expect("Stdout is writable");
}

pub fn section(topic: impl AsRef<str>) -> section::Section {
    let topic = String::from(topic.as_ref());
    println!("- {topic}");

    section::Section { topic }
}

#[must_use]
pub fn header(buildpack: impl AsRef<str>) -> BuildpackDuration {
    let header = fmt::header(buildpack.as_ref());
    println!("{header}");
    println!("");

    let start = Instant::now();
    BuildpackDuration { start }
}

/// Time the entire buidlpack duration
pub struct BuildpackDuration {
    start: Instant,
}

impl BuildpackDuration {
    /// Emit timing details with done block
    pub fn done_timed(self) {
        let time = time::human(&self.start.elapsed());
        let details = fmt::details(format!("finished in {time}"));
        println!("- Done {details}");
    }

    /// Emit done block without timing details
    pub fn done(self) {
        println!("- Done");
    }

    /// Finish without announcing anything
    pub fn done_silently(self) {}
}

pub mod section {
    use std::thread::{self, JoinHandle};

    use commons::fun_run::CmdError;

    use super::{fmt::DEFAULT_DIM, *};

    const CMD_INDENT: &'static str = "      ";
    const PREFIX: &'static str = "  - ";

    #[derive(Debug, Clone, Eq, PartialEq)]
    pub struct Section {
        pub(crate) topic: String,
    }

    struct RunCommandLol<'a> {
        cmd: &'a mut Command,
        name: Option<String>,
        output: OutputConfig,
    }

    impl<'a> RunCommandLol<'a> {
        fn stream(cmd: &'a mut Command) -> Self {
            Self {
                cmd,
                name: None,
                output: OutputConfig::Stream,
            }
        }

        fn stream_without_timing(cmd: &'a mut Command) -> Self {
            Self {
                cmd,
                name: None,
                output: OutputConfig::StreamNoTiming,
            }
        }

        fn quiet(cmd: &'a mut Command) -> Self {
            Self {
                cmd,
                name: None,
                output: OutputConfig::Quiet,
            }
        }

        fn inline_progress(cmd: &'a mut Command) -> Self {
            Self {
                cmd,
                name: None,
                output: OutputConfig::InlineProgress,
            }
        }
    }

    pub enum OutputConfig {
        Stream,
        StreamNoTiming,
        Quiet,
        InlineProgress,
    }

    pub enum RunCommand<'a> {
        Stream(&'a mut Command),
        Quiet(&'a mut Command),
        Inline(&'a mut Command),
        StreamWithName(&'a mut Command, String),
        QuietWithName(&'a mut Command, String),
        InlineWithName(&'a mut Command, String),
    }
    pub enum CommandDone {
        Stream {
            start: Instant,
            result: Result<Output, CmdError>,
        },
        Quiet {
            start: Instant,
            result: Result<Output, CmdError>,
        },
        Inline {
            start: LiveTimingInline,
            result: Result<Output, CmdError>,
        },
    }

    impl CommandDone {
        // Quiet never outputs
        pub fn done_timed(mut self) -> Result<Output, CmdError> {
            match self {
                CommandDone::Stream { start, result } => {
                    let duration = start.elapsed();
                    let time = fmt::details(time::human(&duration));
                    println!(); // Weird output from prior stream adds indentation that's unwanted
                    println!("{PREFIX}Done {time}");
                    result
                }
                CommandDone::Quiet { start: _, result } => result,
                CommandDone::Inline { mut start, result } => {
                    start.done();

                    result
                }
            }
        }

        // Inline always outputs timing info
        pub fn done(mut self) -> Result<Output, CmdError> {
            match self {
                CommandDone::Stream { start: _, result }
                | CommandDone::Quiet { start: _, result } => result,
                CommandDone::Inline { mut start, result } => {
                    start.done();
                    result
                }
            }
        }
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

    impl LiveTimingInline {
        pub fn new() -> Self {
            let (stop_dots, receiver) = std::sync::mpsc::channel();

            let join_dots = thread::spawn(move || {
                print_inline(fmt::colorize(fmt::DEFAULT_DIM, " ["));

                while true {
                    let msg = receiver.recv_timeout(Duration::from_secs(1));
                    print_inline(fmt::colorize(DEFAULT_DIM, "-"));

                    if msg.is_ok() {
                        print_inline(fmt::colorize(fmt::DEFAULT_DIM, "] "));
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
                self.stop_dots.send(1);
                handle.join().expect("Thread is joinable");
            }
        }

        pub fn done(&mut self) {
            self.stop_dots();
            let time = fmt::details(time::human(&self.start.elapsed()));

            println!("{time}");
        }
    }

    impl Section {
        pub fn say(&self, contents: impl AsRef<str>) {
            let contents = contents.as_ref();
            println!("{PREFIX}{contents}");
        }

        pub fn help(&self, contents: impl AsRef<str>) {
            let (leading_indent, _) = PREFIX
                .split_once("-")
                .expect("Prefix must have a `-` character");

            let contents = fmt::help(contents);

            println!("{leading_indent}{contents}");
        }

        #[must_use]
        pub fn say_with_inline_timer(&self, reason: impl AsRef<str>) -> LiveTimingInline {
            let reason = reason.as_ref();
            print_inline(format!("{PREFIX}{reason}"));

            LiveTimingInline::new()
        }

        #[must_use]
        pub fn run(&self, run_command: RunCommand) -> CommandDone {
            match run_command {
                RunCommand::Stream(command) => Self::stream_command(self, command, None),
                RunCommand::Quiet(command) => Self::silent_command(self, command, None),
                RunCommand::Inline(command) => Self::inline_command(self, command, None),

                RunCommand::StreamWithName(command, name) => {
                    Self::stream_command(self, command, Some(name))
                }
                RunCommand::QuietWithName(command, name) => {
                    Self::silent_command(self, command, Some(name))
                }
                RunCommand::InlineWithName(command, name) => {
                    Self::inline_command(self, command, Some(name))
                }
            }
        }

        /// Run a command and output nothing to the screen
        fn silent_command(
            _section: &Section,
            command: &mut Command,
            custom_name: Option<String>,
        ) -> CommandDone {
            let name = if let Some(custom_name) = custom_name {
                custom_name
            } else {
                fmt::value(fun_run::display(command))
            };

            let start = Instant::now();
            let result = command
                .output()
                .map_err(|error| fun_run::on_system_error(name.clone(), error))
                .and_then(|output| fun_run::nonzero_captured(name, output));

            CommandDone::Quiet { start, result }
        }

        /// Run a command. Output command name, but don't stream the contents
        fn inline_command(
            _section: &Section,
            command: &mut Command,
            custom_name: Option<String>,
        ) -> CommandDone {
            let name = if let Some(custom_name) = custom_name {
                fmt::command(custom_name)
            } else {
                fmt::command(fun_run::display(command))
            };

            print_inline(format!("{PREFIX}Running {name} "));

            let start = Instant::now();
            let output = command.output();

            let result = output
                .map_err(|error| fun_run::on_system_error(name.clone(), error))
                .and_then(|output| fun_run::nonzero_captured(name, output));

            CommandDone::Inline {
                start: LiveTimingInline::new(),
                result,
            }
        }

        /// Run a command. Output command name, and stream the contents
        fn stream_command(
            section: &Section,
            command: &mut Command,
            custom_name: Option<String>,
        ) -> CommandDone {
            let name = if let Some(custom_name) = custom_name {
                fmt::command(custom_name)
            } else {
                fmt::command(fun_run::display(command))
            };

            // Problem if you forget self:: then it will call super::say which is valid code
            // but not what we want
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

            CommandDone::Stream { start, result }
        }
    }
}

pub mod fmt {
    use indoc::formatdoc;
    use std::fmt::Display;

    pub(crate) const RED: &'static str = "\x1B[31m";
    pub(crate) const YELLOW: &'static str = "\x1B[33m";
    pub(crate) const CYAN: &'static str = "\x1B[36m";
    pub(crate) const PURPLE: &'static str = "\x1B[35m"; // magenta

    pub(crate) const BOLD_CYAN: &'static str = "\x1B[1;36m";
    pub(crate) const BOLD_PURPLE: &'static str = "\x1B[1;35m"; // magenta

    pub(crate) const DEFAULT_DIM: &'static str = "\x1B[2;1m"; // Default color but softer/less vibrant
    pub(crate) const RESET: &'static str = "\x1B[0m";
    pub(crate) const NOCOLOR: &'static str = "\x1B[0m\x1B[0m"; //differentiate between color clear and explicit no color
    pub(crate) const NOCOLOR_TMP: &'static str = "🙈🙈🙈"; // Used together with NOCOLOR to act as a placeholder

    pub(crate) const HEROKU_COLOR: &'static str = BOLD_PURPLE;
    pub(crate) const VALUE_COLOR: &'static str = YELLOW;
    pub(crate) const COMMAND_COLOR: &'static str = BOLD_CYAN;
    pub(crate) const URL_COLOR: &'static str = CYAN;
    pub(crate) const IMPORTANT_COLOR: &'static str = CYAN;
    pub(crate) const ERROR_COLOR: &'static str = RED;
    pub(crate) const WARNING_COLOR: &'static str = YELLOW;

    #[must_use]
    pub fn command(contents: impl AsRef<str>) -> String {
        value(colorize(COMMAND_COLOR, contents.as_ref()))
    }

    #[must_use]
    pub fn value(contents: impl AsRef<str>) -> String {
        let contents = colorize(VALUE_COLOR, contents.as_ref());
        format!("`{contents}`")
    }

    #[must_use]
    pub fn details(contents: impl AsRef<str>) -> String {
        let contents = contents.as_ref();
        format!("({contents})")
    }

    #[must_use]
    pub(crate) fn header(contents: impl AsRef<str>) -> String {
        let contents = contents.as_ref();
        colorize(HEROKU_COLOR, format!("\n# {contents}"))
    }

    pub(crate) fn lookatme(
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
            println!("{}", self.to_string());
        }
    }

    impl Display for ErrorInfo {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(error(&self).as_str())
        }
    }

    pub fn error(info: &ErrorInfo) -> String {
        let ErrorInfo {
            header,
            body,
            url,
            debug_details,
        } = info;

        let body = lookatme(ERROR_COLOR, "ERROR:", header, body, url);
        if let Some(details) = debug_details {
            format!("{body}\n\nDebug information: {details}")
        } else {
            body
        }
    }

    pub fn warning(header: impl AsRef<str>, body: impl AsRef<str>, url: Option<String>) -> String {
        let header = header.as_ref();
        let body = body.as_ref();

        lookatme(WARNING_COLOR, "WARNING:", header, body, &url)
    }

    pub fn important(
        header: impl AsRef<str>,
        body: impl AsRef<str>,
        url: Option<String>,
    ) -> String {
        let header = header.as_ref();
        let body = body.as_ref();

        lookatme(IMPORTANT_COLOR, "", header, body, &url)
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

    fn bangify(body: impl AsRef<str>) -> String {
        body.as_ref()
            .split("\n")
            .map(|section| format!("! {section}"))
            .collect::<Vec<String>>()
            .join("\n")
    }

    /// Colorizes a body while preserving existing color/reset combinations and clearing before newlines
    ///
    /// Colors with newlines are a problem since the contents stream to git which prepends `remote:` before the libcnb_test
    /// if we don't clear, then we will colorize output that isn't ours
    ///
    /// Explicitly uncolored output is handled by a hacky process of treating two color clears as a special cases
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
        fn lol() {
            // println!("{}", error("ohno", "nope", None));
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
            //             let out = colorize(RED, "hello\nworld");
            //             let expected = r#"\e[31mhello\033[0m
            // \e[31mworld\033[0m"#;

            //             assert_eq!(expected, &out);
        }

        #[test]
        fn simple_case() {
            // let out = colorize(RED, "hello world");
            // assert_eq!(r#"\e[31mhello world\033[0m"#, &out);
        }
    }
}

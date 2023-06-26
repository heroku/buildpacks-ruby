use commons::fun_run;
use libherokubuildpack::command::CommandExt;
use libherokubuildpack::write::line_mapped;
use libherokubuildpack::write::mappers::add_prefix;
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

pub fn say(contents: impl AsRef<str>) {
    let contents = contents.as_ref();
    println!("- {contents}");
}

pub fn section(topic: impl AsRef<str>) -> section::Section {
    let topic = String::from(topic.as_ref());
    say(topic.clone());

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

pub struct BuildpackDuration {
    start: Instant,
}

impl BuildpackDuration {
    pub fn done_timed(self) {
        let time = time::human(&self.start.elapsed());
        let details = fmt::details(format!("finished in {time}"));
        say(format!("Done {details}"));
    }

    pub fn done(self) {
        say("Done");
    }

    pub fn done_silently(self) {}
}

pub mod section {
    use commons::fun_run::CmdError;

    use super::*;

    const CMD_INDENT: &'static str = "      ";
    const PREFIX: &'static str = "  - ";

    #[derive(Debug, Clone, Eq, PartialEq)]
    pub struct Section {
        pub(crate) topic: String,
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
        Silent {
            start: Instant,
            result: Result<Output, CmdError>,
        },
        Quiet {
            start: Instant,
            result: Result<Output, CmdError>,
        },
    }

    impl CommandDone {
        pub fn done_timed(self) -> Result<Output, CmdError> {
            match self {
                CommandDone::Stream { start, result } => {
                    let duration = start.elapsed();
                    let time = fmt::details(time::human(&duration));
                    println!("{PREFIX}Done {time}");
                    result
                }
                CommandDone::Silent { start: _, result } => result,
                CommandDone::Quiet { start, result } => {
                    let duration = start.elapsed();
                    let time = fmt::details(time::human(&duration));
                    print!(", done {time}");
                    result
                }
            }
        }

        pub fn done(self) -> Result<Output, CmdError> {
            match self {
                CommandDone::Stream { start: _, result }
                | CommandDone::Silent { start: _, result } => result,
                CommandDone::Quiet { start: _, result } => {
                    println!();
                    result
                }
            }
        }
    }

    pub struct TimedSection {
        start: Instant,
    }

    impl TimedSection {
        pub fn done(self) {
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
        pub fn start_timer(&self, reason: impl AsRef<str>) -> TimedSection {
            let reason = reason.as_ref();
            print!("{PREFIX}{reason}");

            TimedSection {
                start: Instant::now(),
            }
        }

        #[must_use]
        pub fn run(&self, run_command: RunCommand) -> CommandDone {
            match run_command {
                RunCommand::Stream(command) => Self::stream_command(command, None),
                RunCommand::Quiet(command) => Self::silent_command(command, None),
                RunCommand::Inline(command) => Self::quiet_command(command, None),
                RunCommand::StreamWithName(command, name) => {
                    Self::stream_command(command, Some(name))
                }
                RunCommand::QuietWithName(command, name) => {
                    Self::silent_command(command, Some(name))
                }
                RunCommand::InlineWithName(command, name) => {
                    Self::quiet_command(command, Some(name))
                }
            }
        }

        fn silent_command(command: &mut Command, custom_name: Option<String>) -> CommandDone {
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

            CommandDone::Silent { start, result }
        }

        fn quiet_command(command: &mut Command, custom_name: Option<String>) -> CommandDone {
            let name = if let Some(custom_name) = custom_name {
                custom_name
            } else {
                fmt::value(fun_run::display(command))
            };

            print!("{PREFIX}");
            print!("Running {name} quietly");

            let start = Instant::now();
            let output = command.output();

            let result = output
                .map_err(|error| fun_run::on_system_error(name.clone(), error))
                .and_then(|output| fun_run::nonzero_captured(name, output));
            CommandDone::Quiet { start, result }
        }

        fn stream_command(command: &mut Command, custom_name: Option<String>) -> CommandDone {
            let name = if let Some(custom_name) = custom_name {
                custom_name
            } else {
                fmt::value(fun_run::display(command))
            };

            say("Running {name}");

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

    const RESET: &'static str = r#"\033[0m"#;
    const RED: &'static str = r#"\e[31m"#;
    const YELLOW: &'static str = r#"\e[11m"#;
    const BLUE: &'static str = r#"\e[34m"#;
    const BOLD_PURPLE: &'static str = r#"\e[1;35m"#; // magenta
    const NOCOLOR: &'static str = r#"\033[0m\033[0m"#; //differentiate between color clear and explicit no color
    const NOCOLOR_TMP: &'static str = r#"🙈🙈🙈"#;

    #[must_use]
    pub fn value(contents: impl AsRef<str>) -> String {
        let contents = colorize(BLUE, contents.as_ref());
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
        colorize(BOLD_PURPLE, format!("# {contents}"))
    }

    pub(crate) fn lookatme(
        color: &str,
        noun: impl AsRef<str>,
        header: impl AsRef<str>,
        body: impl AsRef<str>,
        url: Option<String>,
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

    pub(crate) fn help(contents: impl AsRef<str>) -> String {
        let contents = contents.as_ref();
        let help = colorize(BLUE, "Help");
        bangify(format!("{help}: {contents}"))
    }

    pub fn error(header: impl AsRef<str>, body: impl AsRef<str>, url: Option<String>) -> String {
        let header = header.as_ref();
        let body = body.as_ref();

        lookatme(RED, "ERROR:", header, body, url)
    }

    pub fn warning(header: impl AsRef<str>, body: impl AsRef<str>, url: Option<String>) -> String {
        let header = header.as_ref();
        let body = body.as_ref();

        lookatme(YELLOW, "WARNING:", header, body, url)
    }

    pub fn important(
        header: impl AsRef<str>,
        body: impl AsRef<str>,
        url: Option<String>,
    ) -> String {
        let header = header.as_ref();
        let body = body.as_ref();

        lookatme(BLUE, "", header, body, url)
    }

    fn help_url(body: impl AsRef<str>, url: Option<String>) -> String {
        let body = body.as_ref();

        if let Some(url) = url {
            let url = colorize(NOCOLOR, url);
            formatdoc! {"
            {body}

            For more information, refer to the following documentation:
            {url}
        "}
        } else {
            format!("{body}")
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
    fn colorize(color: &str, body: impl AsRef<str>) -> String {
        body.as_ref()
            .split("\n")
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
            println!("{}", error("ohno", "nope", None));
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
            let nested = colorize(BLUE, "nested");

            let out = colorize(RED, format!("hello {nested} color"));
            let expected = format!("{RED}hello {BLUE}nested{RESET}{RED} color{RESET}");

            assert_eq!(expected, out);
        }

        #[test]
        fn splits_newlines() {
            let out = colorize(RED, "hello\nworld");
            let expected = r#"\e[31mhello\033[0m
\e[31mworld\033[0m"#;

            assert_eq!(expected, &out);
        }

        #[test]
        fn simple_case() {
            let out = colorize(RED, "hello world");
            assert_eq!(r#"\e[31mhello world\033[0m"#, &out);
        }
    }
}

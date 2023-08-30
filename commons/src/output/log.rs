use crate::output::background_timer::{start_timer, StopJoinGuard, StopTimer};
use crate::output::fmt;
use std::fmt::Debug;
use std::io::{stdout, Stdout, Write};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[allow(clippy::wildcard_imports)]
use crate::output::interface::*;

/// # Build output logging
///
/// Use the `BuildLog` to output structured text as a buildpack is executing
///
/// ```
/// use commons::output::{interface::*, log::BuildLog};
///
/// let mut logger = BuildLog::new(std::io::stdout())
///     .buildpack_name("Heroku Ruby Buildpack");
///
/// logger = logger
///     .section("Ruby version")
///     .step_timed("Installing")
///     .finish_timed_step()
///     .end_section();
///
/// logger.finish_logging();
/// ```
///
/// To log inside of a layer see [`section_log`].
///
/// For usage details run `cargo run --bin print_style_guide`

#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct BuildLog<T, W: Debug> {
    pub(crate) io: W,
    pub(crate) state: PhantomData<T>,
    pub(crate) started: Instant,
}

/// Various states for `BuildLog` to contain
///
/// The `BuildLog` struct acts as a logging state machine. These structs
/// are meant to represent those states
pub(crate) mod state {
    #[derive(Debug)]
    pub struct NotStarted;

    #[derive(Debug)]
    pub struct Started;

    #[derive(Debug)]
    pub struct InSection;
}

impl<W> Logger for BuildLog<state::NotStarted, W>
where
    W: Write + Send + Sync + Debug + 'static,
{
    fn buildpack_name(mut self, buildpack_name: &str) -> Box<dyn StartedLogger> {
        write_now(&mut self.io, format!("{}\n\n", fmt::header(buildpack_name)));

        Box::new(BuildLog {
            io: self.io,
            state: PhantomData::<state::Started>,
            started: self.started,
        })
    }

    fn without_buildpack_name(self) -> Box<dyn StartedLogger> {
        Box::new(BuildLog {
            io: self.io,
            state: PhantomData::<state::Started>,
            started: self.started,
        })
    }
}

impl<W> BuildLog<state::NotStarted, W>
where
    W: Write + Debug,
{
    pub fn new(io: W) -> Self {
        Self {
            io,
            state: PhantomData::<state::NotStarted>,
            started: Instant::now(),
        }
    }
}

impl BuildLog<state::NotStarted, Stdout> {
    #[allow(dead_code)]
    fn to_stdout() -> Self {
        Self {
            io: stdout(),
            state: PhantomData::<state::NotStarted>,
            started: Instant::now(),
        }
    }
}

impl BuildLog<state::NotStarted, std::fs::File> {
    #[allow(dead_code)]
    fn to_file(path: &std::path::Path) -> Result<Self, std::io::Error> {
        Ok(Self {
            io: fs_err::File::create(path)?.into(),
            state: PhantomData::<state::NotStarted>,
            started: Instant::now(),
        })
    }
}

impl<W> StartedLogger for BuildLog<state::Started, W>
where
    W: Write + Send + Sync + Debug + 'static,
{
    fn section(mut self: Box<Self>, s: &str) -> Box<dyn SectionLogger> {
        writeln_now(&mut self.io, fmt::section(s));

        Box::new(BuildLog {
            io: self.io,
            state: PhantomData::<state::InSection>,
            started: self.started,
        })
    }

    fn finish_logging(mut self: Box<Self>) {
        let elapsed = fmt::time::human(&self.started.elapsed());
        let details = fmt::details(format!("finished in {elapsed}"));

        writeln_now(&mut self.io, fmt::section(format!("Done {details}")));
    }
}
impl<W> SectionLogger for BuildLog<state::InSection, W>
where
    W: Write + Send + Sync + Debug + 'static,
{
    fn mut_step(&mut self, s: &str) {
        writeln_now(&mut self.io, fmt::step(s));
    }

    fn step(mut self: Box<Self>, s: &str) -> Box<dyn SectionLogger> {
        self.mut_step(s);

        Box::new(BuildLog {
            io: self.io,
            state: PhantomData::<state::InSection>,
            started: self.started,
        })
    }

    fn step_timed(self: Box<Self>, s: &str) -> Box<dyn TimedStepLogger> {
        let start = fmt::step(format!("{s}{}", fmt::background_timer_start()));
        let tick = fmt::background_timer_tick();
        let end = fmt::background_timer_end();

        let arc_io = Arc::new(Mutex::new(self.io));
        let background = start_timer(&arc_io, start, tick, end);

        Box::new(FinishTimedStep {
            arc_io,
            background,
            build_timer: self.started,
        })
    }

    fn step_timed_stream(mut self: Box<Self>, s: &str) -> Box<dyn StreamLogger> {
        self.mut_step(s);

        let started = Instant::now();
        let build_timer = self.started;
        let arc_io = Arc::new(Mutex::new(self.io));
        let mut stream = StreamTimed {
            arc_io,
            started,
            build_timer,
        };
        stream.start();

        Box::new(stream)
    }

    fn end_section(self: Box<Self>) -> Box<dyn StartedLogger> {
        Box::new(BuildLog {
            io: self.io,
            state: PhantomData::<state::Started>,
            started: self.started,
        })
    }
}

impl<T, W> ErrorWarningImportantLogger for BuildLog<T, W>
where
    T: Debug,
    W: Write + Debug,
{
    fn warning(&mut self, s: &str) {
        writeln_now(&mut self.io, fmt::warn(s));
    }

    fn important(&mut self, s: &str) {
        writeln_now(&mut self.io, fmt::important(s));
    }
}

impl<T, W> ErrorLogger for BuildLog<T, W>
where
    T: Debug,
    W: Write + Debug,
{
    fn error(&mut self, s: &str) {
        writeln_now(&mut self.io, fmt::error(s));
    }
}

/// Implements Box<dyn Write + Send + Sync>
///
/// Ensures that the `W` can be passed across thread boundries
/// by wrapping in a mutex.
///
/// It implements writing by unlocking and delegating to the internal writer.
/// Can be used for `Box<dyn StreamLogger>::io()`
#[derive(Debug)]
struct LockedWriter<W> {
    arc: Arc<Mutex<W>>,
}

impl<W> Write for LockedWriter<W>
where
    W: Write + Send + Sync + Debug + 'static,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut io = self.arc.lock().expect("Logging mutex poisoned");
        io.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut io = self.arc.lock().expect("Logging mutex poisoned");
        io.flush()
    }
}

/// Used to implement `Box<dyn StreamLogger>` interface
///
/// Mostly used for logging a running command
#[derive(Debug)]
struct StreamTimed<W> {
    arc_io: Arc<Mutex<W>>,
    started: Instant,
    build_timer: Instant,
}

impl<W> StreamTimed<W>
where
    W: Write + Send + Sync + Debug,
{
    fn start(&mut self) {
        let mut guard = self.arc_io.lock().expect("Internal error");
        let mut io = guard.by_ref();
        // Newline before stream
        writeln_now(&mut io, "");
    }
}

impl<W> StreamLogger for StreamTimed<W>
where
    W: Write + Send + Sync + Debug + 'static,
{
    /// Yield boxed writer that can be used for formatting and streaming contents
    /// back to the logger.
    fn io(&mut self) -> Box<dyn Write + Send + Sync> {
        Box::new(libherokubuildpack::write::line_mapped(
            LockedWriter {
                arc: self.arc_io.clone(),
            },
            fmt::cmd_stream_format,
        ))
    }

    fn finish_timed_stream(self: Box<Self>) -> Box<dyn SectionLogger> {
        let duration = self.started.elapsed();
        let mut io = Arc::try_unwrap(self.arc_io)
            .expect("Internal error")
            .into_inner()
            .expect("Internal error");

        // Newline after stream
        writeln_now(&mut io, "");

        let mut section = BuildLog {
            io,
            state: PhantomData::<state::InSection>,
            started: self.build_timer,
        };

        section.mut_step(&format!(
            "Done {}",
            fmt::details(fmt::time::human(&duration))
        ));

        Box::new(section)
    }
}

/// Implements `Box<dyn FinishTimedStep>`
///
/// Used to end a background inline timer i.e. Installing ...... (<0.1s)
#[derive(Debug)]
struct FinishTimedStep<W> {
    arc_io: Arc<Mutex<W>>,
    background: StopJoinGuard<StopTimer>,
    build_timer: Instant,
}

impl<W> TimedStepLogger for FinishTimedStep<W>
where
    W: Write + Send + Sync + Debug + 'static,
{
    fn finish_timed_step(self: Box<Self>) -> Box<dyn SectionLogger> {
        // Must stop background writing thread before retrieving IO
        let duration = self.background.stop().elapsed();

        let mut io = Arc::try_unwrap(self.arc_io)
            .expect("Internal error")
            .into_inner()
            .expect("Internal error");

        let contents = fmt::details(fmt::time::human(&duration));
        write_now(&mut io, format!("{contents}\n"));

        Box::new(BuildLog {
            io,
            state: PhantomData::<state::InSection>,
            started: self.build_timer,
        })
    }
}

/// Internal helper, ensures that all contents are always flushed (never buffered)
///
/// This is especially important for writing individual characters to the same line
fn write_now<D: Write>(destination: &mut D, msg: impl AsRef<str>) {
    write!(destination, "{}", msg.as_ref()).expect("Internal error: UI writer closed");

    destination
        .flush()
        .expect("Internal error: UI writer closed");
}

/// Internal helper, ensures that all contents are always flushed (never buffered)
fn writeln_now<D: Write>(destination: &mut D, msg: impl AsRef<str>) {
    writeln!(destination, "{}", msg.as_ref()).expect("Internal error: UI writer closed");

    destination
        .flush()
        .expect("Internal error: UI writer closed");
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::output::fmt;
    use crate::output::util::{strip_trailing_whitespace, ReadYourWrite};
    use indoc::formatdoc;
    use libcnb_test::assert_contains;
    use libherokubuildpack::command::CommandExt;

    #[test]
    fn test_captures() {
        let writer = ReadYourWrite::writer(Vec::new());
        let reader = writer.reader();

        let mut stream = BuildLog::new(writer)
            .buildpack_name("Heroku Ruby Buildpack")
            .section("Ruby version `3.1.3` from `Gemfile.lock`")
            .step_timed("Installing")
            .finish_timed_step()
            .end_section()
            .section("Hello world")
            .step_timed_stream("Streaming stuff");

        writeln!(stream.io(), "{}", "stuff").unwrap();

        stream.finish_timed_stream().end_section().finish_logging();

        let actual = strip_trailing_whitespace(fmt::strip_control_codes(String::from_utf8_lossy(
            &reader.lock().unwrap(),
        )));
        let expected = formatdoc! {"

            # Heroku Ruby Buildpack

            - Ruby version `3.1.3` from `Gemfile.lock`
              - Installing ... (< 0.1s)
            - Hello world
              - Streaming stuff

                  stuff

              - Done (< 0.1s)
            - Done (finished in < 0.1s)
        "};

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_streaming_a_command() {
        let writer = ReadYourWrite::writer(Vec::new());
        let reader = writer.reader();

        let mut stream = BuildLog::new(writer)
            .buildpack_name("Streaming buildpack demo")
            .section("Command streaming")
            .step_timed_stream("Streaming stuff");

        std::process::Command::new("echo")
            .arg("hello world")
            .output_and_write_streams(stream.io(), stream.io())
            .unwrap();

        stream.finish_timed_stream().end_section().finish_logging();

        let actual = strip_trailing_whitespace(fmt::strip_control_codes(String::from_utf8_lossy(
            &reader.lock().unwrap(),
        )));

        assert_contains!(actual, "      hello world\n");
    }
}

use crate::output::background_timer::{start_timer, StopDrop, StopIt, StopTimer};
use crate::output::fmt;
#[allow(clippy::wildcard_imports)]
use crate::output::interface::*;
use std::fmt::Debug;
use std::fmt::Display;
use std::io::Write;
use std::io::{stdout, Stdout};
use std::marker::PhantomData;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct BuildLog<T, W> {
    io: W,
    state: PhantomData<T>,
    started: Instant,
}

impl<W> StoppedLogger for BuildLog<state::Stopped, W> {}

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
}

impl BuildLog<state::NotStarted, Stdout> {
    fn stdout() -> Self {
        Self {
            io: stdout(),
            state: PhantomData::<state::NotStarted>,
            started: Instant::now(),
        }
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

    fn finish_logging(mut self: Box<Self>) -> Box<dyn StoppedLogger> {
        let elapsed = fmt::time::human(&self.started.elapsed());
        let details = fmt::details(format!("finished in {elapsed}"));

        writeln_now(&mut self.io, fmt::section(format!("Done {details}")));

        Box::new(BuildLog {
            io: self.io,
            state: PhantomData::<state::Stopped>,
            started: self.started,
        })
    }
}

fn write_now<D: Write>(destination: &mut D, msg: impl AsRef<str>) {
    write!(destination, "{}", msg.as_ref()).expect("Internal error: UI writer closed");

    destination
        .flush()
        .expect("Internal error: UI writer closed");
}

fn writeln_now<D: Write>(destination: &mut D, msg: impl AsRef<str>) {
    writeln!(destination, "{}", msg.as_ref()).expect("Internal error: UI writer closed");

    destination
        .flush()
        .expect("Internal error: UI writer closed");
}

#[derive(Debug)]
struct FinishTimedStep<W> {
    arc_io: Arc<Mutex<W>>,
    background: StopDrop<StopTimer>,
    build_timer: Instant,
}

impl<W> TimedStepLogger for FinishTimedStep<W>
where
    W: Write + Send + Sync + Debug + 'static,
{
    fn finish_timed_step(mut self: Box<Self>) -> Box<dyn SectionLogger> {
        // Must stop background writing thread before retrieving IO
        let duration = self.background.stop();

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

impl<W> SectionLogger for BuildLog<state::InSection, W>
where
    W: Write + Send + Sync + Debug + 'static,
{
    fn step(&mut self, s: &str) {
        writeln_now(&mut self.io, fmt::step(s));
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

    fn step_command(&self, c: &Command) -> Box<dyn SectionLogger> {
        todo!()
    }

    fn end_section(self: Box<Self>) -> Box<dyn StartedLogger> {
        Box::new(BuildLog {
            io: self.io,
            state: PhantomData::<state::Started>,
            started: self.started,
        })
    }
}

impl<T, W: Write> ErrorWarningImportantLogger for BuildLog<T, W> {
    fn warning(&mut self, s: &str) {
        writeln_now(&mut self.io, fmt::warn(s));
    }

    fn important(&mut self, s: &str) {
        writeln_now(&mut self.io, fmt::important(s));
    }
}

impl<T, W: Write> ErrorLogger for BuildLog<T, W> {
    fn error(&mut self, s: &str) {
        writeln_now(&mut self.io, fmt::error(s));
    }
}

mod state {
    #[derive(Debug)]
    pub struct NotStarted;

    #[derive(Debug)]
    pub struct Started;

    #[derive(Debug)]
    pub struct InSection;

    #[derive(Default)]
    pub struct Stopped;
}

#[cfg(test)]
mod test {
    use fs_err::File;
    use indoc::formatdoc;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_captures() {
        let tmp = tempdir().unwrap();
        let logfile = tmp.path().join("log.txt");

        BuildLog {
            io: File::create(&logfile).unwrap(),
            state: PhantomData::<state::NotStarted>,
            started: Instant::now(),
        }
        .buildpack_name("Heroku Ruby Buildpack")
        .section("Ruby version `3.1.3` from `Gemfile.lock`")
        .step_timed("Installing")
        .finish_timed_step()
        .end_section()
        .finish_logging();

        let actual = fs_err::read_to_string(&logfile).unwrap();
        let expected = formatdoc! {"

            # Heroku Ruby Buildpack

            - Ruby version `3.1.3` from `Gemfile.lock`
              - Installing ... (< 0.1s)
            - Done (finished in < 0.1s)
        "};

        assert_eq!(expected, fmt::strip_control_codes(actual));
    }
}

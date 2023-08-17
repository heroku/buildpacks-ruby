use crate::output::background_timer::{start_timer, StopDrop, StopIt, StopTimer};
use crate::output::fmt;
#[allow(clippy::wildcard_imports)]
use crate::output::interface::*;
use std::fmt::Display;
use std::io::Write;
use std::io::{stdout, Stdout};
use std::marker::PhantomData;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Concrete type for alternate logging backend
#[derive(Debug)]
pub enum IOBackend {
    Stdout(Stdout),
    Memory(Vec<u8>),
}

impl Write for IOBackend {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            IOBackend::Stdout(io) => io.write(buf),
            IOBackend::Memory(io) => io.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            IOBackend::Stdout(io) => io.flush(),
            IOBackend::Memory(io) => io.flush(),
        }
    }
}

impl Display for IOBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IOBackend::Stdout(_) => Ok(()),
            IOBackend::Memory(m) => f.write_str(&String::from_utf8_lossy(m)),
        }
    }
}

pub struct Build<T> {
    io: IOBackend,
    state: PhantomData<T>,
    started: Instant,
}

impl<T> Build<T> {
    fn captured(&self) -> String {
        self.io.to_string()
    }
}

impl StoppedLogger for Build<state::Stopped> {}

impl Logger for Build<state::NotStarted> {
    fn start(mut self, buildpack_name: &str) -> Box<dyn StartedLogger> {
        writeln_now(&mut self.io, fmt::header(buildpack_name));

        Box::new(Build {
            io: self.io,
            state: PhantomData::<state::Started>,
            started: self.started,
        })
    }
}

impl Build<state::NotStarted> {
    fn stdout() -> Self {
        Self {
            io: IOBackend::Stdout(stdout()),
            state: PhantomData::<state::NotStarted>,
            started: Instant::now(),
        }
    }

    fn capture() -> Self {
        Self {
            io: IOBackend::Memory(Vec::new()),
            state: PhantomData::<state::NotStarted>,
            started: Instant::now(),
        }
    }
}

impl StartedLogger for Build<state::Started> {
    fn section(mut self: Box<Self>, s: &str) -> Box<dyn SectionLogger> {
        writeln_now(&mut self.io, fmt::section(s));

        Box::new(Build {
            io: self.io,
            state: PhantomData::<state::InSection>,
            started: self.started,
        })
    }

    fn finish_logging(mut self: Box<Self>) -> Box<dyn StoppedLogger> {
        let elapsed = fmt::time::human(&self.started.elapsed());
        let details = fmt::details(format!("finished in {elapsed}"));

        writeln_now(&mut self.io, fmt::section(format!("Done {details}")));

        Box::new(Build {
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
struct FinishTimedStep {
    arc_io: Arc<Mutex<IOBackend>>,
    background: StopDrop<StopTimer>,
    build_timer: Instant,
}

impl TimedStepLogger for FinishTimedStep {
    fn finish_timed_step(mut self: Box<Self>) -> Box<dyn SectionLogger> {
        // Must stop background writing thread before retrieving IO
        let duration = self.background.stop();

        let mut io = Arc::try_unwrap(self.arc_io)
            .expect("Internal error")
            .into_inner()
            .expect("Internal error");

        let contents = fmt::details(fmt::time::human(&duration));
        write_now(&mut io, format!("{contents}\n"));

        Box::new(Build {
            io,
            state: PhantomData::<state::InSection>,
            started: self.build_timer,
        })
    }
}

impl SectionLogger for Build<state::InSection> {
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
        Box::new(Build {
            io: self.io,
            state: PhantomData::<state::Started>,
            started: self.started,
        })
    }
}

impl<T> ErrorWarningImportantLogger for Build<T> {
    fn warning(&mut self, s: &str) {
        writeln_now(&mut self.io, fmt::warn(s));
    }

    fn important(&mut self, s: &str) {
        writeln_now(&mut self.io, fmt::important(s));
    }
}

impl<T> ErrorLogger for Build<T> {
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
    use indoc::formatdoc;
    use std::any::Any;

    use super::*;

    #[test]
    fn test_captures() {
        let logger = Build::capture().start("Ruby Version").finish_logging();

        let actual = (&logger as &dyn std::any::Any)
            .downcast_ref::<Box<Build<state::Stopped>>>()
            .unwrap()
            .captured();

        // let actual = string_from_captured_logger(&logger);
        let expected = formatdoc! {"

        "};

        assert_eq!(expected, actual);
    }
}

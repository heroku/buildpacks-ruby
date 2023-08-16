use lazy_static::__Deref;

use crate::output::fmt;
use crate::output::interface::*;
use std::borrow::BorrowMut;
use std::io::Write;
use std::io::{stdout, Stdout};
use std::marker::PhantomData;
use std::process::Command;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct NotStarted;

#[derive(Debug, Default)]
pub struct Started;

#[derive(Debug, Default)]
pub struct InSection;

struct InTimedStep {
    timer: Instant,

    /// First option is for implementation ergonomics so all state structs implement Default
    /// Second option is for threading as `Box<dyn Write + Send + Sync>` does not implement copy
    /// we have to manually replace memory as the value is moved around. To move it outside of the
    /// arc I needed to replace the contents inside the arc https://users.rust-lang.org/t/take-ownership-of-arc-mutex-t-inner-value/38097/2
    ///
    /// Maybe there's a better way to do this. It makes it gnarly but it works
    thread: Option<JoinHandle<Arc<Mutex<Option<Box<dyn Write + Send + Sync>>>>>>,

    sender: Option<std::sync::mpsc::Sender<()>>,
}

struct NullWriter;

impl Write for NullWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(0)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Default for InTimedStep {
    fn default() -> Self {
        Self {
            thread: Default::default(),
            sender: Default::default(),
            timer: Instant::now(),
        }
    }
}

pub struct Writer<T> {
    state: T,
    destination: Box<dyn Write + Send + Sync>,
}

impl<T> Writer<T> {
    fn write(&mut self, s: impl AsRef<str>) {
        write!(&mut self.destination, "{}", s.as_ref())
            .expect("Internal error: could not write to build output");

        self.destination
            .flush()
            .expect("Internal error: could not write to build output");
    }

    fn writeln(&mut self, s: impl AsRef<str>) {
        writeln!(&mut self.destination, "{}", s.as_ref())
            .expect("Internal error: could not write to build output");

        self.destination
            .flush()
            .expect("Internal error: could not write to build output");
    }
}

impl Logger for Writer<NotStarted> {
    fn start(mut self, buildpack_name: &str) -> Box<dyn StartedLogger> {
        self.writeln(fmt::header(buildpack_name));

        Box::new(Writer {
            state: Default::default(),
            destination: self.destination,
        })
    }
}

impl Writer<NotStarted> {
    fn stdout() -> Self {
        Self {
            state: NotStarted::default(),
            destination: Box::new(stdout()),
        }
    }

    fn capture() -> Self {
        Self {
            state: NotStarted::default(),
            destination: Box::new(Vec::new()),
        }
    }
}

impl StartedLogger for Writer<Started> {
    fn section(mut self: Box<Self>, s: &str) -> Box<dyn SectionLogger> {
        self.writeln(fmt::section(s));

        Box::new(Writer {
            state: Default::default(),
            destination: self.destination,
        })
    }

    fn finish_logging(self: Box<Self>) {
        todo!()
    }
}

impl SectionLogger for Writer<InSection> {
    fn step(&mut self, s: &str) {
        self.writeln(fmt::step(s));
    }

    fn step_timed(mut self: Box<Self>, s: &str) -> Box<dyn TimedStepLogger> {
        self.write(fmt::step(s));

        let (sender, receiver) = std::sync::mpsc::channel::<()>();
        let destination = std::sync::Arc::new(std::sync::Mutex::new(Some(self.destination)));

        let thread = std::thread::spawn(move || {
            {
                let mut output = destination
                    .lock()
                    .expect("Internal error: UI thread unlock")
                    .take()
                    .expect("Internal error: UI Option is None");

                write!(output, " .").expect("Internal error: UI writing dots");
                loop {
                    write!(output, ".").expect("Internal error: UI writing dots");

                    if matches!(
                        receiver.try_recv(),
                        Ok(_) | Err(std::sync::mpsc::TryRecvError::Disconnected)
                    ) {
                        write!(output, ". ").expect("Internal error: UI writing dots");
                        break;
                    }

                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
            destination
        });

        Box::new(Writer {
            state: InTimedStep {
                timer: Instant::now(),
                thread: Some(thread),
                sender: Some(sender),
            },
            destination: Box::new(NullWriter),
        })
    }

    fn step_command(&self, c: &Command) -> Box<dyn SectionLogger> {
        todo!()
    }

    fn end_section(self: Box<Self>) -> Box<dyn StartedLogger> {
        todo!()
    }
}

impl TimedStepLogger for Writer<InTimedStep> {
    fn finish_timed_step(mut self: Box<Self>) -> Box<dyn SectionLogger> {
        // self.destination is a NullWriter at this point, the real destination is inside of self.state.thread
        //
        // We must remove before outputting
        let mut thread = self.state.thread.take();
        let sender = self.state.sender.take();

        sender
            .expect("Internal error: Expected channel")
            .send(())
            .expect("Internal error: UI thread channel is closed");

        let destination = thread
            .expect("Internal error: Expected UI thread join thread handle")
            .join()
            .expect("Internal error: UI thread did not stop")
            .lock()
            .expect("Internal Error: Unlocking UI mutex")
            .take()
            .take()
            .expect("Internal error: UI option is None");

        let mut writer = Box::new(Writer {
            state: InSection,
            destination: Box::new(destination),
        });

        writer.writeln(fmt::details(crate::build_output::time::human(
            &self.state.timer.elapsed(),
        )));
        writer
    }
}

impl<T> ErrorWarningImportantLogger for Writer<T> {
    fn warning(&self, s: &str) {
        todo!()
    }

    fn important(&self, s: &str) {
        todo!()
    }
}

impl<T> ErrorLogger for Writer<T> {
    fn error(self, s: &str) {
        todo!()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_captures() {
        let writer = Writer::capture();
        writer.start("Ruby Version");
    }
}

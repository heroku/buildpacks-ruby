use crate::output::fmt;
use crate::output::interface::*;
use std::io::Write;
use std::io::{stdout, Stdout};
use std::marker::PhantomData;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct NotStarted;

#[derive(Debug, Default)]
pub struct Started;

#[derive(Debug, Default)]
pub struct InSection;

struct InTimedStep {
    timer: Instant,

    // Doesn't need to be an option, this if for implementation ergonomics so all implementations over T for Writer will be `Default`
    thread: Option<std::thread::JoinHandle<()>>,
    sender: Option<std::sync::mpsc::Sender<()>>,

    destination: Arc<Mutex<Box<dyn Write + Send + Sync>>>,
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
            destination: Arc::new(Mutex::new(Box::new(std::io::stdout()))),
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
        let destination = std::sync::Arc::new(std::sync::Mutex::new(self.destination));

        let thread = std::thread::spawn(move || {
            let mut output = destination.lock().unwrap();

            write!(output, " .");
            loop {
                write!(output, ".");

                if matches!(
                    receiver.try_recv(),
                    Ok(_) | Err(std::sync::mpsc::TryRecvError::Disconnected)
                ) {
                    write!(output, " .");
                    break;
                }

                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        });

        Box::new(Writer {
            state: InTimedStep {
                timer: Instant::now(),
                thread: Some(thread),
                sender: Some(sender),
                destination,
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
        let contents = crate::build_output::time::human(&self.state.timer.elapsed());
        self.writeln(contents);

        // Box::new(Writer {
        //     state: InTimedStep::new(),
        //     destination: self.destination,
        // })
        todo!()
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

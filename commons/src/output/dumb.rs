use crate::output::interface::*;
use std::ops::Deref;
use std::process::Command;
use std::time::Duration;

// Extremely stupid implementation just to prove it's implementable

pub(crate) enum DumbLogger {
    NotStarted,
    Started,
    InSection,
    InTimedStep { sender: std::sync::mpsc::Sender<()> },
}

impl DumbLogger {
    pub(crate) fn new() -> Self {
        DumbLogger::NotStarted
    }
}

impl Logger for DumbLogger {
    fn start(self, s: &str) -> Box<dyn StartedLogger> {
        match self {
            DumbLogger::NotStarted => Box::new(Self::Started),
            _ => panic!(""),
        }
    }
}

impl StartedLogger for DumbLogger {
    fn section(self: Box<Self>, s: &str) -> Box<dyn SectionLogger> {
        println!("-> {s}");
        Box::new(Self::InSection)
    }

    fn finish_logging(self: Box<Self>) {
        println!("ALL DONE; WHEEEE!");
    }
}

impl SectionLogger for DumbLogger {
    fn step(&self, s: &str) -> Box<dyn SectionLogger> {
        println!("   - step: {s}");
        Box::new(Self::InSection)
    }

    fn step_timed(self: Box<Self>, s: &str) -> Box<dyn TimedStepLogger> {
        print!("   - timed step: {s}");

        let (sender, receiver) = std::sync::mpsc::channel();

        std::thread::spawn(move || loop {
            print!(".");

            if matches!(
                receiver.try_recv(),
                Ok(_) | Err(std::sync::mpsc::TryRecvError::Disconnected)
            ) {
                break;
            }

            std::thread::sleep(Duration::from_secs(1));
        });

        Box::new(Self::InTimedStep { sender })
    }

    fn step_command(&self, c: &Command) -> Box<dyn SectionLogger> {
        todo!()
    }

    fn end_section(self: Box<Self>) -> Box<dyn StartedLogger> {
        println!("\n");
        Box::new(Self::Started)
    }
}

impl TimedStepLogger for DumbLogger {
    fn finish_timed_step(self: Box<Self>) -> Box<dyn SectionLogger> {
        match self.deref() {
            DumbLogger::InTimedStep { sender } => {
                println!("done!");
                sender.send(()).unwrap();
                Box::new(Self::InSection)
            }
            _ => panic!(),
        }
    }
}

impl ErrorLogger for DumbLogger {
    fn error(self, s: &str) {
        match self {
            DumbLogger::NotStarted => panic!(),
            DumbLogger::Started => {
                eprintln!("Error: {s}")
            }
            DumbLogger::InSection => {
                // TODO: Cleaning up section state before output
                eprintln!("Error: {s}")
            }
            DumbLogger::InTimedStep { .. } => {
                // TODO: Cleaning up timed step state before output
                eprintln!("Error: {s}")
            }
        }
    }
}

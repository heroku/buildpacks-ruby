use std::fmt::Debug;
use std::io::Write;

pub trait Logger: Debug {
    fn buildpack_name(self, s: &str) -> Box<dyn StartedLogger>;
    fn without_buildpack_name(self) -> Box<dyn StartedLogger>;
}

pub trait StartedLogger: ErrorWarningImportantLogger + Debug {
    fn section(self: Box<Self>, s: &str) -> Box<dyn SectionLogger>;
    fn finish_logging(self: Box<Self>);
}

pub trait StreamLogger: Debug {
    fn io(&mut self) -> Box<dyn Write + Send + Sync + 'static>;
    fn finish_timed_stream(self: Box<Self>) -> Box<dyn SectionLogger>;
}

pub trait SectionLogger: ErrorWarningImportantLogger + Debug {
    fn step(self: Box<Self>, s: &str) -> Box<dyn SectionLogger>;
    fn mut_step(&mut self, s: &str);
    fn step_timed(self: Box<Self>, s: &str) -> Box<dyn TimedStepLogger>;
    fn step_timed_stream(self: Box<Self>, s: &str) -> Box<dyn StreamLogger>;
    fn end_section(self: Box<Self>) -> Box<dyn StartedLogger>;
}

pub trait TimedStepLogger: Debug {
    fn finish_timed_step(self: Box<Self>) -> Box<dyn SectionLogger>;
}

// Object safety needs to be sorted out
pub trait ErrorWarningImportantLogger: ErrorLogger + Debug {
    /// TODO: make this chainable
    fn warning(&mut self, s: &str);
    fn important(&mut self, s: &str);
}

pub trait ErrorLogger: Debug {
    fn error(&mut self, s: &str);
}

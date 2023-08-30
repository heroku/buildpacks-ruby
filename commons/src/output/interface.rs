use std::fmt::Debug;
use std::io::Write;

pub trait Logger: Debug {
    fn buildpack_name(self, s: &str) -> Box<dyn StartedLogger>;
    fn without_buildpack_name(self) -> Box<dyn StartedLogger>;
}

pub trait StartedLogger: Debug {
    fn section(self: Box<Self>, s: &str) -> Box<dyn SectionLogger>;
    fn finish_logging(self: Box<Self>);

    fn announce(self: Box<Self>) -> Box<dyn StartedAnnounceLogger>;
}

pub trait SectionLogger: Debug {
    fn step(self: Box<Self>, s: &str) -> Box<dyn SectionLogger>;
    fn mut_step(&mut self, s: &str);
    fn step_timed(self: Box<Self>, s: &str) -> Box<dyn TimedStepLogger>;
    fn step_timed_stream(self: Box<Self>, s: &str) -> Box<dyn StreamLogger>;
    fn end_section(self: Box<Self>) -> Box<dyn StartedLogger>;

    fn announce(self: Box<Self>) -> Box<dyn SectionAnnounceLogger>;
}

pub trait StartedAnnounceLogger: ErrorLogger + Debug {
    fn warning(self: Box<Self>, s: &str) -> Box<dyn StartedAnnounceLogger>;
    fn important(self: Box<Self>, s: &str) -> Box<dyn StartedAnnounceLogger>;

    fn end_announce(self: Box<Self>) -> Box<dyn StartedLogger>;
}

pub trait SectionAnnounceLogger: ErrorLogger + Debug {
    fn warning(self: Box<Self>, s: &str) -> Box<dyn SectionAnnounceLogger>;
    fn important(self: Box<Self>, s: &str) -> Box<dyn SectionAnnounceLogger>;

    fn end_announce(self: Box<Self>) -> Box<dyn SectionLogger>;
}

pub trait TimedStepLogger: Debug {
    fn finish_timed_step(self: Box<Self>) -> Box<dyn SectionLogger>;
}

pub trait StreamLogger: Debug {
    fn io(&mut self) -> Box<dyn Write + Send + Sync + 'static>;
    fn finish_timed_stream(self: Box<Self>) -> Box<dyn SectionLogger>;
}

pub trait ErrorLogger: Debug {
    fn error(self: Box<Self>, s: &str);
}

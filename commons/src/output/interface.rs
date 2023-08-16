use std::process::Command;

pub(crate) enum LoggerState {
    NotStarted,
    Started,
    InSection,
    InTimedStep,
}

pub(crate) trait Logger {
    fn start(self, s: &str) -> Box<dyn StartedLogger>;
}

pub(crate) trait StartedLogger: ErrorWarningImportantLogger {
    fn section(self: Box<Self>, s: &str) -> Box<dyn SectionLogger>;
    fn finish_logging(self: Box<Self>);
}

pub(crate) trait SectionLogger: ErrorWarningImportantLogger {
    fn step(&mut self, s: &str); // -> Box<dyn SectionLogger>;
    fn step_timed(self: Box<Self>, s: &str) -> Box<dyn TimedStepLogger>;
    fn step_command(&self, c: &Command) -> Box<dyn SectionLogger>;
    fn end_section(self: Box<Self>) -> Box<dyn StartedLogger>;
}

pub(crate) trait TimedStepLogger: ErrorWarningImportantLogger {
    fn finish_timed_step(self: Box<Self>) -> Box<dyn SectionLogger>;
}

// Object safety needs to be sorted out
pub(crate) trait ErrorWarningImportantLogger: ErrorLogger {
    /// TODO: make this chainable
    fn warning(&self, s: &str);
    fn important(&self, s: &str);
}

pub(crate) trait ErrorLogger {
    fn error(self, s: &str);
}

// print_header() -- Example: print the buildpack
// print_header_with_timer() -- Example: print the buildpack
// print_section() -- Example: entering a section of related things (typically a noun/topic)
// print_step() -- Example: sub-bullet (typically a verb, running, installing, verb)
// print_help_step() -- same as above, different styling and prefix
// print_step_with_inline_timer() -- step, prints immediately what it's doing, printing dot every second, when stopped wraps up.
// --> stop_timer()
// print_step_command() -- As above, command will be run internally, timer stopped, etc.
// print_error(error_spec)
// print_warning(warn_spec)
// print_important(important_spec) -- not a problem, but heads up

// In addition: formatting of sub-text like URLs, "values", env_vars, etc.

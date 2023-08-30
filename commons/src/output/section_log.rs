/// Output logging within a section
///
/// ## Context
///
/// The `impl Layer` has methods that are not mutible and `Box<dyn impl SectionLogger>`
/// is not `Clone` or `Copy`. So we need some way of allowing layers to log via `SectionLogger` without
/// those structs being mutable. We must also get a `Box<dyn SectionLogger>` back out so that we can
/// finalize the section and start logging some thing new.
///
/// Instead of jumping through hoops implementing interior mutability this module exposes functions
/// and assumes that they are called within a `SectionLogger` context.
///
/// ## Use
///
/// The main use case is logging inside of a layer:
///
/// ```no_run
/// use commons::output::section_log;
///
/// // fn create(
/// //     &self,
/// //     context: &libcnb::build::BuildContext<Self::Buildpack>,
/// //     layer_path: &std::path::Path,
/// // ) -> Result<
/// //     libcnb::layer::LayerResult<Self::Metadata>,
/// //     <Self::Buildpack as libcnb::Buildpack>::Error,
/// // > {
///     section_log::step_timed("Installing", || {
///         // Install logic here
///         todo!()
///     })
/// // }
/// ```

#[allow(clippy::wildcard_imports)]
use crate::output::interface::*;
use crate::output::log::{state, BuildLog};
use std::io::Stdout;
use std::marker::PhantomData;
use std::time::Instant;

/// Output a message as a single step, ideally a short message
///
/// ```
/// use commons::output::section_log;
///
/// section_log::step("Clearing cache (ruby version changed)");
/// ```
pub fn step(s: impl AsRef<str>) {
    logger().step(s.as_ref());
}

/// Will print the input string followed by a background timer
/// that will emit to the UI until the passed in function ends
///
/// ```
/// use commons::output::section_log;
///
/// section_log::step_timed("Installing", || {
///     // Install logic here
/// });
/// ```
///
/// Timing information will be output at the end of the step.
pub fn step_timed<T>(s: impl AsRef<str>, f: impl FnOnce() -> T) -> T {
    let timer = logger().step_timed(s.as_ref());
    let out = f();
    timer.finish_timed_step();
    out
}

/// Will print the input string and yield a `Box<dyn StreamLogger>` that can be used to print
/// to the output. The main use case is running commands
///
/// ```no_run
/// use commons::fun_run::CommandWithName;
/// use commons::output::{section_log, fmt};
///
/// let mut cmd = std::process::Command::new("bundle");
/// cmd.arg("install");
///
/// section_log::step_stream(format!("Running {}", fmt::command(cmd.name())), |stream| {
///     cmd.stream_output(stream.io(), stream.io()).unwrap()
/// });
/// ```
///
/// Timing information will be output at the end of the step.
pub fn step_stream<T>(s: impl AsRef<str>, f: impl FnOnce(&mut Box<dyn StreamLogger>) -> T) -> T {
    let mut stream = logger().step_timed_stream(s.as_ref());
    let out = f(&mut stream);
    stream.finish_timed_stream();
    out
}

/// Print an error block to the output
pub fn error(s: impl AsRef<str>) {
    logger().error(s.as_ref());
}

/// Print an warning block to the output
pub fn warning(s: impl AsRef<str>) {
    logger().warning(s.as_ref());
}

/// Print an important block to the output
pub fn important(s: impl AsRef<str>) {
    logger().important(s.as_ref());
}

/// Write to the build output in a `Box<dyn SectionLogger>` format with functions
///
/// ## What
///
/// Logging from within a layer can be difficult because calls to the layer interface are not
/// mutable nor consumable. Functions can be used at any time with no restrictions. The
/// only downside is that the buildpack author (you) is now responsible for:
///
/// - Ensuring that `Box<dyn StartedLogger>::section()` was called right before any of these
/// functions are called.
/// - Ensuring that you are not attempting to log while already logging i.e. calling `step()` within a
/// `step_timed()` call.
///
/// For usage details run `cargo run --bin print_style_guide`
fn logger() -> Box<dyn SectionLogger> {
    Box::new(BuildLog::<state::InSection, Stdout> {
        io: std::io::stdout(),
        state: PhantomData,
        started: Instant::now(),
    })
}

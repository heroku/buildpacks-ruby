//! Write to the build output in a `Box<dyn SectionLogger>` format with functions
//!
//! ## What
//!
//! Logging from within a layer can be difficult because calls to the layer interface are not
//! mutable nor consumable. Functions can be used at any time with no restrictions. The
//! only downside is that the buildpack author (you) is now responsible for:
//!
//! - Ensuring that `Box<dyn StartedLogger>::section()` was called right before any of these
//!   functions are called.
//! - Ensuring that you are not attempting to log while already logging i.e. calling `step()` within a
//!   `step_timed()` call.
//!
//! ## Use
//!
//! The main use case is logging inside of a layer:
//!
//! ```no_run
//! use commons::output::section_log::log_step_timed;
//!
//! // fn create(
//! //     &self,
//! //     context: &libcnb::build::BuildContext<Self::Buildpack>,
//! //     layer_path: &std::path::Path,
//! // ) -> Result<
//! //     libcnb::layer::LayerResult<Self::Metadata>,
//! //     <Self::Buildpack as libcnb::Buildpack>::Error,
//! // > {
//! log_step_timed("Installing", || {
//!         // Install logic here
//!         todo!()
//!     })
//! // }
//! ```
use crate::output::build_log::{state, BuildData, BuildLog};
#[allow(clippy::wildcard_imports)]
pub use crate::output::interface::*;
use std::io::Stdout;
use std::marker::PhantomData;

/// Output a message as a single step, ideally a short message
///
/// ```
/// use commons::output::section_log::log_step;
///
/// log_step("Clearing cache (ruby version changed)");
/// ```
#[deprecated(note = "Use `bullet_stream` instead")]
pub fn log_step(s: impl AsRef<str>) {
    logger().step(s.as_ref());
}

/// Will print the input string followed by a background timer
/// that will emit to the UI until the passed in function ends
///
/// ```
/// use commons::output::section_log::log_step_timed;
///
/// log_step_timed("Installing", || {
///     // Install logic here
/// });
/// ```
///
/// Timing information will be output at the end of the step.
#[deprecated(note = "Use `bullet_stream` instead")]
pub fn log_step_timed<T>(s: impl AsRef<str>, f: impl FnOnce() -> T) -> T {
    let timer = logger().step_timed(s.as_ref());
    let out = f();
    timer.finish_timed_step();
    out
}

#[deprecated(note = "Use `bullet_stream` instead")]
pub fn log_step_stream<T>(
    s: impl AsRef<str>,
    f: impl FnOnce(&mut Box<dyn StreamLogger>) -> T,
) -> T {
    let mut stream = logger().step_timed_stream(s.as_ref());
    let out = f(&mut stream);
    stream.finish_timed_stream();
    out
}

/// Print an error block to the output
#[deprecated(note = "Use `bullet_stream` instead")]
pub fn log_error(s: impl AsRef<str>) {
    logger().announce().error(s.as_ref());
}

/// Print an warning block to the output
#[deprecated(note = "Use `bullet_stream` instead")]
pub fn log_warning(s: impl AsRef<str>) {
    logger().announce().warning(s.as_ref());
}

/// Print an warning block to the output at a later time
#[deprecated(note = "Use `bullet_stream` instead")]
pub fn log_warning_later(s: impl AsRef<str>) {
    logger().announce().warn_later(s.as_ref());
}

/// Print an important block to the output
#[deprecated(note = "Use `bullet_stream` instead")]
pub fn log_important(s: impl AsRef<str>) {
    logger().announce().important(s.as_ref());
}

fn logger() -> Box<dyn SectionLogger> {
    Box::new(BuildLog::<state::InSection, Stdout> {
        io: std::io::stdout(),
        // Be careful not to do anything that might access this state
        // as it's ephemeral data (i.e. not passed in from the start of the build)
        data: BuildData::default(),
        state: PhantomData,
    })
}

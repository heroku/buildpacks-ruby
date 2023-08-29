/// Output logging within a section
///
/// TODO: Example usage in a layer
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

#[allow(clippy::wildcard_imports)]
use crate::output::interface::*;
use crate::output::log::{state, BuildLog};
use std::io::Stdout;
use std::marker::PhantomData;
use std::time::Instant;

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

pub fn step(s: impl AsRef<str>) {
    logger().step(s.as_ref());
}

pub fn step_timed<T>(s: impl AsRef<str>, f: impl FnOnce() -> T) -> T {
    let timer = logger().step_timed(s.as_ref());
    let out = f();
    timer.finish_timed_step();
    out
}

pub fn step_stream<T>(s: impl AsRef<str>, f: impl FnOnce(&mut Box<dyn StreamLogger>) -> T) -> T {
    let mut stream = logger().step_timed_stream(s.as_ref());
    let out = f(&mut stream);
    stream.finish_timed_stream();
    out
}

pub fn error(s: impl AsRef<str>) {
    logger().error(s.as_ref());
}

pub fn warning(s: impl AsRef<str>) {
    logger().warning(s.as_ref());
}

pub fn important(s: impl AsRef<str>) {
    logger().important(s.as_ref());
}

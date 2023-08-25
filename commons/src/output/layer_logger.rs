use std::cell::OnceCell;
use std::sync::{Arc, Mutex, MutexGuard};

#[allow(clippy::wildcard_imports)]
use crate::output::interface::*;

/// Top level interface for to be used for logging within a layer
///
/// ## Use
///
/// ```
///
/// use commons::output::{interface::*, log::BuildLog, layer_logger::LayerLogger};
///
/// let mut build_log = BuildLog::new(std::io::stdout())
///     .buildpack_name("Documentation");
///
/// let section_log = build_log.section("test");
///
/// // Before calling `handle_layer` generate a `LayerLogger` instance
/// // that can be shared with the layer
/// let mut layer_logger = LayerLogger::new(section_log);
///
/// # let logger = layer_logger.clone(); // hidden line, fake the handle_layer interface but ensure this method exists
///
/// // // Pass it into your layer as a field when calling `handle_layer`
/// // context.handle_layer( layer_name!("my_layer") ,
/// //     MyLayerStruct {
/// //         logger: layer_logger.clone()
/// //     }
/// // ).unwrap();
///
/// // From within the layer call `lock()` and then your desired
/// // logging associated function
/// logger.lock().step("Clearing cache");
/// logger.lock().step_timed("Installing", || {
///     // ...
/// });
/// # drop(logger);
///
/// // Once the layer is done executing the section can be finalized
/// // This will consume the layer_logger
/// let build_log = layer_logger.finish_layer();
/// ```
///
/// If you try to log within a log using the same lock
/// instance it will fail to compile:
///
/// ```compile_fail
/// use commons::output::{interface::*, log::{BuildLog, LayerLogger}};
///
/// let log = BuildLog::new(std::io::stdout())
///     .buildpack_name("Testing")
///     .section("test");
///
/// let layer_logger = LayerLogger::new(log);
///
/// let lock = layer_logger.lock();
/// lock.step_timed("Installing", || {
///     // Cannot use while in use
///     lock.step("Clearing cache");
///     // ...
/// });
/// ```
///
/// ## What problem does this solve?
///
/// The `Box<dyn impl SectionLogger>` logging interface needs to be mutible is the logging interface is
/// a state machine that consumes itself.
///
/// The `impl Layer` has methods that are not mutible and `Box<dyn impl SectionLogger>`
/// is not `Clone` or `Copy`. So we need some way of allowing layers to log via `SectionLogger` without
/// those structs being mutable. We must also get a `Box<dyn SectionLogger>` back out so that we can
/// finalize the section and start logging some thing new.
///
/// To achieve that this struct uses interior mutability.
///
/// It attempts to preserve some of the stateful semantics of the underlying `Box<dyn SectionLogger>`
/// but that means it needs runtime checks and can panic.
///
/// ## Panics
///
/// Because this struct uses interior mutability not all consistency/accuracy checks
/// can be performed at compile time. This code will compile correctly but
/// panic when called:
///
/// ```should_panic
/// use commons::output::{interface::*, layer_logger::LayerLogger, log::BuildLog};
///
/// let log = BuildLog::new(std::io::stdout())
///     .buildpack_name("Testing")
///     .section("test");
///
/// let layer_logger = LayerLogger::new(log);
///
/// layer_logger.lock().step_timed("Installing", || {
///     // Cannot use while in use
///     layer_logger.lock().step("Clearing cache");
///     // ...
/// });
/// ```
///
#[derive(Debug)]
pub struct LayerLogger {
    inner: Arc<Mutex<SectionLoggerCell>>,
}

impl<'a> LayerLogger {
    #[must_use]
    pub fn new(logger: Box<dyn SectionLogger>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(SectionLoggerCell::new(logger))),
        }
    }

    #[must_use]
    pub fn clone(&mut self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }

    /// # Panics
    ///
    /// Runtime panic if the interior is already locked
    #[must_use]
    pub fn lock(&'a self) -> GuardedLayerLogger<'a> {
        let lock = self.inner.try_lock().expect("Cannot call lock twice");
        GuardedLayerLogger { inner: lock }
    }

    /// # Panics
    ///
    /// Will panic if there are any surviving clones besides the instance where this is called
    #[must_use]
    pub fn finish_layer(self) -> Box<dyn StartedLogger> {
        let inner = Arc::try_unwrap(self.inner)
            .expect("Layer logger still in use, ensure all clones are dropped before calling the finish_layer associated function")
            .into_inner()
            .expect("Inner error: mutex not poisoned")
            .end_section();
        inner
    }
}

/// Internal struct for consistency guarantees
///
/// We are using a `OnceCell` to allow for interior mutability.
///
/// Several of these associated functions can panic, but the exposed
/// interface is expected to safe for the user. Any panics are internal
/// bugs
///
/// The primary purpose of this struct is to ensure that the interior `OnceCell`
/// is always populated (not None). If that holds true, then this will not panic.
#[derive(Debug)]
struct SectionLoggerCell {
    // Not `pub(crate)` so all manipulation must be through associated functions
    inner: OnceCell<Box<dyn SectionLogger>>,
}

impl SectionLoggerCell {
    pub(crate) fn new(logger: Box<dyn SectionLogger>) -> Self {
        let cell = OnceCell::new();
        let _ = cell.get_or_init(|| logger);
        Self { inner: cell }
    }

    /// # Panics
    ///
    /// Expects the `OnceCell` to always be populated. As long as all updates are made via this
    /// function we ensure that a logger is always set.
    pub(crate) fn update<T>(
        &mut self,
        f: impl FnOnce(Box<dyn SectionLogger>) -> (Box<dyn SectionLogger>, T),
    ) -> T {
        let logger = self.inner.take().expect("Internal error");
        let (log, out) = f(logger);
        self.inner.set(log).expect("Internal error");
        out
    }

    /// # Panics
    ///
    /// Expects the `OnceCell` to always be populated. As long as all updates are made via `update`
    /// we ensure that a logger is always set.
    pub(crate) fn get_mut(&mut self) -> &mut Box<dyn SectionLogger> {
        self.inner.get_mut().expect("Internal error")
    }

    /// # Panics
    ///
    /// Expects the `OnceCell` to always be populated. As long as all updates are made via `update`
    /// we ensure that a logger is always set.
    ///
    /// All interfaces that make internal state invalid are consuming interfaces.
    #[must_use]
    pub(crate) fn end_section(mut self) -> Box<dyn StartedLogger> {
        self.inner.take().expect("Internal error").end_section()
    }
}

/// Guarded interface to the underlying logger
///
/// By wrapping the underlying `SectionLoggerCell` in a guard we can
/// ensure that it is never accessed twice (via runtime panic guarantee).
///
/// This prevents accidentally logging while something is either streaming or
/// printing in the background.
///
#[allow(clippy::module_name_repetitions)]
pub struct GuardedLayerLogger<'a> {
    inner: MutexGuard<'a, SectionLoggerCell>,
}

impl<'a> GuardedLayerLogger<'a> {
    pub fn step_stream<T>(
        mut self,
        s: impl AsRef<str>,
        f: impl FnOnce(&mut Box<dyn StreamLogger>) -> T,
    ) -> T {
        self.inner.update(move |logger| {
            let mut log = logger.step_timed_stream(s.as_ref());
            let out = f(&mut log);

            (log.finish_timed_stream(), out)
        })
    }

    pub fn step_timed<T>(mut self, s: impl AsRef<str>, f: impl FnOnce() -> T) -> T {
        self.inner.update(move |logger| {
            let log = logger.step_timed(s.as_ref());
            let out = f();

            (log.finish_timed_step(), out)
        })
    }

    pub fn step(mut self, s: impl AsRef<str>) {
        self.inner.get_mut().mut_step(s.as_ref());
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::output::fmt;
    use crate::output::log::BuildLog;
    use crate::output::util::ReadYourWrite;
    use libcnb_test::assert_contains;

    #[test]
    fn layer_logger_interface() {
        let writer = ReadYourWrite::writer(Vec::new());
        let reader = writer.reader();
        let log = BuildLog::new(writer)
            .buildpack_name("Testing")
            .section("test");

        let logger = LayerLogger::new(log);

        logger.lock().step("hello");
        {
            let mut vec = reader.lock().unwrap();
            let actual = fmt::strip_control_codes(String::from_utf8_lossy(&vec));
            assert_contains!(actual, "  - hello\n");
            vec.clear();
        }

        let out = logger.lock().step_timed("world", || 1);
        {
            assert_eq!(out, 1);

            let mut vec = reader.lock().unwrap();
            let actual = fmt::strip_control_codes(String::from_utf8_lossy(&vec));
            assert_contains!(actual, "  - world ... (< 0.1s)\n");
            vec.clear();
        }

        logger.lock().step_stream("streamed", |log| {
            writeln!(log.io(), "like ice cream").unwrap();
        });
        {
            let mut vec = reader.lock().unwrap();
            let actual = fmt::strip_control_codes(String::from_utf8_lossy(&vec));
            assert_contains!(actual, "  - streamed\n\n      like ice cream\n");
            vec.clear();
        }

        assert!(
            std::panic::catch_unwind(|| {
                logger
                    .lock()
                    .step_timed("timed", || logger.lock().step("double lock"));
            })
            .is_err(),
            "Expected runtime error due to double lock"
        );
    }
}

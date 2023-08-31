use indoc::formatdoc;
use lazy_static::lazy_static;
use std::fmt::Debug;
use std::io::{stderr, Stderr, Write};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

lazy_static! {
    ///  Acquire a lock to this mutex in any test that manipulates the globals in this file
    /// failure to aquire this lock when using delayed warning in tests will result in race
    /// conditions and random failures
    static ref TEST_ACCESS: Arc<Mutex<()>> = Arc::new(Mutex::new(()));
}

lazy_static! {
    /// Stores a vector of strings to print as warnings later
    static ref WARN_LATER: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));
}
/// Stores a count of existing `WarnGuard`s so that we can warn buildpack developers if they've tried
/// to `warn_later` without an alive `WarnGuard`.
static GUARD_COUNT: AtomicI64 = AtomicI64::new(0);

/// Queue a warning for later
///
/// Build logs can be quite large and people don't always scroll back up to read every line. Delaying
/// a warning and emitting it right before the end of the build can increase the chances the app
/// developer will read it.
///
/// ## Use - Setup a `WarnGuard` in your buildpack
///
/// To ensure warnings are printed, even in the event of errors, you must create a `WarnGuard`
/// in your buildpack that will print any delayed warnings when dropped:
///
/// ```no_run
/// // src/main.rs
/// use commons::output::warn_later::WarnGuard;
///
/// // fn build(&self, context: BuildContext<Self>) -> libcnb::Result<BuildResult, Self::Error> {
///     let warn_later = WarnGuard::new();
///     // ...
///
///
///     // Warnings will be emitted if the warn guard is dropped
///     drop(warn_later);
/// // }
/// ```
///
/// Alternatively you can manually print delayed warnings:
///
/// ```no_run
/// use commons::output::warn_later::WarnGuard;
///
/// // fn build(&self, context: BuildContext<Self>) -> libcnb::Result<BuildResult, Self::Error> {
///     let warn_later = WarnGuard::new();
///     // ...
///
///
///     // Consumes the guard, prints and clears any delayed warnings.
///     warn_later.warn_now();
/// // }
/// ```
///
/// ## Use - Issue a delayed warning
///
/// Once a warn guard is in place you can queue a warning using `section_log::*` or `build_log::*`:
///
/// ```no_run
/// use commons::output::section_log::*;
///
/// log_warning_later("WARNING: Live without warning");
/// ```
///
/// ```no_run
/// use commons::output::build_log::*;
///
/// BuildLog::new(std::io::stdout())
///     .buildpack_name("Julius Caesar")
///     .announce()
///     .warn_later("Beware the ides of march");
/// ```

/// Pushes a string to a global warning vec for to be emitted later
pub(crate) fn push(s: impl AsRef<str>) {
    warn_if_no_guards(std::io::stderr()).expect("stderr to be writeable");

    let mut guard = match WARN_LATER.lock() {
        Ok(guard) => guard,
        Err(error) => {
            eprintln!(
                "[Internal warning]: Thread recovered from mutex poisoning (delayed warning): {error}"
            );
            error.into_inner()
        }
    };
    guard.push(s.as_ref().to_string());
}

/// Removes all delayed warnings from global vector
pub(crate) fn drain() -> Vec<String> {
    let mut guard = match WARN_LATER.lock() {
        Ok(guard) => guard,
        Err(error) => {
            eprintln!(
                "[Internal warning]: Thread recovered from mutex poisoning (delayed warning): {error}"
            );
            error.into_inner()
        }
    };
    guard.drain(..).collect()
}

#[derive(Debug)]
pub struct WarnGuard<W>
where
    W: Write + Debug,
{
    // Private inner to force public construction through `new()` which tracks global state.
    io: W,
}

impl WarnGuard<Stderr> {
    #[must_use]
    #[allow(clippy::new_without_default)] // Ensure guard count is always updated
    pub fn new() -> Self {
        Self::with_io(stderr())
    }
}

impl<W> WarnGuard<W>
where
    W: Write + Debug,
{
    fn with_io(io: W) -> Self {
        GUARD_COUNT.fetch_add(1, Ordering::Relaxed);
        Self { io }
    }

    /// Consumes self, prints and drains all existing delayed warnings
    pub fn warn_now(self) {
        drop(self);
    }
}

/// Emits a warning if no `WarnGuard`'s have been created
fn warn_if_no_guards<W: Write>(mut io: W) -> std::io::Result<()> {
    if GUARD_COUNT.fetch_add(0, Ordering::Relaxed) <= 0 {
        writeln!(
            io,
            "{}",
            formatdoc! {"
                    [Internal warning]: Delayed warnings may not be emitted by the buildpack.
                                        The buildpack has attempted to queue a warning to be seen later, but
                                        no `WarnGuard`s have been constucted.

                                        Add a `let warn_later = WarnGuard::new()` to the buildpack to ensure
                                        delayed warnings are shown.
                "}
        )
    } else {
        Ok(())
    }
}

impl<W> Drop for WarnGuard<W>
where
    W: Write + Debug,
{
    fn drop(&mut self) {
        GUARD_COUNT.fetch_sub(1, Ordering::Relaxed);

        let warnings = drain();
        if !warnings.is_empty() {
            writeln!(&mut self.io).expect("warn guard IO is writeable");
            for warning in &warnings {
                writeln!(&mut self.io, "{warning}").expect("warn guard IO is writeable");
                writeln!(&mut self.io).expect("warn guard IO is writeable");
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::output::section_log::*;
    use crate::output::util::ReadYourWrite;
    use libcnb_test::assert_contains;

    #[test]
    fn test_warning_emitted_if_no_warn_guard_created() {
        let test_guard = TEST_ACCESS.lock().unwrap();

        // Warn that a guard is not yet created
        let mut io: Vec<u8> = Vec::new();
        warn_if_no_guards(&mut io).unwrap();
        assert_contains!(
            String::from_utf8_lossy(&io).to_string(),
            "[Internal warning]",
        );

        // Don't warn when a guard is created
        let warn_guard = WarnGuard::new();
        let mut io = Vec::new();
        warn_if_no_guards(&mut io).unwrap();

        let actual = String::from_utf8_lossy(&io).to_string();
        assert!(
            actual.is_empty(),
            "Expected {actual} to be empty but it was not"
        );

        drop(warn_guard);
        drop(test_guard);
    }

    #[test]
    fn test_logging_a_warning() {
        let _test_guard = TEST_ACCESS.lock().unwrap();

        // Avoid internal warning, ensure warnings are drained
        let warn_guard = WarnGuard::new();
        let expected: Vec<String> = Vec::new();
        assert_eq!(expected, drain());

        let message =
            "Possessing knowledge and performing an action are two entirely different processes";

        log_warning_later(message);

        assert_contains!(drain().into_iter().collect::<String>(), message);

        // Assert empty after calling drain
        let expected: Vec<String> = Vec::new();
        assert_eq!(expected, drain());

        drop(warn_guard);
    }

    #[test]
    fn test_delayed_warnings_on_drop() {
        let test_guard = TEST_ACCESS.lock().unwrap();
        let writer = ReadYourWrite::writer(Vec::new());
        let reader = writer.reader();
        let guard = WarnGuard::with_io(writer);

        let message = "You don't have to have a reason to be tired. You don't have to earn rest or comfort. You're allowed to just be.";
        log_warning_later(message);
        drop(guard);

        let io = reader.lock().unwrap();
        assert_contains!(String::from_utf8_lossy(&io), message);

        drop(test_guard);
    }
}

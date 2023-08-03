/// Internal library
///
/// Export user errors so they're accessible by bin/print_ruby_errors.rs

// When lib.rs is compiled by itself it doesn't know that src/main.rs exercises
// various pub(crate) interfaces and so it tags them as unused. If they're actually unused
// by main.rs we'll get errors there.
#[allow(dead_code)]
pub mod user_errors;

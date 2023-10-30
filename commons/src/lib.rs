#![warn(unused_crate_dependencies)]
#![warn(clippy::pedantic)]

// Used in both testing and printing the style guide
use indoc as _;
// Used in the style guide
use ascii_table as _;

pub mod cache;
pub mod display;
pub mod fun_run;
pub mod gem_version;
pub mod gemfile_lock;
pub mod layer;
pub mod metadata_digest;
pub mod output;

mod err;

use libcnb::data::layer::LayerName;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// As layers are created this is incremented
static LAYER_COUNT: AtomicUsize = AtomicUsize::new(1);
/// Formatting for number of leanding zeros. Max number of layers is 1024 (as of 2025). Four digits allows
/// one buildpack to meet or exceed that value
static DIGITS: usize = 4;

fn prefix(value: usize) -> String {
    format!("{value:0DIGITS$}_")
}

fn next_count() -> usize {
    LAYER_COUNT.fetch_add(1, Ordering::Acquire)
}

/// Removes the `NNNN_` prefix if there is one
pub(crate) fn strip_order_prefix(name: &str) -> String {
    let re = regex::Regex::new(&format!("^\\d{{{DIGITS}}}_"))
        .expect("internal code bugs caught by unit tests");
    re.replace(name, "").to_string()
}

/// Searches the given dir for an entry with the exact name or any NNNN_ prefix and returns Ok(Some(PathBuf))
pub(crate) fn contains_entry_with_name_or_pattern(
    dir: &Path,
    name: &str,
) -> Result<Option<PathBuf>, std::io::Error> {
    let name = strip_order_prefix(name);

    let pattern = format!("^\\d{{{DIGITS}}}_{}$", regex::escape(&name));
    let re = regex::Regex::new(&pattern).expect("internal error if this fails to compile");

    for entry in fs_err::read_dir(dir)?.flatten() {
        if let Some(file_name) = entry.file_name().to_str() {
            if file_name == name || re.is_match(file_name) {
                return Ok(Some(entry.path().clone()));
            }
        }
    }

    Ok(None)
}

/// Gets and increments the next name
///
/// # Panics
///
/// Assumes that prepending a value to a valid layer name is a valid operation
#[must_use]
pub(crate) fn ordered_layer_name(name: LayerName) -> LayerName {
    let prefix = prefix(next_count());
    prefix_layer_name(&prefix, name)
}

/// # Panics
///
/// Assumes that prepending a value to a valid layer name is a valid operation
#[must_use]
#[allow(clippy::needless_pass_by_value)]
fn prefix_layer_name(prefix: impl AsRef<str>, name: LayerName) -> LayerName {
    let prefix = prefix.as_ref();
    format!("{prefix}{}", name.as_str())
        .parse()
        .expect("Prepending to a valid layer name is valid")
}

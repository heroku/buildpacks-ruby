use crate::cache::CacheError;
use byte_unit::{AdjustedByte, Byte, UnitType};
use fs_err::PathExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Remove Least Recently Used (LRU) files in cache above a byte limit
///
/// The cache directory may grow unbounded. This function will limit
/// the size of the directory to the given input. When the directory
/// grows larger than the limit, then files will be deleted to
/// bring the directory size under the given limit.
///
/// # Errors
///
/// - The provided ``cache_path`` is not valid UTF-8 (`OsStringErr`).
/// - Metadata from a file in the ``cache_path`` cannot be retrieved from the OS (`IoError`).
///   this is needed for mtime retrieval to calculate which file is least recently used.
/// - If there's an OS error while deleting a file.
/// - If an internal glob pattern is incorrect
/// - If the OS does not support mtime operation on files.
pub(crate) fn lru_clean(path: &Path, limit: Byte) -> Result<Option<FilesWithSize>, CacheError> {
    let overage = lru_files_above_limit(path, limit)?;

    if overage.files.is_empty() {
        Ok(None)
    } else {
        for file in &overage.files {
            fs_err::remove_file(file).map_err(CacheError::IoError)?;
        }

        Ok(Some(overage))
    }
}

/// Converts all files in a directory (recursively) into a `MiniPathModSize`
/// so they can be sorted by modified date and total size calculated.
fn files(cache_path: &Path) -> Result<Vec<MiniPathModSize>, CacheError> {
    walkdir::WalkDir::new(cache_path)
        .follow_links(true)
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|p| p.path().is_file().then_some(p.path().to_path_buf()))
        .map(MiniPathModSize::new)
        .collect::<Result<Vec<MiniPathModSize>, CacheError>>()
}

/// Calculate total size of files in a directory. If that size is above the given limit, then a list
/// of files (ordered by modified time so the last touched will come first) will be returned via a
/// `FilesWithSize`. If later deleted, those values will reduce the total size of the directory
/// below the limit.
fn lru_files_above_limit(cache_path: &Path, limit: Byte) -> Result<FilesWithSize, CacheError> {
    let max_bytes = limit.as_u128();
    let mut files = files(cache_path)?;
    let bytes = files.iter().map(|p| u128::from(p.size)).sum::<u128>();

    if bytes >= max_bytes {
        let mut current_bytes = bytes;
        files.sort_by(|a, b| a.modified.cmp(&b.modified));

        Ok(FilesWithSize {
            bytes,
            files: files
                .iter()
                .take_while(|m| {
                    current_bytes -= u128::from(m.size);
                    current_bytes >= max_bytes
                })
                .map(|p| p.path.clone())
                .collect::<Vec<PathBuf>>(),
        })
    } else {
        Ok(FilesWithSize::default())
    }
}

/// A list of files and their associated size on disk in bytes
#[derive(Debug, Eq, PartialEq, Default)]
pub struct FilesWithSize {
    /// Size of files on disk
    bytes: u128,

    /// Paths to files
    pub files: Vec<PathBuf>,
}

impl FilesWithSize {
    #[must_use]
    pub fn to_byte(&self) -> Byte {
        Byte::from_u128(self.bytes).unwrap_or(Byte::MAX)
    }

    /// Return byte value with adjusted units.
    ///
    /// When formatted the units will be included.
    #[must_use]
    pub fn adjusted_bytes(&self) -> AdjustedByte {
        self.to_byte().get_appropriate_unit(UnitType::Binary)
    }
}

/// Internal helper for representing a file and it's metadata
#[derive(Debug)]
struct MiniPathModSize {
    size: u64,
    path: PathBuf,
    modified: SystemTime,
}

impl MiniPathModSize {
    fn new(path: PathBuf) -> Result<Self, CacheError> {
        let metadata = path.fs_err_metadata().map_err(CacheError::IoError)?;
        let modified = metadata
            .modified()
            .map_err(CacheError::MtimeUnsupportedOS)?;
        let size = metadata.size();

        Ok(Self {
            size,
            path,
            modified,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::cache::mib;

    use super::*;

    #[test]
    fn test_grabs_files() {
        // FWIW This action must be done on two lines as the tmpdir gets cleaned
        // as soon as the variable goes out of scope and a path
        // reference does not retain it's caller
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path();

        let out = files(dir).unwrap();
        assert!(out.is_empty());

        fs_err::write(dir.join("lol"), "hahah").unwrap();
        let out = files(dir).unwrap();

        assert_eq!(out.len(), 1);
    }

    fn touch_file(path: &PathBuf, f: impl FnOnce(&PathBuf)) {
        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            fs_err::create_dir_all(parent).unwrap();
        }
        fs_err::write(path, "").unwrap();
        f(path);
        fs_err::remove_file(path).unwrap();
    }

    #[test]
    fn test_lru_only_returns_based_on_size() {
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path().join("dir");

        fs_err::create_dir_all(&dir).unwrap();

        assert_eq!(lru_files_above_limit(&dir, mib(0),).unwrap().files.len(), 0);

        touch_file(&dir.join("a"), |file| {
            let overage = lru_files_above_limit(&dir, mib(0)).unwrap();
            assert_eq!(overage.files, vec![file.clone()]);

            let overage = lru_files_above_limit(&dir, mib(10)).unwrap();
            assert_eq!(overage.files.len(), 0);
        });
    }

    #[test]
    fn test_lru_returns_older_files_first() {
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path().join("");

        touch_file(&dir.join("z_older"), |a| {
            touch_file(&dir.join("a_newer"), |b| {
                filetime::set_file_mtime(a, filetime::FileTime::from_unix_time(0, 0)).unwrap();
                filetime::set_file_mtime(b, filetime::FileTime::from_unix_time(1, 0)).unwrap();

                let overage = lru_files_above_limit(&dir, mib(0)).unwrap();
                assert_eq!(overage.files, vec![a.clone(), b.clone()]);
            });
        });
    }

    #[test]
    fn test_lru_does_not_grab_directories() {
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path().join("");
        fs_err::create_dir_all(dir.join("preservation_society")).unwrap();
        let overage = lru_files_above_limit(&dir, mib(0)).unwrap();
        assert_eq!(overage.files, Vec::<PathBuf>::new());
    }
}

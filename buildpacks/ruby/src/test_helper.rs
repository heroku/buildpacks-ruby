use std::path::PathBuf;

pub(crate) fn touch_file(path: &PathBuf, f: impl FnOnce(&PathBuf)) {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).unwrap();
        }
    }
    std::fs::write(path, "").unwrap();
    f(path);
    std::fs::remove_file(path).unwrap();
}

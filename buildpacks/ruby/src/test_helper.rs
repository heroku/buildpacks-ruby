use crate::RubyBuildpack;
use libcnb::build::BuildContext;
use libcnb::detect::DetectContext;
use std::path::PathBuf;

pub fn touch_file(path: &PathBuf, f: impl FnOnce(&PathBuf)) {
    if let Some(parent) = path.parent() {
        println!("path {path:?}");
        println!("parent {parent:?}");
        if !parent.exists() {
            std::fs::create_dir_all(parent).unwrap();
        }
    }
    std::fs::write(path, "").unwrap();
    f(path);
    std::fs::remove_file(path).unwrap();
}

#[allow(dead_code)]
pub(crate) struct TempContext {
    pub detect: DetectContext<RubyBuildpack>,
    pub build: BuildContext<RubyBuildpack>,
    _tmp_dir: tempfile::TempDir,
}

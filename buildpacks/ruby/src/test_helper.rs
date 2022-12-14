use crate::RubyBuildpack;
use libcnb::build::BuildContext;
use libcnb::data::buildpack_plan::Entry;
use libcnb::data::stack_id;
use libcnb::data::{buildpack::SingleBuildpackDescriptor, buildpack_plan::BuildpackPlan};
use libcnb::detect::DetectContext;
use libcnb::generic::{GenericMetadata, GenericPlatform};
use libcnb::Platform;
use std::fs;
use std::path::PathBuf;

pub fn touch_file(path: &PathBuf, f: impl FnOnce(&PathBuf)) {
    if let Some(parent) = path.parent() {
        println!("path {:?}", path);
        println!("parent {:?}", parent);
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

#[allow(dead_code)]
impl TempContext {
    pub fn new(buildpack_toml_string: &str) -> Self {
        let tmp_dir = tempfile::tempdir().unwrap();
        let app_dir = tmp_dir.path().join("app");
        let layers_dir = tmp_dir.path().join("layers");
        let platform_dir = tmp_dir.path().join("platform");
        let buildpack_dir = tmp_dir.path().join("buildpack");

        for dir in [&app_dir, &layers_dir, &buildpack_dir, &platform_dir] {
            fs::create_dir_all(dir).unwrap();
        }

        let stack_id = stack_id!("heroku-22");
        let platform = GenericPlatform::from_path(&platform_dir).unwrap();
        let buildpack_descriptor: SingleBuildpackDescriptor<GenericMetadata> =
            toml::from_str(buildpack_toml_string).unwrap();

        let detect_context = DetectContext {
            platform,
            buildpack_descriptor,
            app_dir: app_dir.clone(),
            buildpack_dir: buildpack_dir.clone(),
            stack_id: stack_id.clone(),
        };

        let platform = GenericPlatform::from_path(&platform_dir).unwrap();
        let buildpack_descriptor: SingleBuildpackDescriptor<GenericMetadata> =
            toml::from_str(buildpack_toml_string).unwrap();
        let buildpack_plan = BuildpackPlan {
            entries: Vec::<Entry>::new(),
        };
        let build_context = BuildContext {
            layers_dir,
            app_dir,
            buildpack_dir,
            stack_id,
            platform,
            buildpack_plan,
            buildpack_descriptor,
        };

        TempContext {
            detect: detect_context,
            build: build_context,
            _tmp_dir: tmp_dir,
        }
    }
}

pub(crate) use commons::layer::cache_buddy::cached_layer_write_metadata;
pub(crate) use commons::layer::cache_buddy::Meta;

/// Takes in a directory and returns a minimal build context for use in testing shared caching behavior
///
/// Intented only for use with this buildpack, but meant to be used by multiple layers to assert caching behavior.
#[cfg(test)]
pub(crate) fn temp_build_context<B: libcnb::Buildpack>(
    from_dir: impl AsRef<std::path::Path>,
) -> libcnb::build::BuildContext<B> {
    let base_dir = from_dir.as_ref().to_path_buf();
    let layers_dir = base_dir.join("layers");
    let app_dir = base_dir.join("app_dir");
    let platform_dir = base_dir.join("platform_dir");
    let buildpack_dir = base_dir.join("buildpack_dir");
    for dir in [&app_dir, &layers_dir, &buildpack_dir, &platform_dir] {
        std::fs::create_dir_all(dir).unwrap();
    }

    let target = libcnb::Target {
        os: String::new(),
        arch: String::new(),
        arch_variant: None,
        distro_name: String::new(),
        distro_version: String::new(),
    };
    let buildpack_toml_string = include_str!("../../buildpack.toml");
    let platform =
        <<B as libcnb::Buildpack>::Platform as libcnb::Platform>::from_path(&platform_dir).unwrap();
    let buildpack_descriptor: libcnb::data::buildpack::ComponentBuildpackDescriptor<
        <B as libcnb::Buildpack>::Metadata,
    > = toml::from_str(buildpack_toml_string).unwrap();
    let buildpack_plan = libcnb::data::buildpack_plan::BuildpackPlan {
        entries: Vec::<libcnb::data::buildpack_plan::Entry>::new(),
    };
    let store = None;

    libcnb::build::BuildContext {
        layers_dir,
        app_dir,
        buildpack_dir,
        target,
        platform,
        buildpack_plan,
        buildpack_descriptor,
        store,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RubyBuildpack;
    use cache_diff::CacheDiff;
    use commons::layer::cache_buddy::{invalid_metadata_action, restored_layer_action};
    use core::panic;
    use libcnb::data::layer_name;
    use libcnb::layer::{EmptyLayerCause, InvalidMetadataAction, LayerState, RestoredLayerAction};
    use magic_migrate::{migrate_toml_chain, try_migrate_deserializer_chain, Migrate, TryMigrate};
    use serde::Deserializer;
    use std::convert::Infallible;

    /// Struct for asserting the behavior of `cached_layer_write_metadata`
    #[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
    #[serde(deny_unknown_fields)]
    struct TestMetadata {
        value: String,
    }
    impl CacheDiff for TestMetadata {
        fn diff(&self, old: &Self) -> Vec<String> {
            if self.value == old.value {
                vec![]
            } else {
                vec![format!("value ({} to {})", old.value, self.value)]
            }
        }
    }
    migrate_toml_chain! {TestMetadata}
}

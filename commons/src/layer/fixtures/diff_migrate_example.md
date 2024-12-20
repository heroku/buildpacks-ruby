# Example

```
use commons::layer::diff_migrate::{DiffMigrateLayer, Meta};
use cache_diff::CacheDiff;
use magic_migrate::TryMigrate;

use libcnb::layer::{LayerState, EmptyLayerCause};
use libcnb::data::layer_name;

# #[derive(Debug, serde::Serialize, serde::Deserialize, Clone, cache_diff::CacheDiff)]
# #[serde(deny_unknown_fields)]
# struct TestMetadata {
#     value: String,
# }
# use magic_migrate::Migrate;
# magic_migrate::migrate_toml_chain!(TestMetadata);
#
# struct FakeBuildpack;
#
# impl libcnb::Buildpack for FakeBuildpack {
#     type Platform = libcnb::generic::GenericPlatform;
#     type Metadata = libcnb::generic::GenericMetadata;
#     type Error = std::convert::Infallible;
#
#     fn detect(
#         &self,
#         _context: libcnb::detect::DetectContext<Self>,
#     ) -> libcnb::Result<libcnb::detect::DetectResult, Self::Error> {
#         todo!()
#     }
#
#     fn build(
#         &self,
#         _context: libcnb::build::BuildContext<Self>,
#     ) -> libcnb::Result<libcnb::build::BuildResult, Self::Error> {
#         todo!()
#     }
# }
# fn install_ruby(path: &std::path::Path) {
#   todo!();
# }
#
# pub(crate) fn call(
#     context: &libcnb::build::BuildContext<FakeBuildpack>,
# ) -> libcnb::Result<(), <FakeBuildpack as libcnb::Buildpack>::Error> {
# let metadata_owned = TestMetadata { value: "Hello".to_string() };
# let metadata = &metadata_owned;
let layer_ref = DiffMigrateLayer {
    build: true,
    launch: true,
}
.cached_layer(layer_name!("ruby"), context, metadata)?;
match &layer_ref.state {
    // CacheDiff reported no difference, cache was kept
    LayerState::Restored { cause } => {
        println!("  - {cause}"); // States that the cache is being used
        match cause {
            Meta::Data(old) => {
                // Inspect or use the old metadata from cache here if you like!
                assert!(metadata.diff(old).is_empty());
            },
            Meta::Message(_) => unreachable!("Will only ever contain metadata when the cache is retained")
        }
    }
    LayerState::Empty { cause } => {
        match cause {
            // Nothing in old cache
            EmptyLayerCause::NewlyCreated => {}
            // Problem restoring old cache (`TryMigrate` could not migrate the old metadata)
            EmptyLayerCause::InvalidMetadataAction { cause }
            // Difference with old cache
            | EmptyLayerCause::RestoredLayerAction { cause } => {
                // Report why the cache was cleared
                println!("  - {cause}");
            }
        }
        println!("  - Installing");
        install_ruby(&layer_ref.path());
        println!("  - Done");
    }
}
# Ok(())
# }
```

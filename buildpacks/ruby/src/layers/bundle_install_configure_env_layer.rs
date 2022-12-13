use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::generic::GenericMetadata;
use libcnb::layer::{Layer, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use std::path::Path;

/// # Set up environment for `bundle install`
///
/// Must run before `execute_bundle_install_layer`. This is decoupled from dependency
/// storage so that it can be updated without having to reinstall gems.
///
/// Path specific bundler environment variables are set in [`BundleInstallCreatePathLayer`]
pub struct BundleInstallConfigureEnvLayer;

impl Layer for BundleInstallConfigureEnvLayer {
    type Buildpack = RubyBuildpack;
    type Metadata = GenericMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: true,
            launch: true,
            cache: false,
        }
    }

    fn create(
        &self,
        context: &BuildContext<Self::Buildpack>,
        _layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        LayerResultBuilder::new(GenericMetadata::default())
            .env(
                LayerEnv::new()
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Delimiter,
                        "BUNDLE_WITHOUT",
                        ":",
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Prepend,
                        "BUNDLE_WITHOUT", // Do not install `development` or `test` groups via bundle install. Additional groups can be specified via user config.
                        "development:test",
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Override,
                        "BUNDLE_GEMFILE", // Tells bundler where to find the `Gemfile`
                        context.app_dir.join("Gemfile"),
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Override,
                        "BUNDLE_CLEAN", // After successful `bundle install` bundler will automatically run `bundle clean`
                        "1",
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Override,
                        "BUNDLE_DEPLOYMENT", // Requires the `Gemfile.lock` to be in sync with the current `Gemfile`.
                        "1",
                    ),
            )
            .build()
    }
}

use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use libcnb::build::BuildContext;
use libcnb::data::buildpack::StackId;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// # Set up path for `bundle install` dependencies
///
/// Must run before `execute_bundle_install_layer` to create directory for dependencies.
///
/// ## Layer dir: Create directory for dependencies
///
/// Dependencies installed via `bundle install` will be stored in this layer's directory.
/// This is accomplished via configuring bundler via environment variables
///
/// Other environment variables for bundler are configured by another layer that is decoupled
/// from dependency storage on disk to miminimize the risk of having to clear dependencies
/// to update an environment variable. [`BundleInstallConfigureEnvLayer`]
pub(crate) struct BundlePathLayer {
    pub ruby_version: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct BundlePathLayerMetadata {
    ruby_version: String,
    stack: StackId,
}

impl Layer for BundlePathLayer {
    type Buildpack = RubyBuildpack;
    type Metadata = BundlePathLayerMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: true,
            launch: true,
            cache: true,
        }
    }

    fn create(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        LayerResultBuilder::new(BundlePathLayerMetadata {
            ruby_version: self.ruby_version.clone(),
            stack: context.stack_id.clone(),
        })
        .env(
            LayerEnv::new()
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Override,
                    "BUNDLE_PATH", // Directs bundler to install gems to this path.
                    layer_path,
                )
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Override,
                    "BUNDLE_BIN", // Install executables for all gems into specified path.
                    &layer_path.join("bin"),
                )
                .chainable_insert(Scope::All, ModificationBehavior::Delimiter, "GEM_PATH", ":")
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Append,
                    "GEM_PATH", // Tells Ruby where gems are located. Should match `BUNDLE_PATH`.
                    layer_path,
                ),
        )
        .build()
    }

    fn existing_layer_strategy(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        if context.stack_id == layer_data.content_metadata.metadata.stack {
            if self.ruby_version == layer_data.content_metadata.metadata.ruby_version {
                println!("---> Loading previously installed gems from cache");
                Ok(ExistingLayerStrategy::Keep)
            } else {
                println!("---> Ruby version changed, clearing gems");
                Ok(ExistingLayerStrategy::Recreate)
            }
        } else {
            println!("---> Stack has changed, clearing installed gems");
            Ok(ExistingLayerStrategy::Recreate)
        }
    }
}

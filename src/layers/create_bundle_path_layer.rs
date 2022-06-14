use crate::RubyBuildpackError;
use libcnb::data::layer_content_metadata::LayerTypes;
use std::path::Path;

use crate::RubyBuildpack;
use libcnb::build::BuildContext;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};

use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use serde::{Deserialize, Serialize};

pub struct CreateBundlePathLayer {
    pub ruby_version: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CreateBundlePathMetadata {
    ruby_version: String,
}

// Creates bundle path layer. The intent is
// for this layer to be used later via `bundle install`
//
// - BUNDLE_PATH
// - GEM_PATH
// - BUNDLE_BIN
impl Layer for CreateBundlePathLayer {
    type Buildpack = RubyBuildpack;
    type Metadata = CreateBundlePathMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: true,
            launch: true,
            cache: true,
        }
    }

    fn create(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        LayerResultBuilder::new(CreateBundlePathMetadata {
            ruby_version: self.ruby_version.clone(),
        })
        .env(
            LayerEnv::new()
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Override,
                    "BUNDLE_PATH",
                    &layer_path,
                )
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Override,
                    "GEM_PATH",
                    &layer_path,
                )
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Override,
                    "BUNDLE_BIN",
                    &layer_path.join("bin"),
                ),
        )
        .build()
    }

    fn existing_layer_strategy(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        if self.ruby_version == layer_data.content_metadata.metadata.ruby_version {
            Ok(ExistingLayerStrategy::Keep)
        } else {
            println!("---> Ruby version changed, clearing gems");
            Ok(ExistingLayerStrategy::Recreate)
        }
    }
}

use crate::RubyBuildpackError;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::generic::GenericMetadata;

use std::path::Path;

use crate::RubyBuildpack;
use libcnb::build::BuildContext;
use libcnb::Env;

use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};

pub struct RakeAssetsCreateCacheLayer {
    pub env: Env,
}

impl Layer for RakeAssetsCreateCacheLayer {
    type Buildpack = RubyBuildpack;
    type Metadata = GenericMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: false,
            launch: false,
            cache: true,
        }
    }

    // Slice layers-
    fn create(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        _layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        LayerResultBuilder::new(GenericMetadata::default()).build()
    }

    fn existing_layer_strategy(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        _layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        Ok(ExistingLayerStrategy::Keep)
    }
}

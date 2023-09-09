use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::generic::GenericMetadata;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::layer_env::LayerEnv;
use std::marker::PhantomData;
use std::path::Path;

/// A struct with one purpose: Retrieve prior `LayerData` from the last build (if there is any)
#[derive(Debug)]
pub struct GetLayerData<B> {
    buildpack: PhantomData<B>,
    layer_types: LayerTypes,
}

impl<B> GetLayerData<B> {
    #[must_use]
    pub fn new(layer_types: LayerTypes) -> Self {
        Self {
            buildpack: PhantomData,
            layer_types,
        }
    }
}

impl<B> Layer for GetLayerData<B>
where
    B: libcnb::Buildpack,
{
    type Buildpack = B;
    type Metadata = GenericMetadata;

    /// An unfortunate byproduct of this interface is that we have to write layer types when we read
    /// cached layer data.
    fn types(&self) -> LayerTypes {
        LayerTypes {
            launch: self.layer_types.launch,
            build: self.layer_types.build,
            cache: self.layer_types.cache,
        }
    }

    fn create(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        _layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, <Self::Buildpack as libcnb::Buildpack>::Error> {
        LayerResultBuilder::new(GenericMetadata::default())
            .env(LayerEnv::new())
            .build()
    }

    fn existing_layer_strategy(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        _layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, <Self::Buildpack as libcnb::Buildpack>::Error> {
        Ok(ExistingLayerStrategy::Keep)
    }

    fn migrate_incompatible_metadata(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        _metadata: &libcnb::generic::GenericMetadata,
    ) -> Result<
        libcnb::layer::MetadataMigration<Self::Metadata>,
        <Self::Buildpack as libcnb::Buildpack>::Error,
    > {
        eprint!("Warning: Clearing cache (Could not seriailize metadata from cache)");
        Ok(libcnb::layer::MetadataMigration::RecreateLayer)
    }
}

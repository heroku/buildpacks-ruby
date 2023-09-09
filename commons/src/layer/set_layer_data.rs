use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::generic::GenericMetadata;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult};
use libcnb::Buildpack;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::Path;

// Does everything but modifies the disk
pub struct SetLayerData<B, M> {
    buildpack: std::marker::PhantomData<B>,
    layer_types: LayerTypes,
    layer_result: LayerResult<M>,
}

impl<B, M> SetLayerData<B, M> {
    pub fn new(layer_types: LayerTypes, layer_result: LayerResult<M>) -> Self {
        Self {
            buildpack: std::marker::PhantomData,
            layer_types,
            layer_result,
        }
    }
}

impl<B, M> Layer for SetLayerData<B, M>
where
    B: Buildpack,
    M: Serialize + DeserializeOwned + Clone,
{
    type Buildpack = B;
    type Metadata = M;

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
    ) -> Result<LayerResult<Self::Metadata>, <Self::Buildpack as Buildpack>::Error> {
        let metadata = self.layer_result.metadata.clone();
        let env = self.layer_result.env.clone();
        let exec_d_programs = self.layer_result.exec_d_programs.clone();
        let sboms = self.layer_result.sboms.clone();

        Ok(LayerResult {
            metadata,
            env,
            exec_d_programs,
            sboms,
        })
    }

    fn update(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        _layer_data: &LayerData<Self::Metadata>,
    ) -> Result<LayerResult<Self::Metadata>, <Self::Buildpack as Buildpack>::Error> {
        let metadata = self.layer_result.metadata.clone();
        let env = self.layer_result.env.clone();
        let exec_d_programs = self.layer_result.exec_d_programs.clone();
        let sboms = self.layer_result.sboms.clone();

        Ok(LayerResult {
            metadata,
            env,
            exec_d_programs,
            sboms,
        })
    }

    fn migrate_incompatible_metadata(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        _metadata: &GenericMetadata,
    ) -> Result<
        libcnb::layer::MetadataMigration<Self::Metadata>,
        <Self::Buildpack as Buildpack>::Error,
    > {
        Ok(libcnb::layer::MetadataMigration::ReplaceMetadata(
            self.layer_result.metadata.clone(),
        ))
    }

    fn existing_layer_strategy(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        _layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, <Self::Buildpack as Buildpack>::Error> {
        Ok(ExistingLayerStrategy::Keep)
    }
}

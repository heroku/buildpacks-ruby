use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::Buildpack;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::path::Path;
use std::path::PathBuf;

/// # Caches a folder in the application directory
///
/// Layers are used for caching, however layers cannot be inside of the app directory.
/// This layer can be used to hold a directory's contents so they are preserved
/// between deploys.
///
/// The primary usecase of this is for caching assets. After `rake assets:precompile` runs
/// file in `<app-dir>/public/assets` need to be preserved between deploys. This allows
/// for faster deploys, and also allows for prior generated assets to remain on the system
///  until "cleaned."
///
///  Historically, sprockets will keep 3 versions of old files on disk. This
///  allows for emails, that might live a long time, to reference a specific SHA of an
///  asset.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct InAppDirCacheLayer<B> {
    pub(crate) app_dir_path: PathBuf,
    buildpack: PhantomData<B>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct InAppDirCacheLayerMetadata {
    app_dir_path: PathBuf,
}

impl<B> InAppDirCacheLayer<B> {
    pub(crate) fn new(app_dir_path: PathBuf) -> Self {
        Self {
            app_dir_path,
            buildpack: PhantomData,
        }
    }
}

impl<B> Layer for InAppDirCacheLayer<B>
where
    B: Buildpack,
{
    type Buildpack = B;
    type Metadata = InAppDirCacheLayerMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: true,
            launch: true,
            cache: true,
        }
    }

    fn create(
        &mut self,
        _context: &BuildContext<Self::Buildpack>,
        _layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, B::Error> {
        LayerResultBuilder::new(InAppDirCacheLayerMetadata {
            app_dir_path: self.app_dir_path.clone(),
        })
        .build()
    }

    fn existing_layer_strategy(
        &mut self,
        _context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, B::Error> {
        if self.app_dir_path == layer_data.content_metadata.metadata.app_dir_path {
            Ok(ExistingLayerStrategy::Keep)
        } else {
            Ok(ExistingLayerStrategy::Recreate)
        }
    }
}

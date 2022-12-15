use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub(crate) struct EnvSecretKeyBaseLayer;

/// # Set the `SECRET_KEY_BASE` environment variable
///
/// This environment variable is primarily used for encrypting/decrypting user sessions.
/// Developers should not need to set it themselves, however if they want,
/// they can over-write the default.
///
/// Must run before any `rake` or `rails` commands are executed as the application may
/// error without this environment variable.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct EnvSecretKeyBaseLayerMetadata {
    default_value: String,
}

impl Layer for EnvSecretKeyBaseLayer {
    type Buildpack = RubyBuildpack;
    type Metadata = EnvSecretKeyBaseLayerMetadata;

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
        _layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        let mut rng = rand::thread_rng();
        let secret_key_base_default: String = (0..64)
            .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
            .collect();

        LayerResultBuilder::new(Self::Metadata {
            default_value: secret_key_base_default.clone(),
        })
        .env(LayerEnv::new().chainable_insert(
            Scope::All,
            ModificationBehavior::Default,
            "SECRET_KEY_BASE",
            secret_key_base_default,
        ))
        .build()
    }

    fn existing_layer_strategy(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        _layer: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        Ok(ExistingLayerStrategy::Keep)
    }
}

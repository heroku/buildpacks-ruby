use crate::RubyBuildpackError;
use libcnb::data::layer_content_metadata::LayerTypes;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::RubyBuildpack;
use libcnb::build::BuildContext;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};

use rand::Rng;

pub struct EnvDefaultsSetSecretKeyBaseLayer;

///
/// # Set the SECRET_KEY_BASE environment variable
///
/// This environment variable is primarially used for
/// encrypting/decrypting user sessions. Developers
/// should not need to set it themselves, however if they
/// want to they can over-write the default.
///
/// Must run before any `rake` or `rails` commands are executed
/// as the application may error without this environment variable.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct EnvDefaultsSetSecretKeyBaseLayerMetadata {
    default_value: String,
}

impl Layer for EnvDefaultsSetSecretKeyBaseLayer {
    type Buildpack = RubyBuildpack;
    type Metadata = EnvDefaultsSetSecretKeyBaseLayerMetadata;

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

use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::generic::GenericMetadata;
use libcnb::layer::{Layer, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use std::path::Path;

pub(crate) struct EnvSecretKeyBaseLayer {
    pub default_value: String,
}

/// # Set the `SECRET_KEY_BASE` environment variable
///
/// This environment variable is primarily used for encrypting/decrypting user sessions.
/// Developers should not need to set it themselves, however if they want,
/// they can over-write the default.
///
/// Must run before any `rake` or `rails` commands are executed as the application may
/// error without this environment variable.
impl Layer for EnvSecretKeyBaseLayer {
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
        _context: &BuildContext<Self::Buildpack>,
        _layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        LayerResultBuilder::new(Self::Metadata::default())
            .env(LayerEnv::new().chainable_insert(
                Scope::All,
                ModificationBehavior::Default,
                "SECRET_KEY_BASE",
                self.default_value.clone(),
            ))
            .build()
    }
}

use crate::RubyBuildpackError;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::generic::GenericMetadata;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::RubyBuildpack;
use libcnb::build::BuildContext;
use libcnb::layer::{Layer, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};

pub struct EnvDefaultsSetStaticVarsLayer;

///
/// # Set application environment variables
///
/// Sets default environment variables for applications.
/// Such as `RAILS_ENV=${RAILS_ENV:-production}`.
///
/// This must be done prior to running `bundle install` as
/// some apps use dynamic code inside of their Gemfile and will
/// expect certain env vars to already be set.
///
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct EnvDefaultsSetStaticVarsLayerMetadata {
    default_value: String,
}

impl Layer for EnvDefaultsSetStaticVarsLayer {
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
        LayerResultBuilder::new(GenericMetadata::default())
            .env(
                LayerEnv::new() //
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Default,
                        "JRUBY_OPTS",
                        "-Xcompile.invokedynamic=false",
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Default,
                        "RACK_ENV",
                        "production",
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Default,
                        "RAILS_ENV",
                        "production",
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Default,
                        "RAILS_SERVE_STATIC_FILES",
                        "enabled",
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Default,
                        "RAILS_LOG_TO_STDOUT",
                        "enabled",
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Default,
                        "MALLOC_ARENA_MAX",
                        "2",
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Default,
                        "DISABLE_SPRING",
                        "1",
                    ),
            )
            .build()
    }
}

use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::generic::GenericMetadata;
use libcnb::layer::{Layer, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub(crate) struct EnvDefaultsLayer;

///
/// # Set application environment variables
///
/// Sets default environment variables for applications such as `RAILS_ENV=${RAILS_ENV:-production}`.
///
/// This must be done prior to running `bundle install` as some apps use dynamic code inside of their Gemfile and will
/// expect certain env vars to already be set.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct EnvDefaultsLayerMetadata {
    default_value: String,
}

impl Layer for EnvDefaultsLayer {
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
                        "JRUBY_OPTS", // Environment variable for jruby apps, does not affect MRI
                        "-Xcompile.invokedynamic=false",
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Default,
                        "RACK_ENV",   // Sets the Rack environment.
                        "production", // This value is cargo-culted, some expect it now
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Default,
                        "RAILS_ENV", // Sets the Rails environment
                        "production",
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Default,
                        "RAILS_SERVE_STATIC_FILES", // Rails 5+ serve files in `public/` dir via `ActionDispatch::Static` middleware
                        "enabled",
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Default,
                        "RAILS_LOG_TO_STDOUT", // Rails 5+ logging to STDOUT instead of a file
                        "enabled",
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Default,
                        "MALLOC_ARENA_MAX", // Reduce maximum memory use, slightly reduces allocation performance with many threads
                        "2",
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Default,
                        "DISABLE_SPRING", // Disable problematic process caching library
                        "1",
                    ),
            )
            .build()
    }
}

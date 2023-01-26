use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use libcnb::build::BuildContext;
use libcnb::data::buildpack::StackId;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libherokubuildpack::log as user;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// # Set up path for `bundle install` dependencies
///
/// ## Layer dir: Create directory for dependencies
///
/// Dependencies installed via `bundle install` will be stored in this layer's directory.
/// This is accomplished via configuring bundler via environment variables
///
/// Other environment variables for bundler are configured by another layer that is decoupled
/// from dependency storage on disk to minimize the risk of having to clear dependencies
/// to update an environment variable.
pub(crate) struct GemsPathLayer {
    pub ruby_version: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct GemsPathLayerMetadata {
    stack: StackId,
    ruby_version: String,
}

impl Layer for GemsPathLayer {
    type Buildpack = RubyBuildpack;
    type Metadata = GemsPathLayerMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: true,
            launch: true,
            cache: true,
        }
    }

    fn create(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        user::log_info("Creating gems cache");

        LayerResultBuilder::new(GemsPathLayerMetadata {
            ruby_version: self.ruby_version.clone(),
            stack: context.stack_id.clone(),
        })
        .env(
            LayerEnv::new()
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Override,
                    "BUNDLE_PATH", // Directs bundler to install gems to this path.
                    layer_path,
                )
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Override,
                    "BUNDLE_BIN", // Install executables for all gems into specified path.
                    layer_path.join("bin"),
                )
                .chainable_insert(Scope::All, ModificationBehavior::Delimiter, "GEM_PATH", ":")
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Append,
                    "GEM_PATH", // Tells Ruby where gems are located. Should match `BUNDLE_PATH`.
                    layer_path,
                ),
        )
        .build()
    }

    fn existing_layer_strategy(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        let contents = CacheContents {
            old_stack: &layer_data.content_metadata.metadata.stack,
            old_version: &layer_data.content_metadata.metadata.ruby_version,
            current_stack: &context.stack_id,
            current_version: self.ruby_version.as_str(),
        };

        match contents.state() {
            Changed::Stack => {
                user::log_info("Clearing gems cache, stack has changed");

                Ok(ExistingLayerStrategy::Recreate)
            }
            Changed::RubyVersion => {
                user::log_info("Clearing gems cache, ruby version changed");

                Ok(ExistingLayerStrategy::Recreate)
            }
            Changed::Nothing => {
                user::log_info("Using gems cache");

                Ok(ExistingLayerStrategy::Keep)
            }
        }
    }
}

enum Changed {
    Stack,
    Nothing,
    RubyVersion,
}

struct CacheContents<'a, 'b, 'c, 'd> {
    old_stack: &'a StackId,
    old_version: &'b str,
    current_stack: &'c StackId,
    current_version: &'d str,
}

impl CacheContents<'_, '_, '_, '_> {
    fn state(&self) -> Changed {
        if self.current_stack != self.old_stack {
            Changed::Stack
        } else if self.current_version != self.old_version {
            Changed::RubyVersion
        } else {
            Changed::Nothing
        }
    }
}

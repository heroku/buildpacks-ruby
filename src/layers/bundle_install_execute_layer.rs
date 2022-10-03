use crate::env_command::EnvCommand;
use crate::RubyBuildpackError;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::generic::GenericMetadata;

use std::path::Path;

use crate::RubyBuildpack;
use libcnb::build::BuildContext;
use libcnb::layer::{Layer, LayerResult, LayerResultBuilder};
use libcnb::Env;

/*

# Installs gems via `bundle install

Depends on:

- `create_bundle_path_layer` for setting `BUNDLE_*` environment variables and creating a
layer dir for the installed gems
- `download_bundler_layer` for installing the bundler gem and putting it on the PATH

## Layer dir

Not used. This layer installs dependencies to a layer dir created in `create_bundle_path`

## Set environment variables

None, this layer consumes environment variables set by other layers.

## Invalidation logic

This layer depends on being able to run `bundle install` idempotently. If nothing changes
then running `bundle install` has no effect. When gems are changed then the `BUNDLE_CLEAN=1`
setting will trigger bundler to clean up any unused gems off of disk. Essentially bundler
handles its own cache invalidation.

The only time the buildpack needs to clear installed gems is when a version of Ruby changes.
This invalidation is handled via the `create_bundle_path_layer` which clears it's layer contents
when a Ruby version change is detected.

*/

pub struct BundleInstallExecuteLayer {
    pub env: Env,
}

impl Layer for BundleInstallExecuteLayer {
    type Buildpack = RubyBuildpack;
    type Metadata = GenericMetadata;

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
        println!("---> Installing gems");
        let mut command = EnvCommand::new("bundle", &["install"], &self.env);
        command.show_env_keys([
            "BUNDLE_BIN",
            "BUNDLE_CLEAN",
            "BUNDLE_DEPLOYMENT",
            "BUNDLE_GEMFILE",
            "BUNDLE_PATH",
            "BUNDLE_WITHOUT",
        ]);

        println!("Running: $ {} ", command);

        command
            .stream()
            .map_err(RubyBuildpackError::BundleInstallCommandError)?;

        LayerResultBuilder::new(GenericMetadata::default()).build()
    }
}
use crate::RubyBuildpackError;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::generic::GenericMetadata;
use std::path::Path;

use crate::RubyBuildpack;
use libcnb::build::BuildContext;
use libcnb::layer::{Layer, LayerResult, LayerResultBuilder};

use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};

/*

# Set up environment for `bundle install`

Must run before `execute_bundle_install_layer`. This is decoupled from dependency
storage so that it can be updated without having to reinstall gems.

## Layer dir:

None

## Set environment variables

- `BUNDLE_CLEAN=1` - After successful `bundle install` bundler will automatically run `bundle clean`.
- `BUNDLE_DEPLOYMENT=1` - Requires the `Gemfile.lock` to be in sync with the current `Gemfile`.
- `BUNDLE_GEMFILE=<app-dir>/Gemfile` - Tells bundler where to find the `Gemfile`.
- `BUNDLE_WITHOUT=development:test:$BUNDLE_WITHOUT` - Do not install `development` or `test` groups via bundle isntall. Additional groups can be specified via user config.
- `NOKOGIRI_USE_SYSTEM_LIBRARIES=1` - Tells `nokogiri` to use the system packages, mostly `openssl`, which Heroku maintains and patches as part of its [stack](https://devcenter.heroku.com/articles/stack-packages). This setting means when a patched version is rolled out on Heroku your application will pick up the new version with no update required to libraries.

## Environment NOT set by this buildpack

The following bundler environment variables are set by another layer:

- `BUNDLE_BIN=<layer-dir>/bin` - Install executables for all gems into specified path.
- `BUNDLE_PATH=<layer-dir>` - Directs bundler to install gems to this path
- `GEM_PATH=<layer-dir>` - Tells Ruby where gems are located. Should match BUNDLE_PATH.

*/
pub struct BundleInstallConfigureEnvLayer;

impl Layer for BundleInstallConfigureEnvLayer {
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
        context: &BuildContext<Self::Buildpack>,
        _layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        LayerResultBuilder::new(GenericMetadata::default())
            .env(
                LayerEnv::new()
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Delimiter,
                        "BUNDLE_WITHOUT",
                        ":",
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Prepend,
                        "BUNDLE_WITHOUT",
                        "development:test",
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Override,
                        "BUNDLE_GEMFILE",
                        context.app_dir.join("Gemfile"),
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Override,
                        "BUNDLE_CLEAN",
                        "1",
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Override,
                        "BUNDLE_DEPLOYMENT",
                        "1",
                    )
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Override,
                        "NOKOGIRI_USE_SYSTEM_LIBRARIES",
                        "1",
                    ),
            )
            .build()
    }
}

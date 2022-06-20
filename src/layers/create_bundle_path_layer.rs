use crate::RubyBuildpackError;
use libcnb::data::layer_content_metadata::LayerTypes;
use std::path::Path;

use crate::RubyBuildpack;
use libcnb::build::BuildContext;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};

use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use serde::{Deserialize, Serialize};

/*

# Set up environment for `bundle install`

Must run before `execute_bundle_install_layer`.

## Layer dir: Create directory for dependencies

Dependencies installed via `bundle install` will be stored in this layer's directory.
This is accomplished via configuring bundler via environment variables

## Set environment variables

- `BUNDLE_BIN=<layer-dir>/bin` - Install executables for all gems into specified path.
- `BUNDLE_CLEAN=1` - After successful `bundle install` bundler will automatically run `bundle clean`.
- `BUNDLE_DEPLOYMENT=1` - Requires the `Gemfile.lock` to be in sync with the current `Gemfile`.
- `BUNDLE_GEMFILE=<app-dir>/Gemfile` - Tells bundler where to find the `Gemfile`.
- `BUNDLE_GLOBAL_PATH_APPENDS_RUBY_SCOPE=1` - Append the Ruby engine and ABI version to path. This makes the path's less "surprising".
- `BUNDLE_PATH=<layer-dir>` - Directs bundler to install gems to this path
- `BUNDLE_WITHOUT=development:test:$BUNDLE_WITHOUT` - Do not install `development` or `test` groups via bundle isntall. Additional groups can be specified via user config.
- `GEM_PATH=<layer-dir` - Tells Ruby where gems are located. Should match BUNDLE_PATH.
- `NOKOGIRI_USE_SYSTEM_LIBRARIES=1` - Tells `nokogiri` to use the system packages, mostly `openssl`, which Heroku maintains and patches as part of its [stack](https://devcenter.heroku.com/articles/stack-packages). This setting means when a patched version is rolled out on Heroku your application will pick up the new version with no update required to libraries.

*/
pub struct CreateBundlePathLayer {
    pub ruby_version: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CreateBundlePathMetadata {
    ruby_version: String,
}

impl Layer for CreateBundlePathLayer {
    type Buildpack = RubyBuildpack;
    type Metadata = CreateBundlePathMetadata;

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
        LayerResultBuilder::new(CreateBundlePathMetadata {
            ruby_version: self.ruby_version.clone(),
        })
        .env(
            LayerEnv::new()
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Override,
                    "BUNDLE_PATH",
                    &layer_path,
                )
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Override,
                    "GEM_PATH",
                    &layer_path,
                )
                .chainable_insert(
                    Scope::All,
                    ModificationBehavior::Override,
                    "BUNDLE_BIN",
                    &layer_path.join("bin"),
                )
                .chainable_insert(
                    Scope::Build,
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
                    Scope::Build,
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
                    "BUNDLE_GLOBAL_PATH_APPENDS_RUBY_SCOPE",
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

    fn existing_layer_strategy(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        if self.ruby_version == layer_data.content_metadata.metadata.ruby_version {
            Ok(ExistingLayerStrategy::Keep)
        } else {
            println!("---> Ruby version changed, clearing gems");
            Ok(ExistingLayerStrategy::Recreate)
        }
    }
}

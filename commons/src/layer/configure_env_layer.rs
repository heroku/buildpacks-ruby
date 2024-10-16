use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::generic::GenericMetadata;
#[allow(deprecated)]
use libcnb::layer::{Layer, LayerResult, LayerResultBuilder};
use libcnb::layer_env::LayerEnv;
use std::marker::PhantomData;
use std::path::Path;

/// Set environment variables
///
/// If you want to set many default environment variables you can use
/// `DefaultEnvLayer`. If you need to set different types of environment
/// variables you can use this struct `ConfigureEnvLayer`
///
/// Example:
///
/// ```rust
///# use libcnb::build::{BuildContext, BuildResult, BuildResultBuilder};
///# use libcnb::data::launch::{LaunchBuilder, ProcessBuilder};
///# use libcnb::data::process_type;
///# use libcnb::detect::{DetectContext, DetectResult, DetectResultBuilder};
///# use libcnb::generic::{GenericError, GenericMetadata, GenericPlatform};
///# use libcnb::{buildpack_main, Buildpack};
///# use libcnb::data::layer::LayerName;
///
///# pub(crate) struct HelloWorldBuildpack;
///
/// use libcnb::Env;
/// use libcnb::data::layer_name;
/// use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
/// use commons::layer::ConfigureEnvLayer;
///
///# impl Buildpack for HelloWorldBuildpack {
///#     type Platform = GenericPlatform;
///#     type Metadata = GenericMetadata;
///#     type Error = GenericError;
///
///#     fn detect(&self, _context: DetectContext<Self>) -> libcnb::Result<DetectResult, Self::Error> {
///#         todo!()
///#     }
///
///#     fn build(&self, context: BuildContext<Self>) -> libcnb::Result<BuildResult, Self::Error> {
///         let env = Env::from_current();
///         // Don't forget to apply context.platform.env() too;
///
///         let layer = context //
///             .handle_layer(
///                 layer_name!("configure_env"),
///                 ConfigureEnvLayer::new(
///                     LayerEnv::new()
///                         .chainable_insert(
///                             Scope::All,
///                             ModificationBehavior::Override,
///                             "BUNDLE_GEMFILE", // Tells bundler where to find the `Gemfile`
///                             context.app_dir.join("Gemfile"),
///                         )
///                         .chainable_insert(
///                             Scope::All,
///                             ModificationBehavior::Override,
///                             "BUNDLE_CLEAN", // After successful `bundle install` bundler will automatically run `bundle clean`
///                             "1",
///                         )
///                         .chainable_insert(
///                             Scope::All,
///                             ModificationBehavior::Override,
///                             "BUNDLE_DEPLOYMENT", // Requires the `Gemfile.lock` to be in sync with the current `Gemfile`.
///                             "1",
///                         )
///                         .chainable_insert(
///                             Scope::All,
///                             ModificationBehavior::Default,
///                             "MY_ENV_VAR",
///                             "Whatever I want"
///                         )
///                 ),
///             )?;
///         let env = layer.env.apply(Scope::Build, &env);
///
///#        todo!()
///#     }
///# }
///
/// ```
pub struct ConfigureEnvLayer<B: libcnb::Buildpack> {
    pub(crate) data: LayerEnv,
    pub(crate) _buildpack: std::marker::PhantomData<B>,
}

impl<B> ConfigureEnvLayer<B>
where
    B: libcnb::Buildpack,
{
    #[must_use]
    pub fn new(env: LayerEnv) -> Self {
        ConfigureEnvLayer {
            data: env,
            _buildpack: PhantomData,
        }
    }
}

#[allow(deprecated)]
impl<B> Layer for ConfigureEnvLayer<B>
where
    B: libcnb::Buildpack,
{
    type Buildpack = B;
    type Metadata = GenericMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: true,
            launch: true,
            cache: false,
        }
    }

    fn create(
        &mut self,
        _context: &BuildContext<Self::Buildpack>,
        _layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, B::Error> {
        LayerResultBuilder::new(GenericMetadata::default())
            .env(self.data.clone())
            .build()
    }
}

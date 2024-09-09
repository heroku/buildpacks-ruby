use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use std::ffi::OsString;
use std::marker::PhantomData;

#[allow(deprecated)]
use super::ConfigureEnvLayer;

/// Set default environment variables
///
/// If all you need to do is set default environment values, you can use
/// the `DefaultEnvLayer::new` function to set those values without having
/// to create a struct from scratch.
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
/// use libcnb::layer_env::Scope;
/// use commons::layer::DefaultEnvLayer;
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
///                 layer_name!("default_env"),
///                 DefaultEnvLayer::new(
///                     [
///                         ("JRUBY_OPTS", "-Xcompile.invokedynamic=false"),
///                         ("RACK_ENV", "production"),
///                         ("RAILS_ENV", "production"),
///                         ("RAILS_SERVE_STATIC_FILES", "enabled"),
///                         ("RAILS_LOG_TO_STDOUT", "enabled"),
///                         ("MALLOC_ARENA_MAX", "2"),
///                         ("DISABLE_SPRING", "1"),
///                     ]
///                     .into_iter(),
///                 ),
///             )?;
///         let env = layer.env.apply(Scope::Build, &env);
///
///#        todo!()
///#     }
///# }
///
/// ```
#[deprecated(since = "1.1.0", note = "Use the layer Struct API instead")]
pub struct DefaultEnvLayer;

#[allow(deprecated)]
impl DefaultEnvLayer {
    #[allow(clippy::new_ret_no_self)]
    pub fn new<E, K, V, B>(env: E) -> ConfigureEnvLayer<B>
    where
        E: IntoIterator<Item = (K, V)> + Clone,
        K: Into<OsString>,
        V: Into<OsString>,
        B: libcnb::Buildpack,
    {
        let mut layer_env = LayerEnv::new();
        for (key, value) in env {
            layer_env =
                layer_env.chainable_insert(Scope::All, ModificationBehavior::Default, key, value);
        }

        ConfigureEnvLayer {
            data: layer_env,
            _buildpack: PhantomData,
        }
    }
}

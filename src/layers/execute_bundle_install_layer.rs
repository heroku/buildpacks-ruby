use crate::{util, RubyBuildpackError};
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::generic::GenericMetadata;

use std::path::Path;
use std::process::Command;

use crate::RubyBuildpack;
use libcnb::build::BuildContext;
use libcnb::layer::{Layer, LayerResult, LayerResultBuilder};
use libcnb::Env;
use std::ffi::OsString;

pub struct ExecuteBundleInstallLayer {
    pub env: Env,
}

impl Layer for ExecuteBundleInstallLayer {
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

        let mut command = Command::new("bundle");
        command.args(&["install"]);

        println!(
            "Running: {} ",
            util::command_to_str_with_env_keys(
                &command,
                &self.env,
                vec![
                    OsString::from("BUNDLE_BIN"),
                    OsString::from("BUNDLE_CLEAN"),
                    OsString::from("BUNDLE_DEPLOYMENT"),
                    OsString::from("BUNDLE_GEMFILE"),
                    OsString::from("BUNDLE_PATH"),
                    OsString::from("BUNDLE_WITHOUT"),
                ]
            )
        );

        util::run_simple_command(
            command.envs(&self.env),
            RubyBuildpackError::BundleInstallCommandError,
            RubyBuildpackError::BundleInstallUnexpectedExitStatus,
        )?;

        // TODO: Also record env vars
        LayerResultBuilder::new(GenericMetadata::default()).build()
    }
}

use std::path::Path;

use crate::util;

use tempfile::NamedTempFile;

use crate::{RubyBuildpack, RubyBuildpackError};
use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::generic::GenericMetadata;
use libcnb::layer::{Layer, LayerResult, LayerResultBuilder};

pub struct RubyLayer;

impl Layer for RubyLayer {
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
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        println!("---> Download and extracting Ruby");

        let tmp_ruby_tgz =
            NamedTempFile::new().map_err(RubyBuildpackError::CouldNotCreateTemporaryFile)?;

        let uri = "https://heroku-buildpack-ruby.s3.amazonaws.com/heroku-20/ruby-2.7.4.tgz";
        util::download(&uri, tmp_ruby_tgz.path()).map_err(RubyBuildpackError::RubyDownloadError)?;

        util::untar(tmp_ruby_tgz.path(), &layer_path)
            .map_err(RubyBuildpackError::RubyUntarError)?;

        LayerResultBuilder::new(GenericMetadata::default()).build()
    }
}

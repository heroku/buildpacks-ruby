use std::path::Path;

use crate::util;

use tempfile::NamedTempFile;

use crate::{RubyBuildpack, RubyBuildpackError};
use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::generic::GenericMetadata;
use libcnb::layer::{Layer, LayerResult, LayerResultBuilder};

pub struct RubyLayer;

use url::Url;

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
        let stack = "heroku-20";
        let version = "2.7.4";

        println!("---> Download and extracting Ruby {}", version);

        let tmp_ruby_tgz =
            NamedTempFile::new().map_err(RubyBuildpackError::CouldNotCreateTemporaryFile)?;

        let url = RubyLayer::download_url(stack, version);

        util::download(url.as_ref(), tmp_ruby_tgz.path())
            .map_err(RubyBuildpackError::RubyDownloadError)?;

        util::untar(tmp_ruby_tgz.path(), &layer_path)
            .map_err(RubyBuildpackError::RubyUntarError)?;

        LayerResultBuilder::new(GenericMetadata::default()).build()
    }
}

impl RubyLayer {
    fn download_url(stack: impl AsRef<str>, version: impl AsRef<str> + std::fmt::Display) -> Url {
        let filename = format!("ruby-{}.tgz", version);
        let mut url = Url::parse("https://heroku-buildpack-ruby.s3.amazonaws.com").unwrap();
        url.path_segments_mut()
            .unwrap()
            .push(stack.as_ref())
            .push(&filename);
        url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ruby_url() {
        let out = RubyLayer::download_url("heroku-20", "2.7.4");
        assert_eq!(
            out.as_ref(),
            "https://heroku-buildpack-ruby.s3.amazonaws.com/heroku-20/ruby-2.7.4.tgz",
        );
    }
}

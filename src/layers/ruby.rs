use std::path::Path;

use crate::util;
use crate::util::UrlError;

use tempfile::NamedTempFile;

use crate::{RubyBuildpack, RubyBuildpackError};
use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::generic::GenericMetadata;
use libcnb::layer::{Layer, LayerResult, LayerResultBuilder};

use libcnb::data::buildpack::StackId;

pub struct RubyLayer {
    pub version: String,
}

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
        context: &BuildContext<Self::Buildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        println!("---> Download and extracting Ruby {}", &self.version);

        let tmp_ruby_tgz =
            NamedTempFile::new().map_err(RubyBuildpackError::CouldNotCreateTemporaryFile)?;

        let url = RubyLayer::download_url(&context.stack_id, &self.version)
            .map_err(RubyBuildpackError::UrlParseError)?;

        util::download(url.as_ref(), tmp_ruby_tgz.path())
            .map_err(RubyBuildpackError::RubyDownloadError)?;

        util::untar(tmp_ruby_tgz.path(), &layer_path)
            .map_err(RubyBuildpackError::RubyUntarError)?;

        LayerResultBuilder::new(GenericMetadata::default()).build()
    }
}

impl RubyLayer {
    fn download_url(stack: &StackId, version: impl std::fmt::Display) -> Result<Url, UrlError> {
        let filename = format!("ruby-{}.tgz", version);
        let base = "https://heroku-buildpack-ruby.s3.amazonaws.com";
        let mut url = Url::parse(base).map_err(UrlError::UrlParseError)?;

        url.path_segments_mut()
            .map_err(|_| UrlError::InvalidBaseUrl(String::from(base)))?
            .push(stack)
            .push(&filename);
        Ok(url)
    }
}

#[cfg(test)]
mod tests {
    use libcnb::data::stack_id;

    use super::*;

    #[test]
    fn test_ruby_url() {
        let out = RubyLayer::download_url(&stack_id!("heroku-20"), "2.7.4").unwrap();
        assert_eq!(
            out.as_ref(),
            "https://heroku-buildpack-ruby.s3.amazonaws.com/heroku-20/ruby-2.7.4.tgz",
        );
    }
}

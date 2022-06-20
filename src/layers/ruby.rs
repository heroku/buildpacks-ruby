use std::path::Path;

use crate::util;
use crate::util::UrlError;

use tempfile::NamedTempFile;

use crate::{RubyBuildpack, RubyBuildpackError};
use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};

use crate::RubyVersion;
use libcnb::data::buildpack::StackId;

use serde::{Deserialize, Serialize};

// Installs an executable version of Ruby for the customer based on the
// passed in version value.
pub struct RubyLayer {
    pub version: RubyVersion,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RubyMetadata {
    pub version: String,
}

impl RubyLayer {
    pub fn version_string(&self) -> String {
        match &self.version {
            RubyVersion::Explicit(v) => v.clone(),
            RubyVersion::Default => String::from("3.1.2"),
        }
    }
}

use url::Url;

impl Layer for RubyLayer {
    type Buildpack = RubyBuildpack;
    type Metadata = RubyMetadata;

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
        println!(
            "---> Download and extracting Ruby {}",
            &self.version_string()
        );

        let tmp_ruby_tgz =
            NamedTempFile::new().map_err(RubyBuildpackError::CouldNotCreateTemporaryFile)?;

        let url = RubyLayer::download_url(&context.stack_id, &self.version_string())
            .map_err(RubyBuildpackError::UrlParseError)?;

        util::download(url.as_ref(), tmp_ruby_tgz.path())
            .map_err(RubyBuildpackError::RubyDownloadError)?;

        util::untar(tmp_ruby_tgz.path(), &layer_path)
            .map_err(RubyBuildpackError::RubyUntarError)?;

        LayerResultBuilder::new(RubyMetadata {
            version: self.version_string(),
        })
        .build()
    }

    fn existing_layer_strategy(
        &self,
        _context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        if self.version_string() == layer_data.content_metadata.metadata.version {
            println!(
                "---> Using previously installed Ruby version {}",
                self.version_string()
            );
            Ok(ExistingLayerStrategy::Keep)
        } else {
            Ok(ExistingLayerStrategy::Recreate)
        }
    }
}

impl RubyLayer {
    fn download_url(stack: &StackId, version: impl std::fmt::Display) -> Result<Url, UrlError> {
        let filename = format!("ruby-{}.tgz", version);
        let base = "https://heroku-buildpack-ruby.s3.us-east-1.amazonaws.com";
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
            "https://heroku-buildpack-ruby.s3.us-east-1.amazonaws.com/heroku-20/ruby-2.7.4.tgz",
        );
    }
}

use std::path::Path;

use crate::util;
use crate::util::UrlError;

use tempfile::NamedTempFile;

use crate::{RubyBuildpack, RubyBuildpackError};
use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};

use crate::lib::gemfile_lock::ResolvedRubyVersion;
use libcnb::data::buildpack::StackId;

use serde::{Deserialize, Serialize};

/*
# Install Ruby version

## Layer dir

The compiled Ruby tgz file is downloaded to a temporary directory and exported to `<layer-dir>`.
The tgz already contains a `bin/` directory with a `ruby` executable file.

## Environment variables

No environment variables are manually set. This layer relies on the
CNB lifecycle to add `<layer-dir>/bin` to the PATH.

## Cache invalidation

When the Ruby version changes, invalidate and re-run.

*/

#[derive(PartialEq, Eq)]
pub struct RubyVersionInstallLayer {
    pub version: ResolvedRubyVersion,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RubyMetadata {
    pub version: String,
    pub stack: StackId,
}

use url::Url;

impl Layer for RubyVersionInstallLayer {
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
        println!("---> Download and extracting Ruby {}", &self.version);

        let tmp_ruby_tgz =
            NamedTempFile::new().map_err(RubyBuildpackError::CouldNotCreateTemporaryFile)?;

        let url = RubyVersionInstallLayer::download_url(&context.stack_id, &self.version)
            .map_err(RubyBuildpackError::UrlParseError)?;

        util::download(url.as_ref(), tmp_ruby_tgz.path())
            .map_err(RubyBuildpackError::RubyDownloadError)?;

        util::untar(tmp_ruby_tgz.path(), layer_path).map_err(RubyBuildpackError::RubyUntarError)?;

        LayerResultBuilder::new(RubyMetadata {
            version: self.version.to_string(),
            stack: context.stack_id.clone(),
        })
        .build()
    }

    fn existing_layer_strategy(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        if context.stack_id == layer_data.content_metadata.metadata.stack {
            if self.version.to_string() == layer_data.content_metadata.metadata.version {
                println!(
                    "---> Using previously installed Ruby version {}",
                    self.version
                );
                Ok(ExistingLayerStrategy::Keep)
            } else {
                println!(
                    "---> Changing Ruby version from {} to {}",
                    layer_data.content_metadata.metadata.version, self.version
                );
                Ok(ExistingLayerStrategy::Recreate)
            }
        } else {
            println!("---> Stack has changed, reinstalling Ruby");
            Ok(ExistingLayerStrategy::Recreate)
        }
    }
}

impl RubyVersionInstallLayer {
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
        let out = RubyVersionInstallLayer::download_url(&stack_id!("heroku-20"), "2.7.4").unwrap();
        assert_eq!(
            out.as_ref(),
            "https://heroku-buildpack-ruby.s3.us-east-1.amazonaws.com/heroku-20/ruby-2.7.4.tgz",
        );
    }
}

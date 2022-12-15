use crate::{RubyBuildpack, RubyBuildpackError};
use commons::gemfile_lock::ResolvedRubyVersion;
use flate2::read::GzDecoder;
use libcnb::build::BuildContext;
use libcnb::data::buildpack::StackId;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;
use tar::Archive;
use tempfile::NamedTempFile;
use url::Url;

/// # Install Ruby version
///
/// ## Layer dir
///
/// The compiled Ruby tgz file is downloaded to a temporary directory and exported to `<layer-dir>`.
/// The tgz already contains a `bin/` directory with a `ruby` executable file.
///
/// This layer relies on the CNB lifecycle to add `<layer-dir>/bin` to the PATH.
///
/// ## Cache invalidation
///
/// When the Ruby version changes, invalidate and re-run.
///
#[derive(PartialEq, Eq)]
pub(crate) struct RubyInstallLayer {
    pub version: ResolvedRubyVersion,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct RubyInstallLayerMetadata {
    pub version: String,
    pub stack: StackId,
}

impl Layer for RubyInstallLayer {
    type Buildpack = RubyBuildpack;
    type Metadata = RubyInstallLayerMetadata;

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

        let tmp_ruby_tgz = NamedTempFile::new()
            .map_err(RubyInstallError::CouldNotCreateDestinationFile)
            .map_err(RubyBuildpackError::RubyInstallError)?;

        let url = RubyInstallLayer::download_url(&context.stack_id, &self.version)
            .map_err(RubyBuildpackError::RubyInstallError)?;

        download(url.as_ref(), tmp_ruby_tgz.path())
            .map_err(RubyBuildpackError::RubyInstallError)?;

        untar(tmp_ruby_tgz.path(), layer_path).map_err(RubyBuildpackError::RubyInstallError)?;

        LayerResultBuilder::new(RubyInstallLayerMetadata {
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

impl RubyInstallLayer {
    fn download_url(
        stack: &StackId,
        version: impl std::fmt::Display,
    ) -> Result<Url, RubyInstallError> {
        let filename = format!("ruby-{}.tgz", version);
        let base = "https://heroku-buildpack-ruby.s3.us-east-1.amazonaws.com";
        let mut url = Url::parse(base).map_err(RubyInstallError::UrlParseError)?;

        url.path_segments_mut()
            .map_err(|_| RubyInstallError::InvalidBaseUrl(String::from(base)))?
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
        let out = RubyInstallLayer::download_url(&stack_id!("heroku-20"), "2.7.4").unwrap();
        assert_eq!(
            out.as_ref(),
            "https://heroku-buildpack-ruby.s3.us-east-1.amazonaws.com/heroku-20/ruby-2.7.4.tgz",
        );
    }
}

pub(crate) fn download(
    uri: impl AsRef<str>,
    destination: impl AsRef<Path>,
) -> Result<(), RubyInstallError> {
    let mut response_reader = ureq::get(uri.as_ref())
        .call()
        .map_err(|err| RubyInstallError::RequestError(Box::new(err)))?
        .into_reader();

    let mut destination_file = fs::File::create(destination.as_ref())
        .map_err(RubyInstallError::CouldNotCreateDestinationFile)?;

    io::copy(&mut response_reader, &mut destination_file)
        .map_err(RubyInstallError::CouldNotWriteDestinationFile)?;

    Ok(())
}

pub(crate) fn untar(
    path: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<(), RubyInstallError> {
    let file = fs::File::open(path.as_ref()).map_err(RubyInstallError::CouldNotOpenFile)?;

    Archive::new(GzDecoder::new(file))
        .unpack(destination.as_ref())
        .map_err(RubyInstallError::CouldNotUnpack)
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum RubyInstallError {
    #[error("Could not parse url {0}")]
    UrlParseError(url::ParseError),

    #[error("Invalid base url {0}")]
    InvalidBaseUrl(String),

    #[error("Could not open file: {0}")]
    CouldNotOpenFile(std::io::Error),
    #[error("Could not untar: {0}")]
    CouldNotUnpack(std::io::Error),

    // Boxed to prevent `large_enum_variant` errors since `ureq::Error` is massive.
    #[error("Download error: {0}")]
    RequestError(Box<ureq::Error>),

    #[error("Could not create file: {0}")]
    CouldNotCreateDestinationFile(std::io::Error),

    #[error("Could not write file: {0}")]
    CouldNotWriteDestinationFile(std::io::Error),
}
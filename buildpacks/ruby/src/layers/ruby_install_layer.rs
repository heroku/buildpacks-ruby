use commons::output::{
    fmt::{self},
    section_log::{log_step, log_step_timed, SectionLogger},
};
use magic_migrate::{try_migrate_link, TryMigrate};

use crate::{
    target_id::{TargetId, TargetIdError},
    RubyBuildpack, RubyBuildpackError,
};
use commons::gemfile_lock::ResolvedRubyVersion;
use flate2::read::GzDecoder;
use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use serde::{Deserialize, Deserializer, Serialize};
use std::convert::Infallible;
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
pub(crate) struct RubyInstallLayer<'a> {
    pub(crate) _in_section: &'a dyn SectionLogger, // force the layer to be called within a Section logging context, not necessary but it's safer
    pub(crate) metadata: RubyInstallLayerMetadata,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct RubyInstallLayerMetadataV1 {
    pub(crate) stack: String,
    pub(crate) version: ResolvedRubyVersion,
}

#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
pub(crate) struct RubyInstallLayerMetadataV2 {
    pub(crate) target_id: TargetId,
    pub(crate) version: ResolvedRubyVersion,
}
try_migrate_link!(RubyInstallLayerMetadataV1, RubyInstallLayerMetadataV2);
pub(crate) type RubyInstallLayerMetadata = RubyInstallLayerMetadataV2;

#[derive(thiserror::Error, Debug)]
pub(crate) enum MetadataMigrateError {
    #[error("Cannot migrate metadata due to target id error: {0}")]
    TargetIdError(TargetIdError),
}

impl TryFrom<RubyInstallLayerMetadataV1> for RubyInstallLayerMetadataV2 {
    type Error = MetadataMigrateError;

    fn try_from(v1: RubyInstallLayerMetadataV1) -> Result<Self, Self::Error> {
        Ok(Self {
            target_id: TargetId::from_stack(&v1.stack)
                .map_err(MetadataMigrateError::TargetIdError)?,
            version: v1.version,
        })
    }
}

impl From<Infallible> for MetadataMigrateError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl TryMigrate for RubyInstallLayerMetadataV1 {
    type TryFrom = Self;
    type Error = MetadataMigrateError;

    fn deserializer<'de>(input: &str) -> impl Deserializer<'de> {
        toml::Deserializer::new(input)
    }
}

impl<'a> Layer for RubyInstallLayer<'a> {
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
        &mut self,
        _context: &BuildContext<Self::Buildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        log_step_timed("Installing", || {
            let tmp_ruby_tgz = NamedTempFile::new()
                .map_err(RubyInstallError::CouldNotCreateDestinationFile)
                .map_err(RubyBuildpackError::RubyInstallError)?;

            let url = download_url(&self.metadata.target_id, &self.metadata.version)
                .map_err(RubyBuildpackError::RubyInstallError)?;

            download(url.as_ref(), tmp_ruby_tgz.path())
                .map_err(RubyBuildpackError::RubyInstallError)?;

            untar(tmp_ruby_tgz.path(), layer_path).map_err(RubyBuildpackError::RubyInstallError)?;

            LayerResultBuilder::new(self.metadata.clone()).build()
        })
    }

    fn migrate_incompatible_metadata(
        &mut self,
        _context: &BuildContext<Self::Buildpack>,
        metadata: &libcnb::generic::GenericMetadata,
    ) -> Result<
        libcnb::layer::MetadataMigration<Self::Metadata>,
        <Self::Buildpack as libcnb::Buildpack>::Error,
    > {
        match Self::Metadata::try_from_str_migrations(
            &toml::to_string(&metadata).expect("TOML deserialization of GenericMetadata"),
        ) {
            Some(Ok(metadata)) => Ok(libcnb::layer::MetadataMigration::ReplaceMetadata(metadata)),
            Some(Err(e)) => {
                log_step(format!("Clearing cache (metadata migration error {e})"));
                Ok(libcnb::layer::MetadataMigration::RecreateLayer)
            }
            None => {
                log_step("Clearing cache (invalid metadata)");
                Ok(libcnb::layer::MetadataMigration::RecreateLayer)
            }
        }
    }

    fn existing_layer_strategy(
        &mut self,
        _context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        let old = &layer_data.content_metadata.metadata;
        let now = self.metadata.clone();

        match cache_state(old.clone(), now) {
            Changed::Nothing(_version) => {
                log_step("Using cached version");

                Ok(ExistingLayerStrategy::Keep)
            }
            Changed::Target(_old, _now) => {
                log_step(format!("Clearing cache {}", fmt::details("OS changed")));

                Ok(ExistingLayerStrategy::Recreate)
            }
            Changed::RubyVersion(_old, _now) => {
                log_step(format!(
                    "Clearing cache {}",
                    fmt::details("ruby version changed")
                ));

                Ok(ExistingLayerStrategy::Recreate)
            }
        }
    }
}

fn cache_state(old: RubyInstallLayerMetadata, now: RubyInstallLayerMetadata) -> Changed {
    let RubyInstallLayerMetadata { target_id, version } = now;

    if old.target_id != target_id {
        Changed::Target(old.target_id, target_id)
    } else if old.version != version {
        Changed::RubyVersion(old.version, version)
    } else {
        Changed::Nothing(version)
    }
}

#[derive(Debug)]
enum Changed {
    Nothing(ResolvedRubyVersion),
    Target(TargetId, TargetId),
    RubyVersion(ResolvedRubyVersion, ResolvedRubyVersion),
}

fn download_url(
    target: &TargetId,
    version: impl std::fmt::Display,
) -> Result<Url, RubyInstallError> {
    let filename = format!("ruby-{version}.tgz");
    let base = "https://heroku-buildpack-ruby.s3.us-east-1.amazonaws.com";
    let mut url = Url::parse(base).map_err(RubyInstallError::UrlParseError)?;

    url.path_segments_mut()
        .map_err(|()| RubyInstallError::InvalidBaseUrl(String::from(base)))?
        .push(&target.stack_name().map_err(RubyInstallError::TargetError)?)
        .push(&filename);
    Ok(url)
}

pub(crate) fn download(
    uri: impl AsRef<str>,
    destination: impl AsRef<Path>,
) -> Result<(), RubyInstallError> {
    let mut response_reader = ureq::get(uri.as_ref())
        .call()
        .map_err(|err| RubyInstallError::RequestError(Box::new(err)))?
        .into_reader();

    let mut destination_file = fs_err::File::create(destination.as_ref())
        .map_err(RubyInstallError::CouldNotCreateDestinationFile)?;

    io::copy(&mut response_reader, &mut destination_file)
        .map_err(RubyInstallError::CouldNotWriteDestinationFile)?;

    Ok(())
}

pub(crate) fn untar(
    path: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<(), RubyInstallError> {
    let file = fs_err::File::open(path.as_ref()).map_err(RubyInstallError::CouldNotOpenFile)?;

    Archive::new(GzDecoder::new(file))
        .unpack(destination.as_ref())
        .map_err(RubyInstallError::CouldNotUnpack)
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum RubyInstallError {
    #[error("Unknown install target: {0}")]
    TargetError(TargetIdError),

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

#[cfg(test)]
mod tests {
    use super::*;

    /// If this test fails due to a change you'll need to
    /// implement `TryMigrate` for the new layer data and add
    /// another test ensuring the latest metadata struct can
    /// be built from the previous version.
    #[test]
    fn metadata_guard() {
        let metadata = RubyInstallLayerMetadata {
            target_id: TargetId {
                arch: String::from("amd64"),
                distro_name: String::from("ubuntu"),
                distro_version: String::from("22.04"),
            },
            version: ResolvedRubyVersion(String::from("3.1.3")),
        };

        let actual = toml::to_string(&metadata).unwrap();
        let expected = r#"
version = "3.1.3"

[target_id]
arch = "amd64"
distro_name = "ubuntu"
distro_version = "22.04"
"#
        .trim();
        assert_eq!(expected, actual.trim());
    }

    #[test]
    fn metadata_migrate_v1_to_v2() {
        let metadata = RubyInstallLayerMetadataV1 {
            stack: String::from("heroku-22"),
            version: ResolvedRubyVersion(String::from("3.1.3")),
        };

        let actual = toml::to_string(&metadata).unwrap();
        let expected = r#"
stack = "heroku-22"
version = "3.1.3"
"#
        .trim();
        assert_eq!(expected, actual.trim());

        let deserialized: RubyInstallLayerMetadataV2 =
            RubyInstallLayerMetadataV2::try_from_str_migrations(&actual)
                .unwrap()
                .unwrap();

        let expected = RubyInstallLayerMetadataV2 {
            target_id: TargetId::from_stack(&metadata.stack).expect("Valid stack"),
            version: metadata.version,
        };
        assert_eq!(expected, deserialized);
    }

    #[test]
    fn test_ruby_url() {
        let out = download_url(
            &TargetId {
                arch: String::from("amd64"),
                distro_name: String::from("ubuntu"),
                distro_version: String::from("22.04"),
            },
            "2.7.4",
        )
        .unwrap();
        assert_eq!(
            out.as_ref(),
            "https://heroku-buildpack-ruby.s3.us-east-1.amazonaws.com/heroku-22/ruby-2.7.4.tgz",
        );
    }
}

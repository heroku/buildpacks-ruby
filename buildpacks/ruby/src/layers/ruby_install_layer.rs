//! # Install Ruby version
//!
//! ## Layer dir
//!
//! The compiled Ruby tgz file is downloaded to a temporary directory and exported to `<layer-dir>`.
//! The tgz already contains a `bin/` directory with a `ruby` executable file.
//!
//! This layer relies on the CNB lifecycle to add `<layer-dir>/bin` to the PATH.
//!
//! ## Cache invalidation
//!
//! When the Ruby version changes, invalidate and re-run.
//!
use crate::layers::shared::{invalid_metadata_action, MetadataDiff};
use crate::{
    target_id::{TargetId, TargetIdError},
    RubyBuildpack, RubyBuildpackError,
};
use bullet_stream::state::SubBullet;
use bullet_stream::{style, Print};
use commons::display::SentenceList;
use commons::gemfile_lock::ResolvedRubyVersion;
use flate2::read::GzDecoder;
use libcnb::data::layer_name;
use libcnb::layer::{
    CachedLayerDefinition, EmptyLayerCause, InvalidMetadataAction, LayerState, RestoredLayerAction,
};
use libcnb::layer_env::LayerEnv;
use magic_migrate::{try_migrate_deserializer_chain, TryMigrate};
use serde::{Deserialize, Deserializer, Serialize};
use std::convert::Infallible;
use std::io::{self, Stdout};
use std::path::Path;
use tar::Archive;
use tempfile::NamedTempFile;
use url::Url;

pub(crate) fn handle(
    context: &libcnb::build::BuildContext<RubyBuildpack>,
    mut bullet: Print<SubBullet<Stdout>>,
    metadata: Metadata,
) -> libcnb::Result<(Print<SubBullet<Stdout>>, LayerEnv), RubyBuildpackError> {
    let layer_ref = context.cached_layer(
        layer_name!("ruby"),
        CachedLayerDefinition {
            build: true,
            launch: true,
            invalid_metadata_action: &invalid_metadata_action,
            restored_layer_action: &|old: &Metadata, _| {
                let diff = metadata.diff(old);
                if diff.is_empty() {
                    (
                        RestoredLayerAction::KeepLayer,
                        "using cached version".to_string(),
                    )
                } else {
                    (
                        RestoredLayerAction::DeleteLayer,
                        format!(
                            "due to {changes}: {differences}",
                            changes = if diff.len() > 1 { "changes" } else { "change" },
                            differences = SentenceList::new(&diff)
                        ),
                    )
                }
            },
        },
    )?;
    match &layer_ref.state {
        LayerState::Restored { cause: _ } => {
            bullet = bullet.sub_bullet("Using cached Ruby version");
        }
        LayerState::Empty { cause } => {
            match cause {
                EmptyLayerCause::NewlyCreated => {}
                EmptyLayerCause::InvalidMetadataAction { cause }
                | EmptyLayerCause::RestoredLayerAction { cause } => {
                    bullet = bullet.sub_bullet(format!("Clearing cache {cause}"));
                }
            }
            let timer = bullet.start_timer("Installing");
            install_ruby(&metadata, &layer_ref.path())?;
            bullet = timer.done();
        }
    }
    layer_ref.write_metadata(metadata)?;
    Ok((bullet, layer_ref.read_env()?))
}

fn install_ruby(metadata: &Metadata, layer_path: &Path) -> Result<(), RubyBuildpackError> {
    let tmp_ruby_tgz = NamedTempFile::new()
        .map_err(RubyInstallError::CouldNotCreateDestinationFile)
        .map_err(RubyBuildpackError::RubyInstallError)?;

    let url = download_url(&metadata.target_id(), &metadata.ruby_version)
        .map_err(RubyBuildpackError::RubyInstallError)?;

    download(url.as_ref(), tmp_ruby_tgz.path()).map_err(RubyBuildpackError::RubyInstallError)?;

    untar(tmp_ruby_tgz.path(), layer_path).map_err(RubyBuildpackError::RubyInstallError)?;

    Ok(())
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct MetadataV1 {
    pub(crate) stack: String,
    pub(crate) version: ResolvedRubyVersion,
}

#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
pub(crate) struct MetadataV2 {
    pub(crate) distro_name: String,
    pub(crate) distro_version: String,
    pub(crate) cpu_architecture: String,
    pub(crate) ruby_version: ResolvedRubyVersion,
}
pub(crate) type Metadata = MetadataV2;

impl MetadataV2 {
    pub(crate) fn target_id(&self) -> TargetId {
        TargetId {
            cpu_architecture: self.cpu_architecture.clone(),
            distro_name: self.distro_name.clone(),
            distro_version: self.distro_version.clone(),
        }
    }
}

try_migrate_deserializer_chain!(
    chain: [MetadataV1, MetadataV2],
    error: MetadataMigrateError,
    deserializer: toml::Deserializer::new,
);

#[derive(thiserror::Error, Debug)]
pub(crate) enum MetadataMigrateError {
    #[error("Cannot migrate metadata due to target id error: {0}")]
    TargetIdError(TargetIdError),
}

impl TryFrom<MetadataV1> for MetadataV2 {
    type Error = MetadataMigrateError;

    fn try_from(v1: MetadataV1) -> Result<Self, Self::Error> {
        let target_id =
            TargetId::from_stack(&v1.stack).map_err(MetadataMigrateError::TargetIdError)?;

        Ok(Self {
            distro_name: target_id.distro_name,
            distro_version: target_id.distro_version,
            cpu_architecture: target_id.cpu_architecture,
            ruby_version: v1.version,
        })
    }
}

impl MetadataDiff for Metadata {
    fn diff(&self, old: &Self) -> Vec<String> {
        let mut differences = Vec::new();
        let Metadata {
            distro_name,
            distro_version,
            cpu_architecture,
            ruby_version,
        } = old;
        if ruby_version != &self.ruby_version {
            differences.push(format!(
                "Ruby version ({old} to {now})",
                old = style::value(ruby_version.to_string()),
                now = style::value(self.ruby_version.to_string())
            ));
        }
        if distro_name != &self.distro_name {
            differences.push(format!(
                "distro name ({old} to {now})",
                old = style::value(distro_name),
                now = style::value(&self.distro_name)
            ));
        }
        if distro_version != &self.distro_version {
            differences.push(format!(
                "distro version ({old} to {now})",
                old = style::value(distro_version),
                now = style::value(&self.distro_version)
            ));
        }
        if cpu_architecture != &self.cpu_architecture {
            differences.push(format!(
                "CPU architecture ({old} to {now})",
                old = style::value(cpu_architecture),
                now = style::value(&self.cpu_architecture)
            ));
        }

        differences
    }
}

fn download_url(
    target: &TargetId,
    version: impl std::fmt::Display,
) -> Result<Url, RubyInstallError> {
    let filename = format!("ruby-{version}.tgz");
    let base = "https://heroku-buildpack-ruby.s3.us-east-1.amazonaws.com";
    let mut url = Url::parse(base).map_err(RubyInstallError::UrlParseError)?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|()| RubyInstallError::InvalidBaseUrl(String::from(base)))?;

        segments.push(&target.stack_name().map_err(RubyInstallError::TargetError)?);
        if target.is_arch_aware() {
            segments.push(&target.cpu_architecture);
        }
        segments.push(&filename);
    }

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
        let metadata = Metadata {
            distro_name: String::from("ubuntu"),
            distro_version: String::from("22.04"),
            cpu_architecture: String::from("amd64"),
            ruby_version: ResolvedRubyVersion(String::from("3.1.3")),
        };

        let actual = toml::to_string(&metadata).unwrap();
        let expected = r#"
distro_name = "ubuntu"
distro_version = "22.04"
cpu_architecture = "amd64"
ruby_version = "3.1.3"
"#
        .trim();
        assert_eq!(expected, actual.trim());
    }

    #[test]
    fn metadata_migrate_v1_to_v2() {
        let metadata = MetadataV1 {
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

        let deserialized: MetadataV2 = MetadataV2::try_from_str_migrations(&actual)
            .unwrap()
            .unwrap();

        let target_id = TargetId::from_stack(&metadata.stack).unwrap();
        let expected = MetadataV2 {
            distro_name: target_id.distro_name,
            distro_version: target_id.distro_version,
            cpu_architecture: target_id.cpu_architecture,
            ruby_version: metadata.version,
        };
        assert_eq!(expected, deserialized);
    }

    #[test]
    fn test_ruby_url() {
        let out = download_url(
            &TargetId {
                cpu_architecture: String::from("amd64"),
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

use crate::{
    target_id::{TargetId, TargetIdError},
    RubyBuildpack, RubyBuildpackError,
};
use bullet_stream::state::SubBullet;
use bullet_stream::Print;
use commons::gemfile_lock::ResolvedRubyVersion;
use commons::layer::MetadataMigrationFYI;
use flate2::read::GzDecoder;
use libcnb::build::BuildContext;
use libcnb::data::layer_name;
use libcnb::layer::{
    CachedLayerDefinition, EmptyLayerCause, IntoAction, InvalidMetadataAction, LayerState,
    RestoredLayerAction,
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

type Metadata = RubyInstallLayerMetadata;

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
///
pub(crate) fn install_ruby(
    context: &BuildContext<RubyBuildpack>,
    mut bullet: Print<SubBullet<Stdout>>,
    new_metadata: RubyInstallLayerMetadata,
) -> Result<(Print<SubBullet<Stdout>>, LayerEnv), libcnb::Error<RubyBuildpackError>> {
    let layer = context.cached_layer(
        layer_name!("ruby"),
        CachedLayerDefinition {
            build: true,
            launch: true,
            invalid_metadata_action: &|invalid_metadata| {
                let toml_string = invalid_metadata.as_ref().map_or_else(String::new, |m| {
                    toml::to_string(m).expect("TOML serializes back to toml")
                });

                match Metadata::try_from_str_migrations(&toml_string) {
                    Some(Ok(migrated)) => {
                        MetadataMigrationFYI::Migrated(migrated, "Success".to_string())
                    }
                    Some(Err(error)) => MetadataMigrationFYI::Delete(format!(
                        "Error while migrating metadata {error}"
                    )),
                    None => MetadataMigrationFYI::Delete(format!(
                        "Could not serialize metadata into a known struct. Metadata: {toml_string}",
                    )),
                }
            },
            restored_layer_action: &|old_metadata: &Metadata, _| {
                cache_state(old_metadata.clone(), new_metadata.clone())
            },
        },
    )?;

    match layer.state {
        LayerState::Restored { .. } => {
            bullet = bullet.sub_bullet("Using cached Ruby version");
        }
        LayerState::Empty {
            cause:
                EmptyLayerCause::InvalidMetadataAction { ref cause }
                | EmptyLayerCause::RestoredLayerAction { ref cause },
        } => {
            bullet = bullet.sub_bullet(format!("Clearing cache ({cause})"));
        }
        LayerState::Empty {
            cause: EmptyLayerCause::NewlyCreated,
        } => {}
    };

    match layer.state {
        LayerState::Restored { .. } => {}
        LayerState::Empty { .. } => {
            let timer = bullet.start_timer("Installing");
            let tmp_ruby_tgz = NamedTempFile::new()
                .map_err(RubyInstallError::CouldNotCreateDestinationFile)
                .map_err(RubyBuildpackError::RubyInstallError)?;

            let url = download_url(&new_metadata.target_id(), &new_metadata.ruby_version)
                .map_err(RubyBuildpackError::RubyInstallError)?;

            download(url.as_ref(), tmp_ruby_tgz.path())
                .map_err(RubyBuildpackError::RubyInstallError)?;

            untar(tmp_ruby_tgz.path(), layer.path())
                .map_err(RubyBuildpackError::RubyInstallError)?;

            layer.write_metadata(new_metadata)?;
            bullet = timer.done();
        }
    };

    Ok((bullet, layer.read_env()?))
}

impl<E> IntoAction<RestoredLayerAction, String, E> for Changed {
    fn into_action(self) -> Result<(RestoredLayerAction, String), E> {
        match self {
            Changed::Nothing => Ok((
                RestoredLayerAction::KeepLayer,
                "Using Cached Ruby Version".to_string(),
            )),
            Changed::CpuArchitecture(old, now) => Ok((
                RestoredLayerAction::DeleteLayer,
                format!("CPU architecture changed: {old} to {now}"),
            )),
            Changed::DistroVersion(old, now) => Ok((
                RestoredLayerAction::DeleteLayer,
                format!("distro version changed: {old} to {now}"),
            )),
            Changed::DistroName(old, now) => Ok((
                RestoredLayerAction::DeleteLayer,
                format!("distro name changed: {old} to {now}"),
            )),
            Changed::RubyVersion(old, now) => Ok((
                RestoredLayerAction::DeleteLayer,
                format!("Ruby version changed: {old} to {now}"),
            )),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct RubyInstallLayerMetadataV1 {
    pub(crate) stack: String,
    pub(crate) version: ResolvedRubyVersion,
}

#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
pub(crate) struct RubyInstallLayerMetadataV2 {
    pub(crate) distro_name: String,
    pub(crate) distro_version: String,
    pub(crate) cpu_architecture: String,
    pub(crate) ruby_version: ResolvedRubyVersion,
}

impl RubyInstallLayerMetadataV2 {
    pub(crate) fn target_id(&self) -> TargetId {
        TargetId {
            cpu_architecture: self.cpu_architecture.clone(),
            distro_name: self.distro_name.clone(),
            distro_version: self.distro_version.clone(),
        }
    }
}

try_migrate_deserializer_chain!(
    chain: [RubyInstallLayerMetadataV1, RubyInstallLayerMetadataV2],
    error: MetadataMigrateError,
    deserializer: toml::Deserializer::new,
);
pub(crate) type RubyInstallLayerMetadata = RubyInstallLayerMetadataV2;

#[derive(thiserror::Error, Debug)]
pub(crate) enum MetadataMigrateError {
    #[error("Cannot migrate metadata due to target id error: {0}")]
    TargetIdError(TargetIdError),
}

impl TryFrom<RubyInstallLayerMetadataV1> for RubyInstallLayerMetadataV2 {
    type Error = MetadataMigrateError;

    fn try_from(v1: RubyInstallLayerMetadataV1) -> Result<Self, Self::Error> {
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

fn cache_state(old: RubyInstallLayerMetadata, now: RubyInstallLayerMetadata) -> Changed {
    let RubyInstallLayerMetadata {
        distro_name,
        distro_version,
        cpu_architecture,
        ruby_version,
    } = now;

    if old.distro_name != distro_name {
        Changed::DistroName(old.distro_name, distro_name)
    } else if old.distro_version != distro_version {
        Changed::DistroVersion(old.distro_version, distro_version)
    } else if old.cpu_architecture != cpu_architecture {
        Changed::CpuArchitecture(old.cpu_architecture, cpu_architecture)
    } else if old.ruby_version != ruby_version {
        Changed::RubyVersion(old.ruby_version, ruby_version)
    } else {
        Changed::Nothing
    }
}

#[derive(Debug)]
enum Changed {
    Nothing,
    DistroName(String, String),
    DistroVersion(String, String),
    CpuArchitecture(String, String),
    RubyVersion(ResolvedRubyVersion, ResolvedRubyVersion),
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
        let metadata = RubyInstallLayerMetadata {
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

        let target_id = TargetId::from_stack(&metadata.stack).unwrap();
        let expected = RubyInstallLayerMetadataV2 {
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

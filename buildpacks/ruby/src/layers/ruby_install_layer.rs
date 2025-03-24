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
use crate::target_id::OsDistribution;
use crate::{
    target_id::{TargetId, TargetIdError},
    RubyBuildpack, RubyBuildpackError,
};
use bullet_stream::state::SubBullet;
use bullet_stream::Print;
use cache_diff::CacheDiff;
use commons::gemfile_lock::ResolvedRubyVersion;
use commons::layer::diff_migrate::{DiffMigrateLayer, LayerRename};
use flate2::read::GzDecoder;
use libcnb::data::layer_name;
use libcnb::layer::{EmptyLayerCause, LayerState};
use libcnb::layer_env::LayerEnv;
use magic_migrate::TryMigrate;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use tar::Archive;
use tempfile::NamedTempFile;
use url::Url;

pub(crate) fn handle<W>(
    context: &libcnb::build::BuildContext<RubyBuildpack>,
    mut bullet: Print<SubBullet<W>>,
    metadata: &Metadata,
) -> libcnb::Result<(Print<SubBullet<W>>, LayerEnv), RubyBuildpackError>
where
    W: Write + Send + Sync + 'static,
{
    let layer_ref = DiffMigrateLayer {
        build: true,
        launch: true,
    }
    .cached_layer_rename(
        LayerRename {
            to: layer_name!("binruby"),
            from: vec![layer_name!("ruby")],
        },
        context,
        metadata,
    )?;
    match &layer_ref.state {
        LayerState::Restored { cause } => {
            bullet = bullet.sub_bullet(cause);
        }
        LayerState::Empty { cause } => {
            match cause {
                EmptyLayerCause::NewlyCreated => {}
                EmptyLayerCause::InvalidMetadataAction { cause }
                | EmptyLayerCause::RestoredLayerAction { cause } => {
                    bullet = bullet.sub_bullet(cause);
                }
            }
            let timer = bullet.start_timer("Installing");
            install_ruby(metadata, &layer_ref.path())?;
            bullet = timer.done();
        }
    }
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

#[derive(Deserialize, Serialize, Debug, Clone, TryMigrate)]
#[try_migrate(from = None)]
#[serde(deny_unknown_fields)]
pub(crate) struct MetadataV1 {
    pub(crate) stack: String,
    pub(crate) version: ResolvedRubyVersion,
}

#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq, TryMigrate)]
#[try_migrate(from = MetadataV1)]
#[serde(deny_unknown_fields)]
pub(crate) struct MetadataV2 {
    pub(crate) distro_name: String,
    pub(crate) distro_version: String,
    pub(crate) cpu_architecture: String,
    pub(crate) ruby_version: ResolvedRubyVersion,
}

#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq, CacheDiff, TryMigrate)]
#[try_migrate(from = MetadataV2)]
#[serde(deny_unknown_fields)]
pub(crate) struct MetadataV3 {
    #[cache_diff(rename = "OS Distribution")]
    pub(crate) os_distribution: OsDistribution,
    #[cache_diff(rename = "CPU architecture")]
    pub(crate) cpu_architecture: String,
    #[cache_diff(rename = "Ruby version")]
    pub(crate) ruby_version: ResolvedRubyVersion,
}

impl MetadataV3 {
    pub(crate) fn target_id(&self) -> TargetId {
        TargetId {
            cpu_architecture: self.cpu_architecture.clone(),
            distro_name: self.os_distribution.name.clone(),
            distro_version: self.os_distribution.version.clone(),
        }
    }
}

pub(crate) type Metadata = MetadataV3;

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

impl TryFrom<MetadataV2> for MetadataV3 {
    type Error = MetadataMigrateError;

    fn try_from(v2: MetadataV2) -> Result<Self, Self::Error> {
        Ok(Self {
            os_distribution: OsDistribution {
                name: v2.distro_name,
                version: v2.distro_version,
            },
            cpu_architecture: v2.cpu_architecture,
            ruby_version: v2.ruby_version,
        })
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

#[tracing::instrument(name = "download_ruby", err)]
pub(crate) fn download(uri: &str, destination: &Path) -> Result<(), RubyInstallError> {
    let mut response_reader = ureq::get(uri)
        .call()
        .map_err(|err| RubyInstallError::RequestError(Box::new(err)))?
        .into_reader();

    let mut destination_file = fs_err::File::create(destination)
        .map_err(RubyInstallError::CouldNotCreateDestinationFile)?;

    std::io::copy(&mut response_reader, &mut destination_file)
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
    use crate::layers::shared::temp_build_context;
    use bullet_stream::strip_ansi;

    /// If this test fails due to a change you'll need to
    /// implement `TryMigrate` for the new layer data and add
    /// another test ensuring the latest metadata struct can
    /// be built from the previous version.
    #[test]
    fn metadata_guard() {
        let metadata = Metadata {
            os_distribution: OsDistribution {
                name: String::from("ubuntu"),
                version: String::from("22.04"),
            },
            cpu_architecture: String::from("amd64"),
            ruby_version: ResolvedRubyVersion(String::from("3.1.3")),
        };

        let actual = toml::to_string(&metadata).unwrap();
        let expected = r#"
cpu_architecture = "amd64"
ruby_version = "3.1.3"

[os_distribution]
name = "ubuntu"
version = "22.04"
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

    #[test]
    fn metadata_diff_messages() {
        let old = Metadata {
            ruby_version: ResolvedRubyVersion("3.5.3".to_string()),
            os_distribution: OsDistribution {
                name: "ubuntu".to_string(),
                version: "20.04".to_string(),
            },
            cpu_architecture: "amd64".to_string(),
        };
        assert_eq!(old.diff(&old), Vec::<String>::new());

        let diff = Metadata {
            ruby_version: ResolvedRubyVersion("3.5.5".to_string()),
            os_distribution: OsDistribution {
                name: "ubuntu".to_string(),
                version: "20.04".to_string(),
            },
            cpu_architecture: old.cpu_architecture.clone(),
        }
        .diff(&old);

        assert_eq!(
            diff.iter().map(strip_ansi).collect::<Vec<String>>(),
            vec!["Ruby version (`3.5.3` to `3.5.5`)".to_string()]
        );

        let diff = Metadata {
            ruby_version: old.ruby_version.clone(),
            os_distribution: OsDistribution {
                name: "alpine".to_string(),
                version: "3.20.0".to_string(),
            },
            cpu_architecture: old.cpu_architecture.clone(),
        }
        .diff(&old);

        assert_eq!(
            diff.iter().map(strip_ansi).collect::<Vec<String>>(),
            vec!["OS Distribution (`ubuntu 20.04` to `alpine 3.20.0`)".to_string()]
        );

        let diff = Metadata {
            ruby_version: old.ruby_version.clone(),
            os_distribution: OsDistribution {
                name: old.os_distribution.name.clone(),
                version: old.os_distribution.version.clone(),
            },
            cpu_architecture: "arm64".to_string(),
        }
        .diff(&old);

        assert_eq!(
            diff.iter().map(strip_ansi).collect::<Vec<String>>(),
            vec!["CPU architecture (`amd64` to `arm64`)".to_string()]
        );
    }

    #[test]
    fn test_ruby_version_difference_clears_cache() {
        let temp = tempfile::tempdir().unwrap();
        let context = temp_build_context::<RubyBuildpack>(temp.path());
        let old = Metadata {
            ruby_version: ResolvedRubyVersion("2.7.2".to_string()),
            os_distribution: OsDistribution {
                name: "ubuntu".to_string(),
                version: "20.04".to_string(),
            },
            cpu_architecture: "x86_64".to_string(),
        };
        let differences = old.diff(&old);
        assert_eq!(differences, Vec::<String>::new());

        DiffMigrateLayer {
            build: true,
            launch: true,
        }
        .cached_layer(layer_name!("ruby"), &context, &old)
        .unwrap();
        let result = DiffMigrateLayer {
            build: true,
            launch: true,
        }
        .cached_layer(layer_name!("ruby"), &context, &old)
        .unwrap();
        let actual = result.state;
        assert!(matches!(actual, LayerState::Restored { .. }));

        let now = Metadata {
            ruby_version: ResolvedRubyVersion("3.0.0".to_string()),
            ..old.clone()
        };
        let differences = now.diff(&old);
        assert_eq!(differences.len(), 1);

        let result = DiffMigrateLayer {
            build: true,
            launch: true,
        }
        .cached_layer(layer_name!("ruby"), &context, &now)
        .unwrap();
        assert!(matches!(
            result.state,
            LayerState::Empty {
                cause: EmptyLayerCause::RestoredLayerAction { .. }
            }
        ));
    }
}

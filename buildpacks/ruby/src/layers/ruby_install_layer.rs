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
use bullet_stream::global::print;
use cache_diff::CacheDiff;
use commons::gemfile_lock::ResolvedRubyVersion;
use commons::layer::diff_migrate::{DiffMigrateLayer, LayerRename};
use flate2::read::GzDecoder;
use libcnb::data::layer_name;
use libcnb::layer::{EmptyLayerCause, LayerState};
use libcnb::layer_env::LayerEnv;
use libherokubuildpack::download::{download_file, DownloadError};
use magic_migrate::TryMigrate;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::thread;
use std::time::Duration;
use tar::Archive;
use tempfile::NamedTempFile;
use url::Url;
const MAX_ATTEMPTS: u8 = 3;

// Latest metadata used for `TryMigrate` trait
pub(crate) type Metadata = MetadataV3;

pub(crate) fn call(
    context: &libcnb::build::BuildContext<RubyBuildpack>,
    metadata: &Metadata,
) -> libcnb::Result<LayerEnv, RubyBuildpackError> {
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
            print::sub_bullet(cause);
        }
        LayerState::Empty { cause } => {
            match cause {
                EmptyLayerCause::NewlyCreated => {}
                EmptyLayerCause::InvalidMetadataAction { cause }
                | EmptyLayerCause::RestoredLayerAction { cause } => {
                    print::sub_bullet(cause);
                }
            }
            install_ruby(metadata, &layer_ref.path())
                .map_err(RubyBuildpackError::RubyInstallError)?;
        }
    }
    layer_ref.read_env()
}

#[tracing::instrument(skip_all)]
fn install_ruby(metadata: &Metadata, layer_path: &Path) -> Result<(), RubyInstallError> {
    let mut timer = print::sub_start_timer("Installing");
    let tmp_ruby_tgz =
        NamedTempFile::new().map_err(RubyInstallError::CouldNotCreateDestinationFile)?;

    let url = download_url(&metadata.target_id(), &metadata.ruby_version)?;
    let mut attempts = 0;
    loop {
        attempts += 1;
        match download_file(url.as_ref(), tmp_ruby_tgz.path())
            .map_err(RubyInstallError::CouldNotDownload)
        {
            Ok(()) => break,
            Err(error) => {
                if attempts >= MAX_ATTEMPTS {
                    return Err(error);
                }
                timer.cancel(format!("{error}"));
                thread::sleep(Duration::from_secs(1));
                timer = print::sub_start_timer("Retrying");
            }
        }
    }

    untar(tmp_ruby_tgz.path(), layer_path)?;
    timer.done();
    Ok(())
}

// Introduced 2024-12-13 https://github.com/heroku/buildpacks-ruby/pull/370
#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq, CacheDiff, TryMigrate)]
#[try_migrate(from = None)]
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

    #[error("Could not create file: {0}")]
    CouldNotCreateDestinationFile(std::io::Error),

    #[error("Error downloading: {0}")]
    CouldNotDownload(DownloadError),
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
                version: "24.04".to_string(),
            },
            cpu_architecture: "amd64".to_string(),
        };
        assert_eq!(old.diff(&old), Vec::<String>::new());

        let diff = Metadata {
            ruby_version: ResolvedRubyVersion("3.5.5".to_string()),
            os_distribution: OsDistribution {
                name: "ubuntu".to_string(),
                version: "24.04".to_string(),
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
            vec!["OS Distribution (`ubuntu 24.04` to `alpine 3.20.0`)".to_string()]
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
                version: "24.04".to_string(),
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

use crate::{RubyBuildpack, RubyBuildpackError};
use bullet_stream::state::SubBullet;
use bullet_stream::Print;
use flate2::read::GzDecoder;
use libcnb::additional_buildpack_binary_path;
use libcnb::build::BuildContext;
use libcnb::data::layer_name;
use libcnb::layer::LayerState;
use libcnb::layer::{CachedLayerDefinition, InvalidMetadataAction, RestoredLayerAction};
use libcnb::layer_env::LayerEnv;
use libherokubuildpack::digest::sha256;
use serde::{Deserialize, Serialize};
use std::io::Stdout;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tar::Archive;
use tempfile::NamedTempFile;

/// Agentmon URL
///
/// - Repo: <https://github.com/heroku/agentmon>
/// - Releases: <https://github.com/heroku/agentmon/releases>
///
/// To get the latest s3 url:
///
/// ```shell
/// $ curl https://agentmon-releases.s3.us-east-1.amazonaws.com/latest
/// ```
const DOWNLOAD_URL: &str =
    "https://agentmon-releases.s3.us-east-1.amazonaws.com/agentmon-0.3.1-linux-amd64.tar.gz";
const DOWNLOAD_SHA: &str = "f9bf9f33c949e15ffed77046ca38f8dae9307b6a0181c6af29a25dec46eb2dac";

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub(crate) struct Metadata {
    download_url: Option<String>,
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum MetricsAgentInstallError {
    #[error("Could not read file permissions {0}")]
    PermissionError(std::io::Error),

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

    #[error("Checksum of download failed. Expected {DOWNLOAD_SHA} got {0}")]
    ChecksumFailed(String),
}

pub(crate) fn install_metrics_agent(
    context: &BuildContext<RubyBuildpack>,
    mut bullet: Print<SubBullet<Stdout>>,
) -> Result<(Print<SubBullet<Stdout>>, LayerEnv), libcnb::Error<RubyBuildpackError>> {
    let new_metadata = Metadata {
        download_url: Some(DOWNLOAD_URL.to_string()),
    };

    let layer = context.cached_layer(
        layer_name!("metrics_agent"),
        CachedLayerDefinition {
            build: true,
            launch: true,
            invalid_metadata_action: &|_| InvalidMetadataAction::DeleteLayer,
            restored_layer_action: &|old_metadata: &Metadata, _| {
                if old_metadata == &new_metadata {
                    RestoredLayerAction::KeepLayer
                } else {
                    RestoredLayerAction::DeleteLayer
                }
            },
        },
    )?;

    let bin_dir = layer.path().join("bin");

    match layer.state {
        LayerState::Restored { .. } => {
            bullet = bullet.sub_bullet("Using cached metrics agent");
        }
        LayerState::Empty { .. } => {
            bullet = bullet.sub_bullet(format!("Installing metrics agent from {DOWNLOAD_URL}"));

            let timer = bullet.start_timer("Downloading");
            install_agentmon(&bin_dir).map_err(RubyBuildpackError::MetricsAgentError)?;
            layer.write_metadata(new_metadata)?;
            bullet = timer.done();
        }
    };

    bullet = bullet.sub_bullet("Writing scripts");
    let execd = write_execd_script(&bin_dir.join("agentmon"), &layer.path())
        .map_err(RubyBuildpackError::MetricsAgentError)?;

    layer.write_exec_d_programs([("spawn_metrics_agent".to_string(), execd)])?;

    Ok((bullet, layer.read_env()?))
}

fn write_execd_script(
    agentmon: &Path,
    layer_path: &Path,
) -> Result<PathBuf, MetricsAgentInstallError> {
    let log = layer_path.join("output.log");
    let execd = layer_path.join("execd");
    let daemon = layer_path.join("launch_daemon");
    let run_loop = layer_path.join("agentmon_loop");

    // Ensure log file exists
    fs_err::write(&log, "").map_err(MetricsAgentInstallError::CouldNotWriteDestinationFile)?;

    // agentmon_loop boots agentmon continuously
    fs_err::copy(
        additional_buildpack_binary_path!("agentmon_loop"),
        &run_loop,
    )
    .map_err(MetricsAgentInstallError::CouldNotWriteDestinationFile)?;

    // The `launch_daemon` schedules `agentmon_loop` to run in the background
    fs_err::copy(additional_buildpack_binary_path!("launch_daemon"), &daemon)
        .map_err(MetricsAgentInstallError::CouldNotWriteDestinationFile)?;

    // The execd bash script will be run by CNB lifecycle, it runs the `launch_daemon`
    fs_err::write(
        &execd,
        format!(
            r#"#!/usr/bin/env bash

               {daemon} --log {log} --loop-path {run_loop} --agentmon {agentmon}
              "#,
            log = log.display(),
            daemon = daemon.display(),
            run_loop = run_loop.display(),
            agentmon = agentmon.display(),
        ),
    )
    .map_err(MetricsAgentInstallError::CouldNotCreateDestinationFile)?;
    chmod_plus_x(&execd).map_err(MetricsAgentInstallError::PermissionError)?;

    Ok(execd)
}

fn install_agentmon(dir: &Path) -> Result<PathBuf, MetricsAgentInstallError> {
    let agentmon = download_untar(DOWNLOAD_URL, dir).map(|()| dir.join("agentmon"))?;

    chmod_plus_x(&agentmon).map_err(MetricsAgentInstallError::PermissionError)?;
    Ok(agentmon)
}

fn download_untar(
    url: impl AsRef<str>,
    destination: &Path,
) -> Result<(), MetricsAgentInstallError> {
    let agentmon_tgz =
        NamedTempFile::new().map_err(MetricsAgentInstallError::CouldNotCreateDestinationFile)?;

    download(url, agentmon_tgz.path())?;

    sha256(agentmon_tgz.path())
        .map_err(MetricsAgentInstallError::CouldNotOpenFile)
        .and_then(|checksum| {
            if DOWNLOAD_SHA == checksum {
                Ok(())
            } else {
                Err(MetricsAgentInstallError::ChecksumFailed(checksum))
            }
        })?;

    untar(agentmon_tgz.path(), destination)?;

    Ok(())
}

fn untar(
    path: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<(), MetricsAgentInstallError> {
    let file =
        fs_err::File::open(path.as_ref()).map_err(MetricsAgentInstallError::CouldNotOpenFile)?;

    Archive::new(GzDecoder::new(file))
        .unpack(destination.as_ref())
        .map_err(MetricsAgentInstallError::CouldNotUnpack)
}

/// Sets file permissions on the given path to 7xx (similar to `chmod +x <path>`)
///
/// i.e. chmod +x will ensure that the first digit
/// of the file permission is 7 on unix so if you pass
/// in 0o455 it would be mutated to 0o755
fn chmod_plus_x(path: &Path) -> Result<(), std::io::Error> {
    let mut perms = fs_err::metadata(path)?.permissions();
    let mut mode = perms.mode();
    mode |= 0o700;
    perms.set_mode(mode);

    fs_err::set_permissions(path, perms)
}

fn download(
    uri: impl AsRef<str>,
    destination: impl AsRef<Path>,
) -> Result<(), MetricsAgentInstallError> {
    let mut response_reader = ureq::get(uri.as_ref())
        .call()
        .map_err(|err| MetricsAgentInstallError::RequestError(Box::new(err)))?
        .into_reader();

    let mut destination_file = fs_err::File::create(destination.as_ref())
        .map_err(MetricsAgentInstallError::CouldNotCreateDestinationFile)?;

    std::io::copy(&mut response_reader, &mut destination_file)
        .map_err(MetricsAgentInstallError::CouldNotWriteDestinationFile)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chmod() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.into_path().join("file");
        std::fs::write(&file, "lol").unwrap();

        let before = file.metadata().unwrap().permissions().mode();

        chmod_plus_x(&file).unwrap();

        let after = file.metadata().unwrap().permissions().mode();
        assert!(before != after);

        // Assert executable
        assert_eq!(after, after | 0o700);
    }
}

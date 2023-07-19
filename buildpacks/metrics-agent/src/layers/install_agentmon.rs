use crate::build_output;
use crate::{MetricsAgentBuildpack, MetricsAgentError};
use cached::proc_macro::cached;
use flate2::read::GzDecoder;
use libcnb::data::layer_content_metadata::LayerTypes;
use libcnb::layer::ExistingLayerStrategy;
use libcnb::{
    additional_buildpack_binary_path,
    generic::GenericMetadata,
    layer::{Layer, LayerResultBuilder},
};
use serde::{Deserialize, Serialize};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use tar::Archive;
use tempfile::NamedTempFile;
use url::Url;

#[derive(Debug)]
pub(crate) struct InstallAgentmon {
    pub(crate) section: build_output::Section,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct Metadata {
    download_url: Option<Url>,
}

// All cloneable subtypes to make cachable
#[derive(thiserror::Error, Debug, Clone)]
pub(crate) enum GetUrlError {
    #[error("Response successful, but body not in the form of a URL: {0}")]
    CannotConvertResponseToString(String),

    #[error("Cannot parse url: {0}")]
    UrlParseError(url::ParseError),

    // Boxed to prevent `large_enum_variant` errors since `ureq::Error` is massive.
    #[error("Network error while retrieving the url: {0}")]
    RequestError(String),
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum DownloadAgentmonError {
    #[error("Could not determine the url of the latest agentmont release.\n{0}")]
    CannotGetLatestUrl(GetUrlError),

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
}

impl Layer for InstallAgentmon {
    type Buildpack = MetricsAgentBuildpack;
    type Metadata = Metadata;

    fn types(&self) -> libcnb::data::layer_content_metadata::LayerTypes {
        LayerTypes {
            build: true,
            launch: true,
            cache: true,
        }
    }

    fn create(
        &self,
        _context: &libcnb::build::BuildContext<Self::Buildpack>,
        layer_path: &std::path::Path,
    ) -> Result<
        libcnb::layer::LayerResult<Self::Metadata>,
        <Self::Buildpack as libcnb::Buildpack>::Error,
    > {
        let destination_dir = layer_path.join("bin");
        let executable = destination_dir.join("agentmon");

        let mut timer = self.section.say_with_inline_timer("Downloading");

        let url = get_latest_url()
            .map_err(DownloadAgentmonError::CannotGetLatestUrl)
            .map_err(MetricsAgentError::DownloadAgentmonError)?;

        download_to_dir(&destination_dir, &url)
            .map_err(MetricsAgentError::DownloadAgentmonError)?;
        timer.done();

        self.section.say("Writing scripts");
        let execd = write_execd(&executable, layer_path)
            .map_err(MetricsAgentError::DownloadAgentmonError)?;

        LayerResultBuilder::new(Metadata {
            download_url: Some(url),
        })
        .exec_d_program("spawn agentmon", execd)
        .build()
    }

    fn existing_layer_strategy(
        &self,
        _context: &libcnb::build::BuildContext<Self::Buildpack>,
        layer_data: &libcnb::layer::LayerData<Self::Metadata>,
    ) -> Result<libcnb::layer::ExistingLayerStrategy, <Self::Buildpack as libcnb::Buildpack>::Error>
    {
        if let Some(old_url) = &layer_data.content_metadata.metadata.download_url {
            let url = get_latest_url()
                .map_err(DownloadAgentmonError::CannotGetLatestUrl)
                .map_err(MetricsAgentError::DownloadAgentmonError)?;

            if old_url == &url {
                self.section.say("Using cache");
                Ok(ExistingLayerStrategy::Keep)
            } else {
                let url = build_output::fmt::value(url);
                self.section
                    .say_with_details("Clearing cache", format!("Updated url {url}"));
                Ok(ExistingLayerStrategy::Recreate)
            }
        } else {
            self.section
                .say_with_details("Clearing cache", "No url found in metadata");

            Ok(ExistingLayerStrategy::Recreate)
        }
    }

    fn migrate_incompatible_metadata(
        &self,
        _context: &libcnb::build::BuildContext<Self::Buildpack>,
        _metadata: &GenericMetadata,
    ) -> Result<
        libcnb::layer::MetadataMigration<Self::Metadata>,
        <Self::Buildpack as libcnb::Buildpack>::Error,
    > {
        self.section
            .say_with_details("Clearing cache", "invalid metadata");

        Ok(libcnb::layer::MetadataMigration::RecreateLayer)
    }
}

fn write_execd(agentmon_path: &Path, layer_path: &Path) -> Result<PathBuf, DownloadAgentmonError> {
    let agentmon_path = agentmon_path
        .canonicalize()
        .map_err(DownloadAgentmonError::CouldNotOpenFile)?;
    let agentmon_path = agentmon_path.display();

    // This script boots and runs agentmon in a loop
    let background_script = {
        let script = layer_path.join("agentmon_loop");

        // Copy compiled binary from `bin/agentmon_loop.rs` to layer
        fs_err::copy(additional_buildpack_binary_path!("agentmon_loop"), &script)
            .map_err(DownloadAgentmonError::CouldNotWriteDestinationFile)?;

        script
            .canonicalize()
            .map_err(DownloadAgentmonError::CouldNotOpenFile)?
    };

    // We use the exec.d to boot a process. This script MUST exit though as otherwise
    // The container would never boot. To handle this we intentionally leak a process
    let execd_script = {
        let script = layer_path.join("agentmon_exec.d");

        let background_script = background_script.display();
        write_bash_script(
            &script,
            format!(r#"start-stop-daemon --start --background --exec "{background_script}" -- --path {agentmon_path}"#),
        )
        .map_err(DownloadAgentmonError::CouldNotWriteDestinationFile)?;

        script
    };

    Ok(execd_script)
}

#[cached]
fn get_latest_url() -> Result<Url, GetUrlError> {
    // This file on S3 stores a raw string that holds the URL to the latest agentmon release
    // It's not a redirect to the latest file, it's a string body that contains a URL.
    let base = Url::parse("https://agentmon-releases.s3.amazonaws.com/latest")
        .expect("Internal error: Bad url");

    let body = ureq::get(base.as_ref())
        .call()
        .map_err(|err| GetUrlError::RequestError(err.to_string()))?
        .into_string()
        .map_err(|error| GetUrlError::CannotConvertResponseToString(error.to_string()))?;

    Url::parse(body.as_str().trim()).map_err(GetUrlError::UrlParseError)
}

fn download_to_dir(destination: &Path, url: &Url) -> Result<(), DownloadAgentmonError> {
    let agentmon_tgz =
        NamedTempFile::new().map_err(DownloadAgentmonError::CouldNotCreateDestinationFile)?;

    download(url.as_ref(), agentmon_tgz.path())?;

    untar(agentmon_tgz.path(), destination)?;

    chmod_plus_x(&destination.join("agentmon")).map_err(DownloadAgentmonError::PermissionError)?;

    Ok(())
}

pub(crate) fn untar(
    path: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<(), DownloadAgentmonError> {
    let file =
        fs_err::File::open(path.as_ref()).map_err(DownloadAgentmonError::CouldNotOpenFile)?;

    Archive::new(GzDecoder::new(file))
        .unpack(destination.as_ref())
        .map_err(DownloadAgentmonError::CouldNotUnpack)
}

/// Sets file permissions on the given path to 7xx (similar to `chmod +x <path>`)
pub fn chmod_plus_x(path: &Path) -> Result<(), std::io::Error> {
    let mut perms = fs_err::metadata(path)?.permissions();
    let mut mode = perms.mode();
    octal_executable_permission(&mut mode);
    perms.set_mode(mode);

    fs_err::set_permissions(path, perms)
}

/// Write a script to the target path while adding a bash shebang line and setting execution permissions
fn write_bash_script(path: &Path, script: impl AsRef<str>) -> std::io::Result<()> {
    let script = script.as_ref();
    fs_err::write(path, format!("#!/usr/bin/env bash\n\n{script}"))?;
    chmod_plus_x(path)?;

    Ok(())
}

/// Ensures the provided octal number's executable
/// bit is enabled.
///
/// i.e. chmod +x will ensure that the first digit
/// of the file permission is 7 on unix so if you pass
/// in 0o455 it would be mutated to 0o755
fn octal_executable_permission(mode: &mut u32) {
    *mode |= 0o700;
}

pub(crate) fn download(
    uri: impl AsRef<str>,
    destination: impl AsRef<Path>,
) -> Result<(), DownloadAgentmonError> {
    let mut response_reader = ureq::get(uri.as_ref())
        .call()
        .map_err(|err| DownloadAgentmonError::RequestError(Box::new(err)))?
        .into_reader();

    let mut destination_file = fs_err::File::create(destination.as_ref())
        .map_err(DownloadAgentmonError::CouldNotCreateDestinationFile)?;

    std::io::copy(&mut response_reader, &mut destination_file)
        .map_err(DownloadAgentmonError::CouldNotWriteDestinationFile)?;

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
    }

    #[test]
    fn test_executable_logic() {
        // Sets executable bit
        let mut mode = 0o455;
        octal_executable_permission(&mut mode);
        assert_eq!(0o755, mode);

        // Does not affect already executable
        let mut mode = 0o745;
        octal_executable_permission(&mut mode);
        assert_eq!(0o745, mode);
    }
}

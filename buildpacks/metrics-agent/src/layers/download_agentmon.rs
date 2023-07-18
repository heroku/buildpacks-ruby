use crate::{MetricsAgentBuildpack, MetricsAgentError};
use libcnb::{
    generic::GenericMetadata,
    layer::{Layer, LayerResultBuilder},
};
use std::os::unix::fs::PermissionsExt;
use url::Url;

use crate::build_output;
use flate2::read::GzDecoder;
use libcnb::data::layer_content_metadata::LayerTypes;
use std::path::Path;
use tar::Archive;
use tempfile::NamedTempFile;
// use url::Url;

#[derive(Debug)]
pub(crate) struct DownloadAgentmon {
    pub(crate) section: build_output::Section,
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum DownloadAgentmonError {
    #[error("Could not read file permissions {0}")]
    PermissionError(std::io::Error),

    #[error("Could not parse url {0}")]
    UrlParseError(url::ParseError),

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

impl Layer for DownloadAgentmon {
    type Buildpack = MetricsAgentBuildpack;
    type Metadata = GenericMetadata;

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
        let mut timer = self.section.say_with_inline_timer("Installing");

        let agentmon_tgz = NamedTempFile::new()
            .map_err(DownloadAgentmonError::CouldNotCreateDestinationFile)
            .map_err(MetricsAgentError::DownloadAgentmonError)?;

        let url = Url::parse("https://agentmon-releases.s3.amazonaws.com/latest")
            .map_err(DownloadAgentmonError::UrlParseError)
            .map_err(MetricsAgentError::DownloadAgentmonError)?;

        download(url.as_ref(), agentmon_tgz.path())
            .map_err(MetricsAgentError::DownloadAgentmonError)?;

        let destination = layer_path.join("bin");
        untar(agentmon_tgz.path(), &destination)
            .map_err(MetricsAgentError::DownloadAgentmonError)?;

        chmod_plus_x(&destination.join("agentmon"))
            .map_err(DownloadAgentmonError::PermissionError)
            .map_err(MetricsAgentError::DownloadAgentmonError)?;

        timer.done();

        LayerResultBuilder::new(GenericMetadata::default()).build()
    }

    fn existing_layer_strategy(
        &self,
        _context: &libcnb::build::BuildContext<Self::Buildpack>,
        _layer_data: &libcnb::layer::LayerData<Self::Metadata>,
    ) -> Result<libcnb::layer::ExistingLayerStrategy, <Self::Buildpack as libcnb::Buildpack>::Error>
    {
        // TODO caching logic
        //
        // The classic buildpack actually downloads this binary on dyno boot every time
        // We could use content headers to check if it needs to be re-downloaded.
        Ok(libcnb::layer::ExistingLayerStrategy::Recreate)
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

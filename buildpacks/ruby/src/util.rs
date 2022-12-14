use flate2::read::GzDecoder;
use std::fs;
use std::io;
use std::path::Path;
use tar::Archive;

pub(crate) fn download(
    uri: impl AsRef<str>,
    destination: impl AsRef<Path>,
) -> Result<(), DownloadError> {
    let mut response_reader = ureq::get(uri.as_ref())
        .call()
        .map_err(|err| DownloadError::RequestError(Box::new(err)))?
        .into_reader();

    let mut destination_file = fs::File::create(destination.as_ref())
        .map_err(DownloadError::CouldNotCreateDestinationFile)?;

    io::copy(&mut response_reader, &mut destination_file)
        .map_err(DownloadError::CouldNotWriteDestinationFile)?;

    Ok(())
}

#[derive(thiserror::Error, Debug)]
pub enum DownloadError {
    // Boxed to prevent `large_enum_variant` errors since `ureq::Error` is massive.
    #[error("Download error: {0}")]
    RequestError(Box<ureq::Error>),
    #[error("Could not create file: {0}")]
    CouldNotCreateDestinationFile(std::io::Error),
    #[error("Could not write file: {0}")]
    CouldNotWriteDestinationFile(std::io::Error),
}

pub(crate) fn untar(
    path: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<(), UntarError> {
    let file = fs::File::open(path.as_ref()).map_err(UntarError::CouldNotOpenFile)?;

    Archive::new(GzDecoder::new(file))
        .unpack(destination.as_ref())
        .map_err(UntarError::CouldNotUnpack)
}

#[derive(thiserror::Error, Debug)]
pub enum UntarError {
    #[error("Could not open file: {0}")]
    CouldNotOpenFile(std::io::Error),
    #[error("Could not untar: {0}")]
    CouldNotUnpack(std::io::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum UrlError {
    #[error("Could not parse url {0}")]
    UrlParseError(url::ParseError),

    #[error("Invalid base url {0}")]
    InvalidBaseUrl(String),
}

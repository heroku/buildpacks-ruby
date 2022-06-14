use flate2::read::GzDecoder;
use libcnb::Env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};
use tar::Archive;

pub fn command_to_str_with_env_keys(command: &Command, env: &Env, keys: Vec<OsString>) -> String {
    format!(
        "{} {} {}",
        keys.iter()
            .map(|key| format!(
                "{}={:?} ",
                key.to_string_lossy(),
                env.get(key).unwrap_or(OsString::from("")).to_string_lossy()
            ))
            .collect::<String>()
            .trim(),
        command.get_program().to_string_lossy(),
        command
            .get_args()
            .map(std::ffi::OsStr::to_string_lossy)
            .collect::<Vec<_>>()
            .join(" ")
    )
}

pub fn download(uri: impl AsRef<str>, destination: impl AsRef<Path>) -> Result<(), DownloadError> {
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

pub fn untar(path: impl AsRef<Path>, destination: impl AsRef<Path>) -> Result<(), UntarError> {
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

/// Helper to run very simple commands where we just need to handle IO errors and non-zero exit
/// codes. Not very useful in complex scenarios, but can cut down the amount of code in simple
/// cases.
pub fn run_simple_command<E, F: FnOnce(std::io::Error) -> E, F2: FnOnce(ExitStatus) -> E>(
    command: &mut Command,
    io_error_fn: F,
    exit_status_fn: F2,
) -> Result<ExitStatus, E> {
    command
        .spawn()
        .and_then(|mut child| child.wait())
        .map_err(io_error_fn)
        .and_then(|exit_status| {
            if exit_status.success() {
                Ok(exit_status)
            } else {
                Err(exit_status_fn(exit_status))
            }
        })
}

#[derive(thiserror::Error, Debug)]
pub enum UrlError {
    #[error("Could not parse url {0}")]
    UrlParseError(url::ParseError),

    #[error("Invalid base url {0}")]
    InvalidBaseUrl(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_to_str_with_env_keys_one_exists() {
        let mut env = Env::new();
        env.insert("TRANSPORT", "perihelion");

        let mut command = Command::new("bundle");
        command.args(&["install", "--path", "lol"]);

        let out = command_to_str_with_env_keys(&command, &env, vec![OsString::from("TRANSPORT")]);
        assert_eq!("TRANSPORT=\"perihelion\" bundle install --path lol", out);
    }

    #[test]
    fn test_command_to_str_with_env_keys_one_missing() {
        let env = Env::new();

        let mut command = Command::new("bundle");
        command.args(&["install", "--path", "lol"]);

        let out = command_to_str_with_env_keys(&command, &env, vec![OsString::from("TRANSPORT")]);
        assert_eq!("TRANSPORT=\"\" bundle install --path lol", out);
    }

    #[test]
    fn test_command_to_str_with_env_keys_two_exist() {
        let mut env = Env::new();
        env.insert("TRANSPORT", "perihelion");
        env.insert("SHOW", "the rise and fall of sanctuary moon");

        let mut command = Command::new("bundle");
        command.args(&["install", "--path", "lol"]);

        let out = command_to_str_with_env_keys(
            &command,
            &env,
            vec![OsString::from("TRANSPORT"), OsString::from("SHOW")],
        );
        assert_eq!("TRANSPORT=\"perihelion\" SHOW=\"the rise and fall of sanctuary moon\" bundle install --path lol", out);
    }

    #[test]
    fn test_command_to_str_with_env_keys_two_with_one_empty() {
        let mut env = Env::new();
        env.insert("TRANSPORT", "perihelion");

        let mut command = Command::new("bundle");
        command.args(&["install", "--path", "lol"]);

        let out = command_to_str_with_env_keys(
            &command,
            &env,
            vec![OsString::from("TRANSPORT"), OsString::from("SHOW")],
        );
        assert_eq!(
            "TRANSPORT=\"perihelion\" SHOW=\"\" bundle install --path lol",
            out
        );
    }
}

use crate::layers::{BundlerLayer, RubyLayer};
use libcnb::build::{BuildContext, BuildResult, BuildResultBuilder};
use libcnb::data::launch::{Launch, Process};
use libcnb::data::{layer_name, process_type};
use libcnb::detect::{DetectContext, DetectResult, DetectResultBuilder};
use libcnb::generic::GenericPlatform;
use libcnb::layer_env::Scope;
use libcnb::{buildpack_main, Buildpack, Env};

use crate::util::{DownloadError, UntarError, UrlError};
use serde::Deserialize;
use std::process::ExitStatus;

mod layers;
mod util;

pub struct RubyBuildpack;
use core::str::FromStr;

#[derive(Debug)]
struct BundleInfo {
    bundler_version: BundlerVersion,
    ruby_version: RubyVersion,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RubyVersion {
    Explicit(String),
    Default,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BundlerVersion {
    Explicit(String),
    Default,
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum BundleInfoError {
    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),
}

use regex::Regex;
impl FromStr for BundleInfo {
    type Err = BundleInfoError;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let bundled_with_re = Regex::new("BUNDLED WITH\\s   (\\d+\\.\\d+\\.\\d+)")
            .map_err(BundleInfoError::RegexError)?;
        let ruby_version_re = Regex::new("RUBY VERSION\\s   ruby (\\d+\\.\\d+\\.\\d+)")
            .map_err(BundleInfoError::RegexError)?;

        let bundler_version = match bundled_with_re.captures(string).and_then(|c| c.get(1)) {
            Some(result) => BundlerVersion::Explicit(result.as_str().to_string()),
            None => BundlerVersion::Default,
        };

        let ruby_version = match ruby_version_re.captures(string).and_then(|c| c.get(1)) {
            Some(result) => RubyVersion::Explicit(result.as_str().to_string()),
            None => RubyVersion::Default,
        };

        Ok(Self {
            bundler_version,
            ruby_version,
        })
    }
}

impl Buildpack for RubyBuildpack {
    type Platform = GenericPlatform;
    type Metadata = RubyBuildpackMetadata;
    type Error = RubyBuildpackError;

    fn detect(&self, context: DetectContext<Self>) -> libcnb::Result<DetectResult, Self::Error> {
        if context.app_dir.join("Gemfile.lock").exists() {
            DetectResultBuilder::pass().build()
        } else {
            DetectResultBuilder::fail().build()
        }
    }

    fn build(&self, context: BuildContext<Self>) -> libcnb::Result<BuildResult, Self::Error> {
        println!("---> Ruby Buildpack");

        let gemfile_lock = std::fs::read_to_string(context.app_dir.join("Gemfile.lock")).unwrap();
        let bundle_info = BundleInfo::from_str(&gemfile_lock)
            .map_err(RubyBuildpackError::GemfileLockParsingError)?;

        let ruby_layer = context //
            .handle_layer(
                layer_name!("ruby"),
                RubyLayer {
                    version: bundle_info.ruby_version,
                },
            );
        let ruby_layer = ruby_layer?;

        context.handle_layer(
            layer_name!("bundler"),
            BundlerLayer {
                ruby_env: ruby_layer.env.apply(Scope::Build, &Env::new()),
                version: bundle_info.bundler_version,
            },
        )?;

        BuildResultBuilder::new()
            .launch(
                Launch::new()
                    .process(Process::new(
                        process_type!("web"),
                        "bundle",
                        vec!["exec", "ruby", "app.rb"],
                        false,
                        true,
                    ))
                    .process(Process::new(
                        process_type!("worker"),
                        "bundle",
                        vec!["exec", "ruby", "worker.rb"],
                        false,
                        false,
                    )),
            )
            .build()
    }
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct RubyBuildpackMetadata {
    pub ruby_url: String,
}

#[derive(thiserror::Error, Debug)]
pub enum RubyBuildpackError {
    #[error("Cannot download: {0}")]
    RubyDownloadError(DownloadError),
    #[error("Cannot untar: {0}")]
    RubyUntarError(UntarError),
    #[error("Cannot create temporary file: {0}")]
    CouldNotCreateTemporaryFile(std::io::Error),
    #[error("Cannot generate checksum: {0}")]
    CouldNotGenerateChecksum(std::io::Error),
    #[error("Cannot install bundler: {0}")]
    GemInstallBundlerCommandError(std::io::Error),
    #[error("Bundler gem install exit: {0}")]
    GemInstallBundlerUnexpectedExitStatus(ExitStatus),
    #[error("Bundle install errored: {0}")]
    BundleInstallCommandError(std::io::Error),
    #[error("Bundle install exit: {0}")]
    BundleInstallUnexpectedExitStatus(ExitStatus),
    #[error("Bundle config error: {0}")]
    BundleConfigCommandError(std::io::Error),
    #[error("Bundle config exit: {0}")]
    BundleConfigUnexpectedExitStatus(ExitStatus),

    #[error("Url error: {0}")]
    UrlParseError(UrlError),

    #[error("Error evaluating Gemfile.lock: {0}")]
    GemfileLockParsingError(BundleInfoError),
}
impl From<RubyBuildpackError> for libcnb::Error<RubyBuildpackError> {
    fn from(error: RubyBuildpackError) -> Self {
        Self::BuildpackError(error)
    }
}

buildpack_main!(RubyBuildpack);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gemfile_lock() {
        let info = BundleInfo::from_str(
            r#"
GEM
  remote: https://rubygems.org/
  specs:
    mini_histogram (0.3.1)

PLATFORMS
  ruby
  x86_64-darwin-20
  x86_64-linux

DEPENDENCIES
  mini_histogram

RUBY VERSION
   ruby 3.1.0p-1

BUNDLED WITH
   2.3.4
"#,
        )
        .unwrap();

        assert_eq!(
            info.bundler_version,
            BundlerVersion::Explicit("2.3.4".to_string())
        );
        assert_eq!(
            info.ruby_version,
            RubyVersion::Explicit("3.1.0".to_string())
        );
    }

    #[test]
    fn test_default_versions() {
        let info = BundleInfo::from_str("").unwrap();
        assert_eq!(info.bundler_version, BundlerVersion::Default);
        assert_eq!(info.ruby_version, RubyVersion::Default);
    }
}

use core::str::FromStr;
use regex::Regex;

/*

# Parse and store contents of Gemfile.lock

Before installing bundler or Ruby versions we first need information about the application.
This struct holds both of these values. When no value is present it will return a `Default`
enum.

*/
#[derive(Debug)]
pub struct GemfileLock {
    pub bundler_version: BundlerVersion,
    pub ruby_version: RubyVersion,
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
pub enum GemfileLockError {
    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),
}

impl FromStr for GemfileLock {
    type Err = GemfileLockError;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let bundled_with_re = Regex::new("BUNDLED WITH\\s   (\\d+\\.\\d+\\.\\d+)")
            .map_err(GemfileLockError::RegexError)?;
        let ruby_version_re = Regex::new("RUBY VERSION\\s   ruby (\\d+\\.\\d+\\.\\d+)")
            .map_err(GemfileLockError::RegexError)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gemfile_lock() {
        let info = GemfileLock::from_str(
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
        let info = GemfileLock::from_str("").unwrap();
        assert_eq!(info.bundler_version, BundlerVersion::Default);
        assert_eq!(info.ruby_version, RubyVersion::Default);
    }
}

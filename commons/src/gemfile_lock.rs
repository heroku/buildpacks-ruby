use core::str::FromStr;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// # Parse and store contents of Gemfile.lock
///
/// Before installing bundler or Ruby versions we first need information about the application.
/// This struct holds both of these values. When no value is present it will return a `Default`
/// enum.
/// ```rust
/// use core::str::FromStr;
/// use commons::gemfile_lock::BundlerVersion;
/// use commons::gemfile_lock::GemfileLock;
///
/// let contents = r#"
/// GEM
///   remote: https://rubygems.org/
///   specs:
///     mini_histogram (0.3.1)
///
/// PLATFORMS
///   ruby
///   x86_64-darwin-20
///   x86_64-linux
///
/// DEPENDENCIES
///   mini_histogram
///
/// RUBY VERSION
///    ruby 3.1.0p-1
///
/// BUNDLED WITH
///    2.3.4
/// "#;
/// let info = GemfileLock::from_str(contents).unwrap();
///
/// assert_eq!(
///     info.bundler_version,
///     BundlerVersion::Explicit("2.3.4".to_string())
/// );
/// ```
#[derive(Debug)]
pub struct GemfileLock {
    pub bundler_version: BundlerVersion,
    pub ruby_version: RubyVersion,
}

impl GemfileLock {
    #[must_use]
    pub fn ruby_source(&self) -> String {
        match self.ruby_version {
            RubyVersion::Explicit(_) => String::from("Gemfile.lock"),
            RubyVersion::Default => String::from("default"),
        }
    }

    #[must_use]
    pub fn bundler_source(&self) -> String {
        match self.bundler_version {
            BundlerVersion::Explicit(_) => String::from("Gemfile.lock"),
            BundlerVersion::Default => String::from("default"),
        }
    }

    #[must_use]
    pub fn resolve_ruby(&self, default: &str) -> ResolvedRubyVersion {
        match &self.ruby_version {
            RubyVersion::Explicit(version) => ResolvedRubyVersion(version.to_string()),
            RubyVersion::Default => ResolvedRubyVersion(default.to_string()),
        }
    }

    #[must_use]
    pub fn resolve_bundler(&self, default: &str) -> ResolvedBundlerVersion {
        match &self.bundler_version {
            BundlerVersion::Explicit(version) => ResolvedBundlerVersion(version.to_string()),
            BundlerVersion::Default => ResolvedBundlerVersion(default.to_string()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(deny_unknown_fields)]
pub struct ResolvedRubyVersion(pub String);

impl Display for ResolvedRubyVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(deny_unknown_fields)]
pub struct ResolvedBundlerVersion(pub String);

impl Display for ResolvedBundlerVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum RubyVersion {
    Explicit(String),
    Default,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BundlerVersion {
    Explicit(String),
    Default,
}

impl FromStr for GemfileLock {
    type Err = std::convert::Infallible;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let bundled_with_re =
            Regex::new("BUNDLED WITH\\s   (\\d+\\.\\d+\\.\\d+)").expect("Clippy checked");
        let main_ruby_version_re =
            Regex::new("RUBY VERSION\\s   ruby (\\d+\\.\\d+\\.\\d+((-|\\.)\\S*\\d+)?)")
                .expect("Clippy checked");
        let jruby_version_re = Regex::new("\\(jruby ((\\d+|\\.)+)\\)").expect("Clippy checked");

        let bundler_version = match bundled_with_re.captures(string).and_then(|c| c.get(1)) {
            Some(result) => BundlerVersion::Explicit(result.as_str().to_string()),
            None => BundlerVersion::Default,
        };

        let ruby_version = match main_ruby_version_re.captures(string).and_then(|c| c.get(1)) {
            Some(main_ruby_match) => match jruby_version_re.captures(string).and_then(|c| c.get(1))
            {
                Some(jruby_match) => RubyVersion::Explicit(format!(
                    "{}-jruby-{}",
                    main_ruby_match.as_str(),
                    jruby_match.as_str()
                )),
                None => RubyVersion::Explicit(main_ruby_match.as_str().to_string()),
            },
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
    fn test_does_not_capture_patch_version() {
        let info = GemfileLock::from_str(
            r"
RUBY VERSION
   ruby 3.3.5p100

BUNDLED WITH
   2.3.4
",
        )
        .unwrap();

        assert_eq!(
            info.bundler_version,
            BundlerVersion::Explicit("2.3.4".to_string())
        );
        assert_eq!(
            info.ruby_version,
            RubyVersion::Explicit("3.3.5".to_string())
        );
    }

    #[test]
    fn test_rc_dot_version() {
        let info = GemfileLock::from_str(
            r"
RUBY VERSION
   ruby 3.4.0.rc1

BUNDLED WITH
   2.3.4
",
        )
        .unwrap();

        assert_eq!(
            info.bundler_version,
            BundlerVersion::Explicit("2.3.4".to_string())
        );
        assert_eq!(
            info.ruby_version,
            RubyVersion::Explicit("3.4.0.rc1".to_string())
        );
    }

    #[test]
    fn test_preview_version() {
        let info = GemfileLock::from_str(
            r"
RUBY VERSION
   ruby 3.4.0.preview2

BUNDLED WITH
   2.3.4
",
        )
        .unwrap();

        assert_eq!(
            info.bundler_version,
            BundlerVersion::Explicit("2.3.4".to_string())
        );
        assert_eq!(
            info.ruby_version,
            RubyVersion::Explicit("3.4.0.preview2".to_string())
        );
    }

    #[test]
    fn test_parse_gemfile_lock() {
        let info = GemfileLock::from_str(
            r"
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
",
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

    #[test]
    fn test_jruby() {
        let info = GemfileLock::from_str(
            r"
GEM
  remote: https://rubygems.org/
  specs:
PLATFORMS
  java
RUBY VERSION
   ruby 2.5.7p001 (jruby 9.2.13.0)
DEPENDENCIES
",
        )
        .unwrap();

        assert_eq!(
            info.ruby_version,
            RubyVersion::Explicit(String::from("2.5.7-jruby-9.2.13.0"))
        );
    }
}

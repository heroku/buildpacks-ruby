use regex::Regex;

lazy_static! {
    static ref RUBY_VERSION_RE: Regex = Regex::new(r#"RUBY VERSION\s   ruby (\d+\.\d+\.\d+)"#)
        .expect("Internal error: Bad regex for ruby version");
    static ref JRUBY_ENGINE_RE: Regex =
        Regex::new(r#"\(jruby ((\d+|\.)+)\)"#).expect("Internal error: Bad regex for jruby engine");
    static ref BUNDLED_WITH_RE: Regex = Regex::new(r#"BUNDLED WITH\s   (\d+\.\d+\.\d+)"#)
        .expect("Internal error: Bad regex for bundled with");
}

/// On MRI this is the same as the Ruby version i.e. 3.1.2 would be Some("3.1.2")
/// On engines (like jruby) this will be the specification that the engine implements
/// i.e. jruby '9.3.6.0' implements ruby spec '2.6.8'
fn ruby_version(lockfile: &str) -> Option<String> {
    RUBY_VERSION_RE
        .captures(lockfile)
        .and_then(|capture| capture.get(1))
        .map(|m| m.as_str().to_string())
}

/// Holds raw data about a Ruby engine version from
/// the Gemfile.lock
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EngineVersion {
    /// JRuby specialized engine version found
    /// string contains raw String value from Gemfile.lock
    JRuby(String),
}

/// Returns `EngineVersion` variant along with it's version if one is found.
/// Otherwise returns `EngineVersion::None`.
///
/// Currently supported engines:
/// - jruby
fn ruby_engine_version(lockfile: &str) -> Option<EngineVersion> {
    JRUBY_ENGINE_RE
        .captures(lockfile)
        .and_then(|capture| capture.get(1))
        .map(|m| m.as_str())
        .map(|engine_version| EngineVersion::JRuby(engine_version.to_string()))
}

/// Returns raw values from Gemfile.lock
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LockfileRuby {
    /// MRI version found
    Version(String),
    /// Engine version found that implements the
    /// specified Ruby version spec
    VersionWithEngine(String, EngineVersion),
    /// Engine version found, but missing the
    /// required Ruby version spec
    EngineMissingRuby(EngineVersion),
    /// No ruby version information found in the
    /// Gemfile.lock
    None,
}

/// Parses the Gemfile.lock contents to return information about
/// ruby versions.
#[must_use]
pub(crate) fn ruby_info(lockfile: &str) -> LockfileRuby {
    match (ruby_version(lockfile), ruby_engine_version(lockfile)) {
        (None, None) => LockfileRuby::None,
        (None, Some(engine)) => LockfileRuby::EngineMissingRuby(engine),
        (Some(version), None) => LockfileRuby::Version(version),
        (Some(version), Some(engine)) => LockfileRuby::VersionWithEngine(version, engine),
    }
}

/// Parses the Gemfile.lock contents to return information about
/// what exact bundler version is needed (if any).
#[must_use]
pub(crate) fn bundled_with(lockfile: &str) -> Option<String> {
    BUNDLED_WITH_RE
        .captures(lockfile)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gemfile_lock() {
        let lockfile = r#"
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
"#;

        assert_eq!(bundled_with(lockfile), Some("2.3.4".to_string()));
        assert_eq!(ruby_version(lockfile), Some(String::from("3.1.0")));
    }

    #[test]
    fn test_no_bundled_with() {
        assert_eq!(bundled_with(""), None);
    }

    #[test]
    fn test_jruby() {
        let lockfile = r#"
GEM
  remote: https://rubygems.org/
  specs:
PLATFORMS
  java
RUBY VERSION
   ruby 2.5.7p001 (jruby 9.2.13.0)
DEPENDENCIES
"#;

        assert_eq!(ruby_version(lockfile), Some(String::from("2.5.7")));

        assert_eq!(
            ruby_engine_version(lockfile),
            Some(EngineVersion::JRuby(String::from("9.2.13.0")))
        );
    }
}

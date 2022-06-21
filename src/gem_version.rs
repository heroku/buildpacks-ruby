use core::str::FromStr;
use regex::Regex;

/// # Struct to hold semver-ish versions for comparison
///
/// Based off of Ruby's `Gem::Version` logic:
///
/// - <https://github.com/rubygems/rubygems/blob/ecc8e895b69063562b9bf749b353948e051e4171/lib/rubygems/version.rb>
/// - <https://github.com/rubygems/rubygems/blob/ecc8e895b69063562b9bf749b353948e051e4171/test/rubygems/test_gem_version.rb>
///
/// Example:
///
/// ```rust
/// use crate::gem_version::GemVersion;
///
/// let version = GemVersion::from_str("1.0.0");
/// assert!(version < GemVersion::from_str("2.0.0"));
/// ```
#[derive(Debug, Eq, PartialEq, Default, PartialOrd, Ord)]
pub struct GemVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub extra: Option<String>,
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum GemVersionError {
    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),
}

impl FromStr for GemVersion {
    type Err = GemVersionError;
    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let digits_re = Regex::new("^\\d+$").map_err(GemVersionError::RegexError)?;
        let parts = string.split('.').map(str::trim);

        let mut major_minor_patch = parts
            .clone()
            .take_while(|part| digits_re.is_match(part))
            .map(|p| p.parse().unwrap_or(0));

        let major = major_minor_patch.next();
        let minor = major_minor_patch.next();
        let patch = major_minor_patch.next();

        let leftovers = major_minor_patch.map(|i| i.to_string());

        let extra = leftovers
            .chain(
                parts
                    .clone()
                    .skip_while(|part| digits_re.is_match(part))
                    .map(std::string::ToString::to_string),
            )
            .collect::<Vec<String>>()
            .join(".");

        Ok(GemVersion {
            major: major.unwrap_or(0),
            minor: minor.unwrap_or(0),
            patch: patch.unwrap_or(0),
            extra: if extra.is_empty() { None } else { Some(extra) },
        })
    }
}

// impl PartialOrd for GemVersion {
//     fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
//         let this = self.version.split(".");
//         let that = other.version.split(".");
//         None
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gem_version_equal_comparison() {
        assert_eq!(GemVersion::from_str("1.2"), GemVersion::from_str("1.2"));
        assert_eq!(GemVersion::from_str("1.2"), GemVersion::from_str("1.2.0"));
        assert_ne!(GemVersion::from_str("1.2"), GemVersion::from_str("1.3"));
        assert_ne!(
            GemVersion::from_str("1.2.b1"),
            GemVersion::from_str("1.2.b.1")
        );
    }

    #[test]
    fn test_handles_whitespace() {
        vec!["1.0", "1.0 ", " 1.0 ", "1.0\n", "\n1.0\n", "1.0"]
            .iter()
            .map(|str| {
                assert_eq!(
                    GemVersion::from_str(str),
                    GemVersion::from_str("1.0"),
                    "Expected {} to eq 1.0 but it did not",
                    str
                );
            })
            .for_each(drop);
    }

    #[test]
    fn test_handles_empty_versions() {
        vec!["", "   ", " "]
            .iter()
            .map(|str| {
                assert_eq!(
                    GemVersion::from_str(str),
                    GemVersion::from_str("0"),
                    "Expected {} to eq 0 but it did not",
                    str
                );
            })
            .for_each(drop);
    }

    #[test]
    fn test_gt_lt() {
        assert!(GemVersion::from_str("1.0").unwrap() > GemVersion::from_str("1.0.a").unwrap());
        assert!(GemVersion::from_str("1.8.2").unwrap() > GemVersion::from_str("0.0.0").unwrap());
        assert!(GemVersion::from_str("1.8.2").unwrap() > GemVersion::from_str("1.8.2.a").unwrap());
        assert!(
            GemVersion::from_str("1.8.2.b").unwrap() > GemVersion::from_str("1.8.2.a").unwrap()
        );

        assert!(GemVersion::from_str("5.x").unwrap() > GemVersion::from_str("5.0.0.rc2").unwrap());

        // Eq
        assert_eq!(
            GemVersion::from_str("0.beta.1").unwrap(),
            GemVersion::from_str("0.0.beta.1").unwrap()
        );

        assert_eq!(
            GemVersion::from_str("1.8.2.a10").unwrap() > GemVersion::from_str("1.8.2.a9").unwrap()
        );

        assert_eq!(
            GemVersion::from_str("1.9.3").unwrap() > GemVersion::from_str("1.9.2.99").unwrap()
        );

        // Less than
        assert!(
            GemVersion::from_str("1.8.2.a").unwrap() < GemVersion::from_str("1.8.2.b").unwrap()
        );

        assert!(
            GemVersion::from_str("0.0.beta").unwrap() < GemVersion::from_str("0.0.beta.1").unwrap()
        );

        assert!(
            GemVersion::from_str("0.0.beta").unwrap() < GemVersion::from_str("0.beta.1").unwrap()
        );
        assert!(GemVersion::from_str("5.a").unwrap() < GemVersion::from_str("5.0.0.rc2").unwrap());
        assert!(GemVersion::from_str("1.9.3").unwrap() < GemVersion::from_str("1.9.3.1").unwrap());
    }

    #[test]
    fn test_parsing_gem_version() {
        let version = GemVersion::from_str("2.0.0").unwrap();
        assert_eq!(version.major, 2);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
        assert_eq!(version.extra, None);

        let version = GemVersion::from_str("5.3").unwrap();
        assert_eq!(version.major, 5);
        assert_eq!(version.minor, 3);
        assert_eq!(version.patch, 0);
        assert_eq!(version.extra, None);

        let version = GemVersion::from_str("6").unwrap();
        assert_eq!(version.major, 6);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
        assert_eq!(version.extra, None);

        let version = GemVersion::from_str("5.2.4.a").unwrap();
        assert_eq!(version.major, 5);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 4);
        assert_eq!(version.extra, Some("a".to_string()));

        let version = GemVersion::from_str("2.9.b").unwrap();
        assert_eq!(version.major, 2);
        assert_eq!(version.minor, 9);
        assert_eq!(version.patch, 0);
        assert_eq!(version.extra, Some("b".to_string()));

        let version = GemVersion::from_str("2b.9.b").unwrap();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
        assert_eq!(version.extra, Some("2b.9.b".to_string()));

        let version = GemVersion::from_str("1.2.pre.1").unwrap();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 0);
        assert_eq!(version.extra, Some("pre.1".to_string()));

        let version = GemVersion::from_str("1..2").unwrap();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
        assert_eq!(version.extra, Some(".2".to_string()));

        let version = GemVersion::from_str("").unwrap();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
        assert_eq!(version.extra, None);

        let version = GemVersion::from_str("0").unwrap();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
        assert_eq!(version.extra, None);

        let version = GemVersion::from_str("1.0.10.20").unwrap();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 10);
        assert_eq!(version.extra, Some("20".to_string()));
    }
}

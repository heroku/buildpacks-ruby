use core::str::FromStr;
use regex::Regex;
use std::collections::HashMap;

#[derive(Debug)]
pub struct GemList {
    pub gems: HashMap<String, String>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct GemVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub extra: Option<String>,
}

impl FromStr for GemVersion {
    type Err = GemListError;
    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let digits_re = Regex::new("^\\d+$").map_err(GemListError::RegexError)?;
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

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum GemListError {
    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),
}

impl GemList {
    fn has(&self, str: &str) -> bool {
        self.gems.get(str).is_some()
    }

    fn version_for(&self, str: &str) -> Option<&String> {
        self.gems.get(str)
    }
}

impl FromStr for GemList {
    type Err = GemListError;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        // https://regex101.com/r/EIJe5G/1
        let gem_entry_re =
            Regex::new("  \\* (\\S+) \\(([a-zA-Z0-9\\.]+)\\)").map_err(GemListError::RegexError)?;

        let gems = gem_entry_re
            .captures_iter(string)
            .map(
                |capture| {
                    let name = match capture.get(1) {
                        Some(m) => m.as_str(),
                        None => "",
                    };

                    let version = match capture.get(2) {
                        Some(m) => m.as_str(),
                        None => "0.0.0",
                    };
                    (name.to_string(), version.to_string())
                }, //
            )
            .collect();

        Ok(GemList { gems })
    }
}

#[cfg(test)]

mod tests {
    use super::*;

    // https://github.com/rubygems/rubygems/blob/master/lib/rubygems/version.rb
    // https://github.com/rubygems/rubygems/blob/ecc8e895b69063562b9bf749b353948e051e4171/test/rubygems/test_gem_version.rb
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

    #[test]
    fn test_parsing_gem_list() {
        let gem_list = GemList::from_str(
            r#"
Gems included by the bundle:
  * actioncable (6.1.4.1)
  * actionmailbox (6.1.4.1)
  * actionmailer (6.1.4.1)
  * actionpack (6.1.4.1)
  * actiontext (6.1.4.1)
  * actionview (6.1.4.1)
  * activejob (6.1.4.1)
  * activemodel (6.1.4.1)
  * activerecord (6.1.4.1)
  * activestorage (6.1.4.1)
  * activesupport (6.1.4.1)
  * addressable (2.8.0)
  * ast (2.4.2)
  * railties (6.1.4.1)
Use `bundle info` to print more detailed information about a gem
            "#,
        )
        .unwrap();

        assert!(gem_list.has("railties"));
        assert!(!gem_list.has("foo"));

        assert_eq!(gem_list.version_for("railties").unwrap(), "6.1.4.1");
        assert_eq!(gem_list.version_for("foo"), None);

        assert_eq!(gem_list.gems.len(), 14);
    }
}

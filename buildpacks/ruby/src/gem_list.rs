use bullet_stream::state::SubBullet;
use bullet_stream::{style, Print};
use commons::gem_version::GemVersion;
use core::str::FromStr;
use fun_run::{CmdError, CommandWithName};
use regex::Regex;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::Stdout;
use std::process::Command;

/// ## Gets list of an application's dependencies
///
/// Requires `ruby` and `bundle` to be installed and on the PATH
#[derive(Debug)]
pub(crate) struct GemList {
    pub(crate) gems: HashMap<String, GemVersion>,
}

/// Converts the output of `$ gem list` into a data structure that can be inspected and compared
///
/// ```
/// use commons::gem_list::GemList;
/// use commons::gem_version::GemVersion;
/// use std::str::FromStr;
///
///         let gem_list = GemList::from_str(
///             r#"
/// Gems included by the bundle:
///   * actioncable (6.1.4.1)
///   * actionmailbox (6.1.4.1)
///   * actionmailer (6.1.4.1)
///   * actionpack (6.1.4.1)
///   * actiontext (6.1.4.1)
///   * actionview (6.1.4.1)
///   * activejob (6.1.4.1)
///   * activemodel (6.1.4.1)
///   * activerecord (6.1.4.1)
///   * activestorage (6.1.4.1)
///   * activesupport (6.1.4.1)
///   * addressable (2.8.0)
///   * ast (2.4.2)
///   * railties (6.1.4.1)
/// Use `bundle info` to print more detailed information about a gem
///             "#,
///         ).unwrap();
///
///         assert!(gem_list.has("railties"));
///
///         assert_eq!(
///            gem_list.version_for("railties").unwrap(),
///            &GemVersion::from_str("6.1.4.1").unwrap()
///         );
/// ```
impl GemList {
    /// Calls `bundle list` and returns a `GemList` struct
    ///
    /// # Errors
    ///
    /// Errors if the command `bundle list` is unsuccessful.
    pub(crate) fn from_bundle_list<T, K, V>(
        envs: T,
        bullet: Print<SubBullet<Stdout>>,
    ) -> Result<(Print<SubBullet<Stdout>>, Self), CmdError>
    where
        T: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        let mut cmd = Command::new("bundle");
        cmd.arg("list").env_clear().envs(envs);

        let timer = bullet.start_timer(format!("Running {}", style::command(cmd.name())));
        let output = cmd.named_output()?;
        GemList::from_str(&output.stdout_lossy()).map(|gem_list| (timer.done(), gem_list))
    }

    #[must_use]
    pub(crate) fn has(&self, str: &str) -> bool {
        self.gems.contains_key(&str.trim().to_lowercase())
    }
}

impl FromStr for GemList {
    type Err = CmdError;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        // https://regex101.com/r/EIJe5G/1
        let gem_entry_re = Regex::new("  \\* (\\S+) \\(([a-zA-Z0-9\\.]+)\\)")
            .expect("Internal error: invalid regex");

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
                    (
                        name.to_string().to_lowercase(),
                        GemVersion::from_str(version).unwrap_or_default(),
                    )
                }, //
            )
            .collect::<HashMap<String, GemVersion>>();

        Ok(GemList { gems })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsing_gem_list() {
        let gem_list = GemList::from_str(
            r"
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
            ",
        )
        .unwrap();

        assert!(gem_list.has("railties"));
        assert!(!gem_list.has("foo"));

        assert_eq!(gem_list.gems.len(), 14);
    }
}

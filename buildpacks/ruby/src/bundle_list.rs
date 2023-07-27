use crate::build_output::{RunCommand, Section};
use commons::fun_run::{CmdErrorDiagnostics, CmdMapExt};
use commons::gem_version::GemVersion;
use core::str::FromStr;
use regex::Regex;
use std::collections::HashMap;
use std::process::Command;

/// ## Gets list of an application's dependencies
///
/// Requires `ruby` and `bundle` to be installed and on the PATH
#[derive(Debug)]
pub(crate) struct GemList {
    pub gems: HashMap<String, GemVersion>,
}

/// Converts the output of `$ gem list` into a data structure that can be inspected and compared
///
impl GemList {
    /// Calls `bundle list` and returns a `GemList` struct
    ///
    /// # Errors
    ///
    /// Errors if the command `bundle list` is unsuccessful.
    pub(crate) fn from_bundle_list(
        env: &libcnb::Env,
        build_output: &Section,
    ) -> Result<Self, CmdErrorDiagnostics> {
        Command::new("bundle")
            .arg("list")
            .env_clear()
            .envs(env)
            .cmd_map(|cmd| build_output.run(RunCommand::inline_progress(cmd)))
            .map_err(|error| {
                CmdErrorDiagnostics::from_cmd_error(error)
                    .run_and_insert(Command::new("bundle").arg("env").env_clear().envs(env))
                    .run_and_insert(Command::new("gem").arg("env").env_clear().envs(env))
            })
            .and_then(|output| GemList::from_str(&String::from_utf8_lossy(&output.stdout)))
    }

    #[must_use]
    pub(crate) fn has(&self, str: &str) -> bool {
        self.gems.get(&str.trim().to_lowercase()).is_some()
    }
}

impl FromStr for GemList {
    type Err = CmdErrorDiagnostics;

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

        assert_eq!(gem_list.gems.len(), 14);
    }
}

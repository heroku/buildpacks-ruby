use crate::build_output::{RunCommand, Section};
use commons::fun_run::{self, CmdError, CmdMapExt};
use commons::gem_version::GemVersion;
use core::str::FromStr;
use regex::Regex;
use std::collections::HashMap;
use std::fmt::Display;
use std::process::Command;
use std::process::Output;

/// ## Gets list of an application's dependencies
///
/// Requires `ruby` and `bundle` to be installed and on the PATH
#[derive(Debug)]
pub(crate) struct GemList {
    pub gems: HashMap<String, GemVersion>,
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum CmdErrorWithDiagnostics {
    #[error("{0}\n{1}")]
    CommandErrorWithDiagnostics(fun_run::CmdError, DiagnosticCommands),
}

#[derive(Debug)]
pub(crate) enum DiagnosticCmd {
    Error(CmdError),
    Info { name: String, output: Output },
}

impl Display for DiagnosticCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("System diagnostic information:\n\n")?;
        match self {
            DiagnosticCmd::Error(error) => writeln!(f, "{error}"),
            DiagnosticCmd::Info { name, output } => {
                let status = output.status;
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                writeln!(
                    f,
                    "Command: {name}\nstatus: {status}\nstdout: {stdout}\n stderr: {stderr}"
                )
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct DiagnosticCommands(Vec<DiagnosticCmd>);

impl Display for DiagnosticCommands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for diagnostic in &self.0 {
            writeln!(f, "{diagnostic}")?;
        }
        Ok(())
    }
}

// If `bundle list` is failing then there's something deeply wrong.
//
// Annotate the failure with as much diagnostic information as possible
fn run_diagnostic_cmd(cmd: &mut Command) -> DiagnosticCmd {
    let diagnostic = cmd.cmd_map(|cmd| {
        let name = fun_run::display(cmd);

        cmd.output()
            .map_err(|error| fun_run::on_system_error(name.clone(), error))
            .and_then(|output| fun_run::nonzero_captured(name.clone(), output))
            .map(|output| (name, output))
    });

    match diagnostic {
        Ok((name, output)) => DiagnosticCmd::Info { name, output },
        Err(error) => DiagnosticCmd::Error(error),
    }
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
    ) -> Result<Self, CmdErrorWithDiagnostics> {
        let result = Command::new("bundle")
            .arg("list")
            .env_clear()
            .envs(env)
            .cmd_map(|cmd| build_output.run(RunCommand::inline_progress(cmd)));

        match result {
            Ok(output) => GemList::from_str(&String::from_utf8_lossy(&output.stdout)),
            Err(error) => Err(CmdErrorWithDiagnostics::CommandErrorWithDiagnostics(
                error,
                DiagnosticCommands(vec![
                    Command::new("bundle")
                        .arg("env")
                        .env_clear()
                        .envs(env)
                        .cmd_map(run_diagnostic_cmd),
                    Command::new("gem")
                        .arg("env")
                        .env_clear()
                        .envs(env)
                        .cmd_map(run_diagnostic_cmd),
                ]),
            )),
        }
    }

    #[must_use]
    pub(crate) fn has(&self, str: &str) -> bool {
        self.gems.get(&str.trim().to_lowercase()).is_some()
    }
}

impl FromStr for GemList {
    type Err = CmdErrorWithDiagnostics;

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

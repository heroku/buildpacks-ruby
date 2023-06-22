use crate::fun_run::{self, CmdMapExt};
use core::str::FromStr;
use regex::Regex;
use std::{ffi::OsStr, process::Command};

/// Run `rake -P` and parse output to show what rake tasks an application has
///
/// ```rust,no_run
/// use commons::rake_task_detect::RakeDetect;
/// use libcnb::Env;
///
/// let rake_detect = RakeDetect::from_rake_command(&Env::new(), false).unwrap();
/// assert!(!rake_detect.has_task("assets:precompile"));
/// ```
#[derive(Default)]
pub struct RakeDetect {
    output: String,
}

#[derive(thiserror::Error, Debug)]
pub enum RakeError {
    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),

    #[error("Error detecting rake tasks: {0}")]
    DashpCommandError(fun_run::CmdError),
}

impl RakeDetect {
    /// # Errors
    ///
    /// Will return `Err` if `bundle exec rake -p` command cannot be invoked by the operating system.
    pub fn from_rake_command<T: IntoIterator<Item = (K, V)>, K: AsRef<OsStr>, V: AsRef<OsStr>>(
        envs: T,
        error_on_failure: bool,
    ) -> Result<Self, RakeError> {
        Command::new("bundle")
            .args(["exec", "rake", "-P", "--trace"])
            .env_clear()
            .envs(envs)
            .cmd_map(|cmd| {
                let name = fun_run::display(cmd);
                cmd.output()
                    .map_err(|error| fun_run::on_system_error(name.clone(), error))
                    .and_then(|output| {
                        if error_on_failure {
                            fun_run::nonzero_captured(name.clone(), output)
                        } else {
                            Ok(output)
                        }
                    })
            })
            .map_err(RakeError::DashpCommandError)
            .and_then(|output| RakeDetect::from_str(&String::from_utf8_lossy(&output.stdout)))
    }

    #[must_use]
    pub fn has_task(&self, string: &str) -> bool {
        let task_re = Regex::new(&format!("\\s{string}")).expect("Internal error with regex");
        task_re.is_match(&self.output)
    }
}

impl FromStr for RakeDetect {
    type Err = RakeError;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        Ok(RakeDetect {
            output: string.to_lowercase(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsing_rake_dash_p() {
        let rake_detect = RakeDetect::from_str(
            r#"
rake about
    environment
rake action_mailbox:ingress:environment
rake action_mailbox:ingress:exim
    action_mailbox:ingress:environment
rake action_mailbox:ingress:postfix
    action_mailbox:ingress:environment
rake action_mailbox:ingress:qmail
    action_mailbox:ingress:environment
rake action_mailbox:install
rake action_mailbox:install:migrations
rake action_text:install
rake action_text:install:migrations
rake active_storage:install
    environment
rake active_storage:install:migrations
rake active_storage:update
    environment
rake app:binstub:yarn
rake app:template
    environment
rake app:templates:copy
rake app:update
    update:configs
    update:bin
    update:active_storage
    update:upgrade_guide_info
rake app:update:active_storage
rake app:update:bin
rake app:update:configs
rake app:update:upgrade_guide_info
rake assets:bench
rake assets:clean
    environment
rake assets:clobber
    environment
rake assets:environment
rake assets:precompile
    environment
    yarn:install
        "#,
        )
        .unwrap();

        assert!(rake_detect.has_task("assets:precompile"));
    }
}

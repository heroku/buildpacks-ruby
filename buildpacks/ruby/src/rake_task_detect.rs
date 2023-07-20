use crate::build_output::{RunCommand, Section};
use commons::fun_run::{self, CmdError, CmdMapExt};
use core::str::FromStr;
use regex::Regex;
use std::{ffi::OsString, process::Command};

#[derive(Default)]
pub(crate) struct RakeDetect {
    output: String,
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum CannotDetectRakeTasks {
    #[error("Error detecting rake tasks: {0}")]
    DashpCommandError(fun_run::CmdError),
}

impl RakeDetect {
    /// # Errors
    ///
    /// Will return `Err` if `bundle exec rake -p` command cannot be invoked by the operating system.
    pub(crate) fn from_rake_command(
        section: &Section,
        env: &libcnb::Env,
        error_on_failure: bool,
        highlight_keys: Vec<OsString>,
    ) -> Result<Self, CannotDetectRakeTasks> {
        Command::new("bundle")
            .args(["exec", "rake", "-P", "--trace"])
            .env_clear()
            .envs(env)
            .cmd_map(|cmd| {
                let name = fun_run::display_with_env_keys(cmd, env, &highlight_keys);
                section
                    .run(RunCommand::silent(cmd).with_name(name))
                    .or_else(|error| {
                        if error_on_failure {
                            Err(error)
                        } else {
                            match error {
                                CmdError::SystemError(_, _) => Err(error),
                                CmdError::NonZeroExitNotStreamed(_, output)
                                | CmdError::NonZeroExitAlreadyStreamed(_, output) => Ok(output),
                            }
                        }
                    })
            })
            .map_err(CannotDetectRakeTasks::DashpCommandError)
            .and_then(|output| RakeDetect::from_str(&String::from_utf8_lossy(&output.stdout)))
    }

    #[must_use]
    pub(crate) fn has_task(&self, string: &str) -> bool {
        let task_re = Regex::new(&format!("\\s{string}")).expect("Internal error with regex");
        task_re.is_match(&self.output)
    }
}

impl FromStr for RakeDetect {
    type Err = CannotDetectRakeTasks;

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

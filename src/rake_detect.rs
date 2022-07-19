use core::str::FromStr;
use regex::Regex;

use crate::env_command::{EnvCommand, EnvCommandError};
use libcnb::Env;

#[derive(Default)]
pub struct RakeDetect {
    #[allow(dead_code)]
    output: String,
}

#[derive(thiserror::Error, Debug)]
pub enum RakeDetectError {
    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),

    #[error("Error detecting rake tasks: {0}")]
    RakeDashpCommandError(EnvCommandError),
}

impl RakeDetect {
    #[allow(dead_code)]
    pub fn from_rake_command(env: &Env, error_on_failure: bool) -> Result<Self, RakeDetectError> {
        let mut command = EnvCommand::new("bundle", &["exec", "rake", "-P", "--trace"], env);
        let outcome = command
            .on_non_zero_exit(move |error| {
                if error_on_failure {
                    Err(error)
                } else {
                    Ok(error.result)
                }
            })
            .call()
            .map_err(RakeDetectError::RakeDashpCommandError)?;

        if outcome.status.success() {
            RakeDetect::from_str(&outcome.stdout)
        } else {
            Ok(RakeDetect::default())
        }
    }

    #[allow(dead_code)]
    pub fn has_task(&self, string: &str) -> bool {
        let task_re = Regex::new(&format!("\\s{}", string)).expect("Internal error with regex");
        task_re.is_match(&self.output)
    }
}

impl FromStr for RakeDetect {
    type Err = RakeDetectError;

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

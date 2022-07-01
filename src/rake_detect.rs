use core::str::FromStr;
use regex::Regex;

use libcnb::Env;
use std::process::{Command, ExitStatus};

use std::str::Utf8Error;
pub struct RakeDetect {
    #[allow(dead_code)]
    output: String,
}

#[derive(thiserror::Error, Debug)]
pub enum RakeDetectError {
    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),

    #[error("Command `bundle exec rake -P` errored: {0}")]
    RakeDashpCommandError(std::io::Error),

    #[error("Command `bundle exec rake -P` exited with non-zero error code {0} stdout:\n{1}\nstderr:\n{2}\n")]
    RakeDashpUnexpectedExitStatus(ExitStatus, String, String),

    #[error("Encoding error: {0}")]
    EncodingError(#[from] Utf8Error),
}

impl RakeDetect {
    pub fn from_rake_command(env: &Env) -> Result<Self, RakeDetectError> {
        let mut command = Command::new("bundle");
        command.args(&["exec", "rake", "-P", "--trace"]).envs(env);

        let output = command
            .output()
            .map_err(RakeDetectError::RakeDashpCommandError)?;

        let stdout = std::str::from_utf8(&output.stdout).map_err(RakeDetectError::EncodingError)?;
        let stderr = std::str::from_utf8(&output.stderr).map_err(RakeDetectError::EncodingError)?;
        if output.status.success() {
            RakeDetect::from_str(stdout)
        } else {
            Err(RakeDetectError::RakeDashpUnexpectedExitStatus(
                output.status,
                stdout.to_string(),
                stderr.to_string(),
            ))
        }
    }

    #[allow(dead_code)]
    fn has_task(&self, string: &str) -> bool {
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

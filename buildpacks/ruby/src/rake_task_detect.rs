use bullet_stream::{
    state::SubBullet,
    {style, Print},
};
use core::str::FromStr;
use fun_run::{CmdError, CommandWithName};
use std::io::Stdout;
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
pub(crate) struct RakeDetect {
    output: String,
}

/// # Errors
///
/// Will return `Err` if `bundle exec rake -p` command cannot be invoked by the operating system.
pub(crate) fn call<T: IntoIterator<Item = (K, V)>, K: AsRef<OsStr>, V: AsRef<OsStr>>(
    bullet: Print<SubBullet<Stdout>>,
    envs: T,
    error_on_failure: bool,
) -> Result<(Print<SubBullet<Stdout>>, RakeDetect), CmdError> {
    let mut cmd = Command::new("bundle");
    cmd.args(["exec", "rake", "-P", "--trace"])
        .env_clear()
        .envs(envs);

    let timer = bullet.start_timer(format!("Running {}", style::command(cmd.name())));
    let output = cmd.named_output().or_else(|error| {
        if error_on_failure {
            Err(error)
        } else {
            match error {
                CmdError::SystemError(_, _) => Err(error),
                CmdError::NonZeroExitNotStreamed(output)
                | CmdError::NonZeroExitAlreadyStreamed(output) => Ok(output),
            }
        }
    })?;

    RakeDetect::from_str(&output.stdout_lossy()).map(|rake_detect| (timer.done(), rake_detect))
}

impl RakeDetect {
    #[must_use]
    pub(crate) fn has_task(&self, string: &str) -> bool {
        let task_re = regex::Regex::new(&format!("\\s{string}")).expect("clippy");
        task_re.is_match(&self.output)
    }
}

impl FromStr for RakeDetect {
    type Err = CmdError;

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
            r"
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
        ",
        )
        .unwrap();

        assert!(rake_detect.has_task("assets:precompile"));
    }
}

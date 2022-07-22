use libcnb::Env;

use crate::RubyBuildpackError;

use crate::env_command::EnvCommand;
use crate::gem_list::GemList;
use crate::rake_detect::RakeDetect;

pub struct RakeApplicationTasksExecute;

impl RakeApplicationTasksExecute {
    pub fn call(env: &Env) -> Result<(), RubyBuildpackError> {
        // ## Get list of gems and their versions from the system
        println!("---> Detecting gems");
        let gem_list =
            GemList::from_bundle_list(env).map_err(RubyBuildpackError::GemListGetError)?;

        let has_sprockets = gem_list.has("sprockets");

        // Get list of valid rake tasks
        println!("---> Detecting rake tasks");
        let rake_detect = RakeDetect::from_rake_command(env, has_sprockets)
            .map_err(RubyBuildpackError::RakeDetectError)?;

        if rake_detect.has_task("assets:precompile") {
            let assets_precompile = EnvCommand::new("rake", &["assets:precompile", "--trace"], env);
            assets_precompile.call().unwrap();
        }

        Ok(())
    }
}

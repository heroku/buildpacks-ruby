//! # Install the bundler gem
//!
//! ## Layer dir: Install bundler to disk
//!
//! Installs a copy of `bundler` to the `<layer-dir>` with a bundler executable in
//! `<layer-dir>/bin`.
use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use commons::gemfile_lock::ResolvedBundlerVersion;
use commons::output::{
    fmt,
    section_log::{log_step, log_step_timed, SectionLogger},
};
use fun_run::{self, CommandWithName};
use libcnb::build::BuildContext;
use libcnb::data::layer_content_metadata::LayerTypes;
#[allow(deprecated)]
use libcnb::layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::Env;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct Metadata {
    pub(crate) version: ResolvedBundlerVersion,
}

pub(crate) struct BundleDownloadLayer<'a> {
    pub(crate) env: Env,
    pub(crate) metadata: Metadata,
    pub(crate) _section_logger: &'a dyn SectionLogger,
}

#[allow(deprecated)]
impl<'a> Layer for BundleDownloadLayer<'a> {
    type Buildpack = RubyBuildpack;
    type Metadata = Metadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: true,
            launch: true,
            cache: true,
        }
    }

    fn create(
        &mut self,
        _context: &BuildContext<Self::Buildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        let bin_dir = layer_path.join("bin");
        let gem_path = layer_path;

        let mut cmd = Command::new("gem");
        cmd.args(["install", "bundler"]);
        cmd.args(["--version", &self.metadata.version.to_string()])
            .env_clear()
            .envs(&self.env);

        // Format `gem install --version <version>` without other content for display
        let short_name = fun_run::display(&mut cmd);

        // Directory where bundler's contents will live
        cmd.args(["--install-dir", &format!("{}", layer_path.display())]);
        // Directory where `bundle` executable lives
        cmd.args(["--bindir", &format!("{}", bin_dir.display())]);
        cmd.args([
            "--force",       // Overwrite if it already exists
            "--no-document", // Don't install ri or rdoc documentation, which takes extra time
            "--env-shebang", // Start the `bundle` executable with `#! /usr/bin/env ruby`
        ]);

        log_step_timed(format!("Running {}", fmt::command(short_name)), || {
            cmd.named_output().map_err(|error| {
                fun_run::map_which_problem(error, cmd.mut_cmd(), self.env.get("PATH").cloned())
            })
        })
        .map_err(RubyBuildpackError::GemInstallBundlerCommandError)?;

        LayerResultBuilder::new(self.metadata.clone())
            .env(
                LayerEnv::new()
                    .chainable_insert(Scope::All, ModificationBehavior::Delimiter, "PATH", ":")
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Prepend,
                        "PATH", // Ensure this path comes before default bundler that ships with ruby, don't rely on the lifecycle
                        bin_dir,
                    )
                    .chainable_insert(Scope::All, ModificationBehavior::Delimiter, "GEM_PATH", ":")
                    .chainable_insert(
                        Scope::All,
                        ModificationBehavior::Prepend,
                        "GEM_PATH", // Bundler is a gem too, allow it to be required
                        gem_path,
                    ),
            )
            .build()
    }

    fn existing_layer_strategy(
        &mut self,
        _context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        let old = &layer_data.content_metadata.metadata;
        if let Some(cause) = metadata_diff(old, &self.metadata) {
            log_step(format!("Clearing cache due to change: {cause}"));

            Ok(ExistingLayerStrategy::Recreate)
        } else {
            log_step("Using cached version");
            Ok(ExistingLayerStrategy::Keep)
        }
    }
}

fn metadata_diff(old: &Metadata, metadata: &Metadata) -> Option<String> {
    let Metadata { version } = old;

    if version == &metadata.version {
        None
    } else {
        Some(format!(
            "Bundler version ({old} to {now})",
            old = fmt::value(version.to_string()),
            now = fmt::value(metadata.version.to_string())
        ))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// If this test fails due to a change you'll need to implement
    /// `migrate_incompatible_metadata` for the Layer trait
    #[test]
    fn metadata_guard() {
        let metadata = Metadata {
            version: ResolvedBundlerVersion(String::from("2.3.6")),
        };

        let actual = toml::to_string(&metadata).unwrap();
        let expected = r#"
version = "2.3.6"
"#
        .trim();
        assert_eq!(expected, actual.trim());
    }
}

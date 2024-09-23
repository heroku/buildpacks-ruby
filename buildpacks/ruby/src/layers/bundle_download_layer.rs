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
    section_log::{log_step, log_step_timed},
};
use fun_run::{self, CommandWithName};
use libcnb::data::layer_name;
use libcnb::layer::{
    CachedLayerDefinition, EmptyLayerCause, InvalidMetadataAction, LayerState, RestoredLayerAction,
};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::Env;
use serde::{Deserialize, Serialize};
use std::process::Command;

pub(crate) fn handle(
    context: &libcnb::build::BuildContext<RubyBuildpack>,
    env: &Env,
    metadata: Metadata,
) -> libcnb::Result<LayerEnv, RubyBuildpackError> {
    // TODO switch logging to bullet stream
    let layer_ref = context.cached_layer(
        layer_name!("bundler"),
        CachedLayerDefinition {
            build: true,
            launch: true,
            invalid_metadata_action: &|_| {
                (
                    InvalidMetadataAction::DeleteLayer,
                    "invalid metadata".to_string(),
                )
            },
            restored_layer_action: &|old: &Metadata, _| {
                if let Some(cause) = metadata_diff(old, &metadata) {
                    (RestoredLayerAction::DeleteLayer, cause)
                } else {
                    (
                        RestoredLayerAction::KeepLayer,
                        "using cached version".to_string(),
                    )
                }
            },
        },
    )?;

    let bin_dir = layer_ref.path().join("bin");
    let gem_path = layer_ref.path();
    match &layer_ref.state {
        LayerState::Restored { cause: _ } => {
            log_step("Using cached version");
        }
        LayerState::Empty { cause } => {
            match cause {
                EmptyLayerCause::NewlyCreated => {}
                EmptyLayerCause::InvalidMetadataAction { cause }
                | EmptyLayerCause::RestoredLayerAction { cause } => {
                    log_step(format!("Clearing cache {cause}"));
                }
            }

            let mut cmd = Command::new("gem");
            cmd.args(["install", "bundler"]);
            cmd.args(["--version", &metadata.version.to_string()])
                .env_clear()
                .envs(env);

            // Format `gem install --version <version>` without other content for display
            let short_name = fun_run::display(&mut cmd);

            // Directory where bundler's contents will live
            cmd.args(["--install-dir", &format!("{}", layer_ref.path().display())]);
            // Directory where `bundle` executable lives
            cmd.args(["--bindir", &format!("{}", bin_dir.display())]);
            cmd.args([
                "--force",       // Overwrite if it already exists
                "--no-document", // Don't install ri or rdoc documentation, which takes extra time
                "--env-shebang", // Start the `bundle` executable with `#! /usr/bin/env ruby`
            ]);

            log_step_timed(format!("Running {}", fmt::command(short_name)), || {
                cmd.named_output().map_err(|error| {
                    fun_run::map_which_problem(error, cmd.mut_cmd(), env.get("PATH").cloned())
                })
            })
            .map_err(RubyBuildpackError::GemInstallBundlerCommandError)?;
        }
    }
    layer_ref.write_env(
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
    )?;
    layer_ref.write_metadata(metadata)?;
    layer_ref.read_env()
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct Metadata {
    pub(crate) version: ResolvedBundlerVersion,
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
    /// `invalid_metadata_action`
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

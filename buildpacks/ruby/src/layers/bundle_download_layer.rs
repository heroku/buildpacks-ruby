use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use bullet_stream::state::SubBullet;
use bullet_stream::{style, Print};
use commons::gemfile_lock::ResolvedBundlerVersion;
use commons::layer::MetadataMigrationFYI;
use fun_run::{self, CommandWithName};
use libcnb::build::BuildContext;
use libcnb::data::layer_name;
use libcnb::layer::{
    CachedLayerDefinition, EmptyLayerCause, IntoAction, LayerState, RestoredLayerAction,
};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::Env;
use magic_migrate::try_migrate_deserializer_chain;
use magic_migrate::TryMigrate;
use serde::Deserializer;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::io::Stdout;
use std::process::Command;

/// # Install the bundler gem
///
/// ## Layer dir: Install bundler to disk
///
/// Installs a copy of `bundler` to the `<layer-dir>` with a bundler executable in
/// `<layer-dir>/bin`. Must run before [`crate.steps.bundle_install`].
// [derive(Debug)]
pub(crate) fn download_bundler(
    context: &BuildContext<RubyBuildpack>,
    env: &Env,
    mut bullet: Print<SubBullet<Stdout>>,
    new_metadata: BundleDownloadLayerMetadata,
) -> Result<(Print<SubBullet<Stdout>>, LayerEnv), libcnb::Error<RubyBuildpackError>> {
    let layer = context.cached_layer(
        layer_name!("bundler"),
        CachedLayerDefinition {
            build: true,
            launch: true,
            invalid_metadata_action: &|invalid_metadata| {
                let toml_string = invalid_metadata.as_ref().map_or_else(String::new, |m| {
                    toml::to_string(m).expect("TOML serializes back to toml")
                });

                match Metadata::try_from_str_migrations(&toml_string) {
                    Some(Ok(migrated)) => {
                        MetadataMigrationFYI::Migrated(migrated, "Success".to_string())
                    }
                    Some(Err(error)) => MetadataMigrationFYI::Delete(format!(
                        "Error while migrating metadata {error}"
                    )),
                    None => MetadataMigrationFYI::Delete(format!(
                        "Could not serialize metadata into a known struct. Metadata: {toml_string}",
                    )),
                }
            },
            restored_layer_action: &|old_metadata: &Metadata, _| {
                cache_state(old_metadata.clone(), new_metadata.clone())
            },
        },
    )?;

    match layer.state {
        LayerState::Restored { .. } => {
            bullet = bullet.sub_bullet("Using cached Ruby version");
        }
        LayerState::Empty {
            cause:
                EmptyLayerCause::InvalidMetadataAction { ref cause }
                | EmptyLayerCause::RestoredLayerAction { ref cause },
        } => {
            bullet = bullet.sub_bullet(format!("Clearing cache ({cause})"));
        }
        LayerState::Empty {
            cause: EmptyLayerCause::NewlyCreated,
        } => {}
    };

    let bin_dir = layer.path().join("bin");
    let gem_path = layer.path();
    match layer.state {
        LayerState::Restored { .. } => {}
        LayerState::Empty { .. } => {
            let mut cmd = Command::new("gem");
            cmd.args([
                "install",
                "bundler",
                "--version", // Specify exact version to install
                &new_metadata.version.to_string(),
            ])
            .env_clear()
            .envs(env);

            // Format `gem install --version <version>` without other content for display
            let short_name = fun_run::display(&mut cmd);

            // Arguments we don't need in the output
            cmd.args([
                "--install-dir", // Directory where bundler's contents will live
                &layer.path().to_string_lossy(),
                "--bindir", // Directory where `bundle` executable lives
                &bin_dir.to_string_lossy(),
                "--force",       // Overwrite if it already exists
                "--no-document", // Don't install ri or rdoc documentation, which takes extra time
                "--env-shebang", // Start the `bundle` executable with `#! /usr/bin/env ruby`
            ]);

            let timer = bullet.start_timer(format!("Running {}", style::command(short_name)));
            cmd.named_output()
                .map_err(|error| {
                    fun_run::map_which_problem(error, cmd.mut_cmd(), env.get("PATH").cloned())
                })
                .map_err(RubyBuildpackError::GemInstallBundlerCommandError)?;

            bullet = timer.done();
        }
    };

    layer.write_metadata(new_metadata)?;
    layer.write_env(
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

    Ok((bullet, layer.read_env()?))
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct BundleDownloadLayerMetadataV1 {
    pub(crate) version: ResolvedBundlerVersion,
}
pub(crate) type BundleDownloadLayerMetadata = BundleDownloadLayerMetadataV1;

#[derive(thiserror::Error, Debug)]
pub(crate) enum MetadataMigrateError {}

try_migrate_deserializer_chain!(
    chain: [BundleDownloadLayerMetadataV1],
    error: MetadataMigrateError,
    deserializer: toml::Deserializer::new,
);

type Metadata = BundleDownloadLayerMetadata;

impl<E> IntoAction<RestoredLayerAction, String, E> for State {
    fn into_action(self) -> Result<(RestoredLayerAction, String), E> {
        match self {
            State::NothingChanged(_version) => Ok((
                RestoredLayerAction::KeepLayer,
                "Using CachedVersion".to_string(),
            )),
            State::BundlerVersionChanged(_old, _now) => Ok((
                RestoredLayerAction::DeleteLayer,
                "Clearing cache (bundler version changed)".to_string(),
            )),
        }
    }
}

enum State {
    NothingChanged(ResolvedBundlerVersion),
    BundlerVersionChanged(ResolvedBundlerVersion, ResolvedBundlerVersion),
}

fn cache_state(old: BundleDownloadLayerMetadata, now: BundleDownloadLayerMetadata) -> State {
    let BundleDownloadLayerMetadata { version } = now; // Ensure all properties are checked

    if old.version == version {
        State::NothingChanged(version)
    } else {
        State::BundlerVersionChanged(old.version, version)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// If this test fails due to a change you'll need to implement
    /// `migrate_incompatible_metadata` for the Layer trait
    #[test]
    fn metadata_guard() {
        let metadata = BundleDownloadLayerMetadata {
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

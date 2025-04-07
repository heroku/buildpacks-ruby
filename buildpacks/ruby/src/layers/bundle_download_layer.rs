//! # Install the bundler gem
//!
//! ## Layer dir: Install bundler to disk
//!
//! Installs a copy of `bundler` to the `<layer-dir>` with a bundler executable in
//! `<layer-dir>/bin`. Must run before [`crate.steps.bundle_install`].
use crate::RubyBuildpack;
use crate::RubyBuildpackError;
use bullet_stream::state::SubBullet;
use bullet_stream::Print;
use cache_diff::CacheDiff;
use commons::gemfile_lock::ResolvedBundlerVersion;
use commons::layer::diff_migrate::DiffMigrateLayer;
use fun_run::{self, CommandWithName};
use libcnb::data::layer_name;
use libcnb::layer::{EmptyLayerCause, LayerState};
use libcnb::layer_env::{LayerEnv, ModificationBehavior, Scope};
use libcnb::Env;
use magic_migrate::TryMigrate;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use std::process::Command;

pub(crate) fn handle<W>(
    context: &libcnb::build::BuildContext<RubyBuildpack>,
    env: &Env,
    mut bullet: Print<SubBullet<W>>,
    metadata: &Metadata,
) -> libcnb::Result<(Print<SubBullet<W>>, LayerEnv), RubyBuildpackError>
where
    W: Write + Send + Sync + 'static,
{
    let layer_ref = DiffMigrateLayer {
        build: true,
        launch: true,
    }
    .cached_layer(layer_name!("bundler"), context, metadata)?;

    let layer_env = LayerEnv::new()
        .chainable_insert(Scope::All, ModificationBehavior::Delimiter, "PATH", ":")
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Prepend,
            "PATH",
            // Ensure this path comes before default bundler that ships with ruby, don't rely on the lifecycle
            layer_ref.path().join("bin"),
        )
        .chainable_insert(Scope::All, ModificationBehavior::Delimiter, "GEM_PATH", ":")
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Prepend,
            "GEM_PATH", // Bundler is a gem too, allow it to be required
            layer_ref.path(),
        );
    layer_ref.write_env(&layer_env)?;
    match &layer_ref.state {
        LayerState::Restored { cause } => {
            bullet = bullet.sub_bullet(cause);
        }
        LayerState::Empty { cause } => {
            match cause {
                EmptyLayerCause::NewlyCreated => {}
                EmptyLayerCause::InvalidMetadataAction { cause }
                | EmptyLayerCause::RestoredLayerAction { cause } => {
                    bullet = bullet.sub_bullet(cause);
                }
            }
            bullet = download_bundler(bullet, env, &metadata.version, &layer_ref.path())
                .map_err(RubyBuildpackError::GemInstallBundlerCommandError)?;
        }
    }
    Ok((bullet, layer_ref.read_env()?))
}

pub(crate) type Metadata = MetadataV1;

#[derive(Deserialize, Serialize, Debug, Clone, CacheDiff, TryMigrate)]
#[try_migrate(from = None)]
#[serde(deny_unknown_fields)]
pub(crate) struct MetadataV1 {
    #[cache_diff(rename = "Bundler version")]
    pub(crate) version: ResolvedBundlerVersion,
}

#[tracing::instrument(skip(bullet, env, gem_path), err)]
fn download_bundler<W>(
    mut bullet: Print<SubBullet<W>>,
    env: &Env,
    version: &ResolvedBundlerVersion,
    gem_path: &Path,
) -> Result<Print<SubBullet<W>>, fun_run::CmdError>
where
    W: Write + Send + Sync + 'static,
{
    let bin_dir = gem_path.join("bin");

    let mut cmd = Command::new("gem");
    cmd.args(["install", "bundler"]);
    cmd.args(["--version", &version.to_string()]) // Specify exact version to install
        .env_clear()
        .envs(env);

    let short_name = fun_run::display(&mut cmd); // Format `gem install --version <version>` without other content for display

    cmd.args(["--install-dir", &format!("{}", gem_path.display())]); // Directory where bundler's contents will live
    cmd.args(["--bindir", &format!("{}", bin_dir.display())]); // Directory where `bundle` executable lives
    cmd.args([
        "--force",       // Overwrite if it already exists
        "--no-document", // Don't install ri or rdoc documentation, which takes extra time
        "--env-shebang", // Start the `bundle` executable with `#! /usr/bin/env ruby`
    ]);

    bullet
        .time_cmd(&mut cmd.named(short_name))
        .map_err(|error| {
            fun_run::map_which_problem(error, cmd.mut_cmd(), env.get("PATH").cloned())
        })?;

    Ok(bullet)
}

#[cfg(test)]
mod test {
    use super::*;
    use bullet_stream::strip_ansi;

    #[test]
    fn test_metadata_diff() {
        let old = Metadata {
            version: ResolvedBundlerVersion("2.3.5".to_string()),
        };
        assert!(old.diff(&old).is_empty());

        let diff = Metadata {
            version: ResolvedBundlerVersion("2.3.6".to_string()),
        }
        .diff(&old);
        assert_eq!(
            diff.iter().map(strip_ansi).collect::<Vec<String>>(),
            vec!["Bundler version (`2.3.5` to `2.3.6`)"]
        );
    }

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

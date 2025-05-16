//! Installs Ruby gems (libraries) via `bundle install`
//!
//! Creates the cache where gems live. We want 'bundle install'
//! to execute on every build (as opposed to only when the cache is empty).
//!
//! Gems can be plain Ruby code which are OS, Architecture, and Ruby version independent.
//! They can also be native extensions that use Ruby's C API or contain libraries that
//! must be compiled and will then be invoked via FFI. These native extensions are
//! OS, Architecture, and Ruby version dependent. Due to this, when one of these changes
//! we must clear the cache and re-run `bundle install`.
use crate::target_id::OsDistribution;
use crate::{BundleWithout, RubyBuildpack, RubyBuildpackError};
use bullet_stream::global::print;
use cache_diff::CacheDiff;
use commons::gemfile_lock::ResolvedRubyVersion;
use commons::layer::diff_migrate::DiffMigrateLayer;
use fun_run::{self, CommandWithName};
use libcnb::data::layer_name;
use libcnb::layer::{EmptyLayerCause, LayerState};
use libcnb::{
    layer_env::{LayerEnv, ModificationBehavior, Scope},
    Env,
};
use magic_migrate::TryMigrate;
use serde::{Deserialize, Serialize};
use std::{path::Path, process::Command};

pub(crate) fn call(
    context: &libcnb::build::BuildContext<RubyBuildpack>,
    env: &Env,
    metadata: &Metadata,
    without: &BundleWithout,
) -> libcnb::Result<LayerEnv, RubyBuildpackError> {
    let layer_ref = DiffMigrateLayer {
        build: true,
        launch: true,
    }
    .cached_layer(layer_name!("gems"), context, metadata)?;
    match &layer_ref.state {
        LayerState::Restored { cause } => {
            print::sub_bullet(cause);
        }
        LayerState::Empty { cause } => match cause {
            EmptyLayerCause::NewlyCreated => (),
            EmptyLayerCause::InvalidMetadataAction { cause }
            | EmptyLayerCause::RestoredLayerAction { cause } => {
                print::sub_bullet(cause);
            }
        },
    }

    let env = {
        let layer_env = layer_env(&layer_ref.path(), &context.app_dir, without);
        layer_ref.write_env(&layer_env)?;
        layer_env.apply(Scope::Build, env)
    };

    let mut cmd = Command::new("bundle");
    cmd.arg("install")
        .env_clear() // Current process env vars already merged into env
        .envs(&env);

    print::sub_stream_cmd(cmd.named_fn(|cmd| display_name(cmd, &env)))
        .map_err(|error| fun_run::map_which_problem(error, cmd.mut_cmd(), env.get("PATH").cloned()))
        .map_err(RubyBuildpackError::BundleInstallCommandError)?;

    print::sub_time_cmd(
        Command::new("bundle")
            .args(["clean", "--force"])
            .env_clear()
            .envs(&env),
    )
    .map_err(RubyBuildpackError::BundleInstallCommandError)?;

    layer_ref.read_env()
}

pub(crate) type Metadata = MetadataV4;

// Introduced in https://github.com/heroku/buildpacks-ruby/pull/370
// 2024-12-13
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, CacheDiff, TryMigrate)]
#[cache_diff(custom = clear_v1)]
#[try_migrate(from = None)]
#[serde(deny_unknown_fields)]
pub(crate) struct MetadataV3 {
    #[cache_diff(rename = "OS Distribution")]
    pub(crate) os_distribution: OsDistribution,
    #[cache_diff(rename = "CPU Architecture")]
    pub(crate) cpu_architecture: String,
    #[cache_diff(rename = "Ruby version")]
    pub(crate) ruby_version: ResolvedRubyVersion,
    #[cache_diff(ignore)]
    pub(crate) force_bundle_install_key: String,

    #[cache_diff(ignore)]
    // Placeholder for a deprecated struct
    pub(crate) digest: toml::Value, // Must be last for serde to be happy https://github.com/toml-rs/toml-rs/issues/142
}

// Introduced to drop support for MetadataDigest
// 2025-05-16
#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq, CacheDiff, TryMigrate)]
#[try_migrate(from = MetadataV3)]
#[serde(deny_unknown_fields)]
pub(crate) struct MetadataV4 {
    #[cache_diff(rename = "OS Distribution")]
    pub(crate) os_distribution: OsDistribution,
    #[cache_diff(rename = "CPU Architecture")]
    pub(crate) cpu_architecture: String,
    #[cache_diff(rename = "Ruby version")]
    pub(crate) ruby_version: ResolvedRubyVersion,
}

fn clear_v1(_new: &MetadataV3, old: &MetadataV3) -> Vec<String> {
    if &old.force_bundle_install_key == "v1" {
        vec!["Internal gem directory structure changed".to_string()]
    } else {
        Vec::new()
    }
}

impl TryFrom<MetadataV3> for MetadataV4 {
    type Error = std::convert::Infallible;

    fn try_from(value: MetadataV3) -> Result<Self, Self::Error> {
        Ok(Self {
            os_distribution: value.os_distribution,
            cpu_architecture: value.cpu_architecture,
            ruby_version: value.ruby_version,
        })
    }
}

fn layer_env(layer_path: &Path, app_dir: &Path, without_default: &BundleWithout) -> LayerEnv {
    let layer_env = LayerEnv::new()
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Override,
            "GEM_HOME", // Tells bundler where to install gems, along with GEM_PATH
            layer_path,
        )
        .chainable_insert(Scope::All, ModificationBehavior::Delimiter, "GEM_PATH", ":")
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Prepend,
            "GEM_PATH", // Tells Ruby where gems are located.
            layer_path,
        )
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Default,
            "BUNDLE_WITHOUT", // Do not install `development` or `test` groups via bundle install. Additional groups can be specified via user config.
            without_default.as_str(),
        )
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Override,
            "BUNDLE_GEMFILE", // Tells bundler where to find the `Gemfile`
            app_dir.join("Gemfile"),
        )
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Override,
            "BUNDLE_FROZEN", // Requires the `Gemfile.lock` to be in sync with the current `Gemfile`.
            "1",
        );
    layer_env
}

/// Displays the `bundle install` command with `BUNDLE_` environment variables
/// that we use to configure bundler.
fn display_name(cmd: &mut Command, env: &Env) -> String {
    fun_run::display_with_env_keys(
        cmd,
        env,
        ["BUNDLE_FROZEN", "BUNDLE_GEMFILE", "BUNDLE_WITHOUT"],
    )
}

#[cfg(test)]
mod test {
    use crate::target_id::TargetId;

    use super::*;
    use bullet_stream::strip_ansi;
    use commons::display::SentenceList;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    /// `CacheDiff` logic controls cache invalidation
    /// When the vec is empty the cache is kept, otherwise it is invalidated
    #[test]
    fn metadata_diff_messages() {
        let old = Metadata {
            ruby_version: ResolvedRubyVersion("3.5.3".to_string()),
            os_distribution: OsDistribution {
                name: "ubuntu".to_string(),
                version: "24.04".to_string(),
            },
            cpu_architecture: "amd64".to_string(),
        };
        assert_eq!(old.diff(&old), Vec::<String>::new());

        let diff = Metadata {
            ruby_version: ResolvedRubyVersion("3.5.5".to_string()),
            os_distribution: old.os_distribution.clone(),
            cpu_architecture: old.cpu_architecture.clone(),
        }
        .diff(&old);

        assert_eq!(
            diff.iter().map(strip_ansi).collect::<Vec<String>>(),
            vec!["Ruby version (`3.5.3` to `3.5.5`)".to_string()]
        );

        let diff = Metadata {
            ruby_version: old.ruby_version.clone(),
            os_distribution: OsDistribution {
                name: "alpine".to_string(),
                version: "3.20.0".to_string(),
            },
            cpu_architecture: old.cpu_architecture.clone(),
        }
        .diff(&old);

        assert_eq!(
            diff.iter().map(strip_ansi).collect::<Vec<String>>(),
            vec!["OS Distribution (`ubuntu 24.04` to `alpine 3.20.0`)".to_string()]
        );

        let diff = Metadata {
            ruby_version: old.ruby_version.clone(),
            os_distribution: old.os_distribution.clone(),
            cpu_architecture: "arm64".to_string(),
        }
        .diff(&old);

        assert_eq!(
            diff.iter().map(strip_ansi).collect::<Vec<String>>(),
            vec!["CPU Architecture (`amd64` to `arm64`)".to_string()]
        );
    }

    /// If this test fails due to user change you may need
    /// to rev a cache key to force 'bundle install'
    /// to re-run otherwise it won't be picked up by
    /// anyone that is seeing `DiffState::Same`
    #[test]
    fn layer_env_change_keep_guard() {
        let layer_env = layer_env(
            &PathBuf::from("layer_path"),
            &PathBuf::from("app_path"),
            &BundleWithout(String::from("development:test")),
        );

        let env = layer_env.apply(Scope::All, &Env::new());

        let actual = commons::display::env_to_sorted_string(&env);
        let expected = r"
BUNDLE_FROZEN=1
BUNDLE_GEMFILE=app_path/Gemfile
BUNDLE_WITHOUT=development:test
GEM_HOME=layer_path
GEM_PATH=layer_path
        ";
        assert_eq!(expected.trim(), actual.trim());
    }

    /// Guards the current metadata deserialization
    /// If this fails you need to implement a migration from the last format
    /// to the current format.
    #[test]
    fn metadata_guard() {
        let target_id = TargetId::from_stack("heroku-22").unwrap();
        let metadata = Metadata {
            os_distribution: OsDistribution {
                name: target_id.distro_name.clone(),
                version: target_id.distro_version.clone(),
            },
            cpu_architecture: target_id.cpu_architecture,
            ruby_version: ResolvedRubyVersion(String::from("3.1.3")),
        };

        let actual = toml::to_string(&metadata).unwrap();
        let toml_string = r#"
cpu_architecture = "amd64"
ruby_version = "3.1.3"

[os_distribution]
name = "ubuntu"
version = "22.04"
"#
        .trim()
        .to_string();
        assert_eq!(toml_string, actual.trim());

        let deserialized: Metadata = toml::from_str(&toml_string).unwrap();

        assert_eq!(metadata, deserialized);
    }

    #[test]
    fn metadata_migrate_v1_to_v2() {
        let target_id = TargetId::from_stack("heroku-24").unwrap();
        let metadata = MetadataV3 {
            ruby_version: ResolvedRubyVersion(String::from("3.1.3")),
            force_bundle_install_key: String::from("v1"),
            #[allow(deprecated)]
            digest: toml::from_str(r#"
                platform_env = "c571543beaded525b7ee46ceb0b42c0fb7b9f6bfc3a211b3bbcfe6956b69ace3"
                [files]
                "/workspace/Gemfile" = "32b27d2934db61b105fea7c2cb6159092fed6e121f8c72a948f341ab5afaa1ab"
            "#
            )
            .unwrap(),
            os_distribution: OsDistribution {
                name: target_id.distro_name,
                version: target_id.distro_version
            },
            cpu_architecture: "arm64".to_string(),
        };

        let actual = toml::to_string(&metadata).unwrap();
        let toml_string = r#"
cpu_architecture = "arm64"
ruby_version = "3.1.3"
force_bundle_install_key = "v1"

[os_distribution]
name = "ubuntu"
version = "24.04"

[digest]
platform_env = "c571543beaded525b7ee46ceb0b42c0fb7b9f6bfc3a211b3bbcfe6956b69ace3"

[digest.files]
"/workspace/Gemfile" = "32b27d2934db61b105fea7c2cb6159092fed6e121f8c72a948f341ab5afaa1ab"
"#
        .trim()
        .to_string();
        assert_eq!(toml_string, actual.trim());

        let deserialized: MetadataV3 = MetadataV3::try_from_str_migrations(&toml_string)
            .unwrap()
            .unwrap();

        // Cache clear logic for force_bundle_install_key = "v1"
        assert_eq!(
            "Internal gem directory structure changed".to_string(),
            SentenceList::new(&deserialized.diff(&deserialized)).to_string(),
        );
    }
}

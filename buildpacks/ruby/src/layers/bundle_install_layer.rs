use crate::{BundleWithout, RubyBuildpack, RubyBuildpackError};
use commons::output::{
    fmt::{self, HELP},
    section_log::{log_step, log_step_stream, SectionLogger},
};
use commons::{
    display::SentenceList, gemfile_lock::ResolvedRubyVersion, metadata_digest::MetadataDigest,
};
use fun_run::CommandWithName;
use fun_run::{self, CmdError};
use libcnb::{
    build::BuildContext,
    data::layer_content_metadata::LayerTypes,
    layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder},
    layer_env::{LayerEnv, ModificationBehavior, Scope},
    Env,
};
use magic_migrate::{try_migrate_deserializer_chain, TryMigrate};
use serde::{Deserialize, Deserializer, Serialize};
use std::convert::Infallible;
use std::{path::Path, process::Command};

use crate::target_id::{TargetId, TargetIdError};

const HEROKU_SKIP_BUNDLE_DIGEST: &str = "HEROKU_SKIP_BUNDLE_DIGEST";
pub(crate) const FORCE_BUNDLE_INSTALL_CACHE_KEY: &str = "v1";

/// Mostly runs 'bundle install'
///
/// Creates the cache where gems live. We want 'bundle install'
/// to execute on every build (as opposed to only when the cache is empty)
///
/// To help achieve this the logic inside of `BundleInstallLayer::update` and
/// `BundleInstallLayer::create` are the same.
#[derive(Debug)]
pub(crate) struct BundleInstallLayer<'a> {
    pub(crate) env: Env,
    pub(crate) without: BundleWithout,
    pub(crate) _section_log: &'a dyn SectionLogger,
    pub(crate) metadata: BundleInstallLayerMetadata,
}

#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
pub(crate) struct BundleInstallLayerMetadataV1 {
    pub(crate) stack: String,
    pub(crate) ruby_version: ResolvedRubyVersion,
    pub(crate) force_bundle_install_key: String,
    pub(crate) digest: MetadataDigest, // Must be last for serde to be happy https://github.com/toml-rs/toml-rs/issues/142
}

#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
pub(crate) struct BundleInstallLayerMetadataV2 {
    pub(crate) distro_name: String,
    pub(crate) distro_version: String,
    pub(crate) cpu_architecture: String,
    pub(crate) ruby_version: ResolvedRubyVersion,
    pub(crate) force_bundle_install_key: String,

    /// A struct that holds the cryptographic hash of components that can
    /// affect the result of `bundle install`. When these values do not
    /// change between deployments we can skip re-running `bundle install` since
    /// the outcome should not change.
    ///
    /// While a fully resolved `bundle install` is relatively fast, it's not
    /// instantaneous. This check can save ~1 second on overall build time.
    ///
    /// This value is cached with metadata, so changing the struct
    /// may cause metadata to be invalidated (and the cache cleared).
    ///
    pub(crate) digest: MetadataDigest, // Must be last for serde to be happy https://github.com/toml-rs/toml-rs/issues/142
}

try_migrate_deserializer_chain!(
    chain: [BundleInstallLayerMetadataV1, BundleInstallLayerMetadataV2],
    error: MetadataMigrateError,
    deserializer: toml::Deserializer::new,
);
pub(crate) type BundleInstallLayerMetadata = BundleInstallLayerMetadataV2;

#[derive(thiserror::Error, Debug)]
pub(crate) enum MetadataMigrateError {
    #[error("Could not migrate metadata {0}")]
    UnsupportedStack(TargetIdError),
}

// CNB spec moved from the concept of "stacks" (i.e. "heroku-22" which represented an OS and system dependencies) to finer
// grained "target" which includes the OS, OS version, and architecture. This function converts the old stack id to the new target id.
impl TryFrom<BundleInstallLayerMetadataV1> for BundleInstallLayerMetadataV2 {
    type Error = MetadataMigrateError;

    fn try_from(v1: BundleInstallLayerMetadataV1) -> Result<Self, Self::Error> {
        let target_id =
            TargetId::from_stack(&v1.stack).map_err(MetadataMigrateError::UnsupportedStack)?;

        Ok(Self {
            distro_name: target_id.distro_name.clone(),
            distro_version: target_id.distro_version.clone(),
            cpu_architecture: target_id.cpu_architecture.clone(),
            ruby_version: v1.ruby_version,
            force_bundle_install_key: v1.force_bundle_install_key,
            digest: v1.digest,
        })
    }
}

impl<'a> BundleInstallLayer<'a> {
    #[allow(clippy::unnecessary_wraps)]
    fn build_layer_env(
        &self,
        context: &BuildContext<RubyBuildpack>,
        layer_path: &Path,
    ) -> Result<LayerEnv, RubyBuildpackError> {
        let out = layer_env(layer_path, &context.app_dir, &self.without);

        Ok(out)
    }
}

#[derive(Debug)]
enum UpdateState {
    /// Holds message indicating the reason why we want to run 'bundle install'
    Run(String),

    /// Do not run 'bundle install'
    Skip(Vec<String>),
}

/// Determines if 'bundle install' should execute on a given call to `BundleInstallLatyer::update`
///
///
fn update_state(old: &BundleInstallLayerMetadata, now: &BundleInstallLayerMetadata) -> UpdateState {
    let forced_env = std::env::var_os(HEROKU_SKIP_BUNDLE_DIGEST);
    let old_key = &old.force_bundle_install_key;
    let now_key = &now.force_bundle_install_key;

    if old_key != now_key {
        UpdateState::Run(format!(
            "buildpack author triggered internal change {old_key} to {now_key}"
        ))
    } else if let Some(value) = forced_env {
        let value = value.to_string_lossy();

        UpdateState::Run(format!("found {HEROKU_SKIP_BUNDLE_DIGEST}={value}"))
    } else if let Some(changed) = now.digest.changed(&old.digest) {
        UpdateState::Run(format!("{changed}"))
    } else {
        let checked = now.digest.checked_list();
        UpdateState::Skip(checked)
    }
}

impl Layer for BundleInstallLayer<'_> {
    type Buildpack = RubyBuildpack;
    type Metadata = BundleInstallLayerMetadata;

    fn types(&self) -> LayerTypes {
        LayerTypes {
            build: true,
            launch: true,
            cache: true,
        }
    }

    /// Runs with gems cache from last execution
    fn update(
        &mut self,
        context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        let metadata = self.metadata.clone();
        let layer_env = self.build_layer_env(context, &layer_data.path)?;
        let env = layer_env.apply(Scope::Build, &self.env);

        match update_state(&layer_data.content_metadata.metadata, &metadata) {
            UpdateState::Run(reason) => {
                log_step(reason);

                bundle_install(&env).map_err(RubyBuildpackError::BundleInstallCommandError)?;
            }
            UpdateState::Skip(checked) => {
                let bundle_install = fmt::value("bundle install");

                log_step(format!(
                    "Skipping {bundle_install} (no changes found in {sources})",
                    sources = SentenceList::new(&checked).join_str("or")
                ));

                log_step(format!(
                    "{HELP} To force run {bundle_install} set {}",
                    fmt::value(format!("{HEROKU_SKIP_BUNDLE_DIGEST}=1"))
                ));
            }
        }

        LayerResultBuilder::new(metadata).env(layer_env).build()
    }

    /// Runs when with empty cache
    fn create(
        &mut self,
        context: &BuildContext<Self::Buildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        let layer_env = self.build_layer_env(context, layer_path)?;
        let env = layer_env.apply(Scope::Build, &self.env);

        bundle_install(&env).map_err(RubyBuildpackError::BundleInstallCommandError)?;

        LayerResultBuilder::new(self.metadata.clone())
            .env(layer_env)
            .build()
    }

    /// When there is a cache determines if we will run:
    /// - update (keep cache and bundle install)
    /// - recreate (destroy cache and bundle instal)
    ///
    /// CAUTION: We should Should never Keep, this will prevent env vars
    /// if a coder updates env vars they won't be set unless update or
    /// create is run.
    fn existing_layer_strategy(
        &mut self,
        _context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        let old = &layer_data.content_metadata.metadata;
        let now = self.metadata.clone();

        let clear_and_run = Ok(ExistingLayerStrategy::Recreate);
        let keep_and_run = Ok(ExistingLayerStrategy::Update);

        match cache_state(old.clone(), now) {
            Changed::Nothing => {
                log_step("Loading cached gems");

                keep_and_run
            }
            Changed::DistroName(old, now) => {
                log_step(format!(
                    "Clearing cache {}",
                    fmt::details(format!("distro name changed: {old} to {now}"))
                ));

                clear_and_run
            }
            Changed::DistroVersion(old, now) => {
                log_step(format!(
                    "Clearing cache {}",
                    fmt::details(format!("distro version changed: {old} to {now}"))
                ));

                clear_and_run
            }
            Changed::CpuArchitecture(old, now) => {
                log_step(format!(
                    "Clearing cache {}",
                    fmt::details(format!("cpu architecture changed: {old} to {now}"))
                ));

                clear_and_run
            }
            Changed::RubyVersion(old, now) => {
                log_step(format!(
                    "Clearing cache {}",
                    fmt::details(format!("Ruby version changed: {old} to {now}"))
                ));

                clear_and_run
            }
        }
    }

    fn migrate_incompatible_metadata(
        &mut self,
        _context: &BuildContext<Self::Buildpack>,
        metadata: &libcnb::generic::GenericMetadata,
    ) -> Result<
        libcnb::layer::MetadataMigration<Self::Metadata>,
        <Self::Buildpack as libcnb::Buildpack>::Error,
    > {
        match Self::Metadata::try_from_str_migrations(
            &toml::to_string(&metadata).expect("TOML deserialization of GenericMetadata"),
        ) {
            Some(Ok(metadata)) => Ok(libcnb::layer::MetadataMigration::ReplaceMetadata(metadata)),
            Some(Err(e)) => {
                log_step(format!("Clearing cache (metadata migration error {e})"));
                Ok(libcnb::layer::MetadataMigration::RecreateLayer)
            }
            None => {
                log_step("Clearing cache (invalid metadata)");
                Ok(libcnb::layer::MetadataMigration::RecreateLayer)
            }
        }
    }
}

/// The possible states of the cache values, used for determining `ExistingLayerStrategy`
#[derive(Debug)]
enum Changed {
    Nothing,
    DistroName(String, String),
    DistroVersion(String, String),
    CpuArchitecture(String, String),
    RubyVersion(ResolvedRubyVersion, ResolvedRubyVersion),
}

// Compare the old metadata to current metadata to determine the state of the
// cache. Based on that state, we can log and determine `ExistingLayerStrategy`
fn cache_state(old: BundleInstallLayerMetadata, now: BundleInstallLayerMetadata) -> Changed {
    let BundleInstallLayerMetadata {
        distro_name,
        distro_version,
        cpu_architecture,
        ruby_version,
        force_bundle_install_key: _,
        digest: _, // digest state handled elsewhere
    } = now; // ensure all values are handled or we get a clippy warning

    if old.distro_name != distro_name {
        Changed::DistroName(old.distro_name, distro_name)
    } else if old.distro_version != distro_version {
        Changed::DistroVersion(old.distro_version, distro_version)
    } else if old.cpu_architecture != cpu_architecture {
        Changed::CpuArchitecture(old.cpu_architecture, cpu_architecture)
    } else if old.ruby_version != ruby_version {
        Changed::RubyVersion(old.ruby_version, ruby_version)
    } else {
        Changed::Nothing
    }
}

fn layer_env(layer_path: &Path, app_dir: &Path, without_default: &BundleWithout) -> LayerEnv {
    // CAREFUL: See environment variable warning below vvvvvvvvvv
    let layer_env = LayerEnv::new()
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Override,
            "BUNDLE_PATH", // Directs bundler to install gems to this path.
            layer_path,
        )
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Override,
            "BUNDLE_BIN", // Install executables for all gems into specified path.
            layer_path.join("bin"),
        )
        .chainable_insert(Scope::All, ModificationBehavior::Delimiter, "GEM_PATH", ":")
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Prepend,
            "GEM_PATH", // Tells Ruby where gems are located. Should match `BUNDLE_PATH`.
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
            "BUNDLE_CLEAN", // After successful `bundle install` bundler will automatically run `bundle clean`
            "1",
        )
        .chainable_insert(
            Scope::All,
            ModificationBehavior::Override,
            "BUNDLE_DEPLOYMENT", // Requires the `Gemfile.lock` to be in sync with the current `Gemfile`.
            "1",
        );
    // CAREFUL: Changes to these ^^^^^^^ environment variables
    //
    // Not every run is guaranteed to trigger a `bundle_install`
    // Rev the `force_bundle_install` cache key to ensure consistent
    // state (when appropriate).
    layer_env
}

/// Sets the needed environment variables to configure bundler and uses them
/// to execute the `bundle install` command. The results are streamed to stdout/stderr.
///
/// # Errors
///
/// When the 'bundle install' command fails this function returns an error.
///
fn bundle_install(env: &Env) -> Result<(), CmdError> {
    let path_env = env.get("PATH").cloned();
    let display_with_env = |cmd: &'_ mut Command| {
        fun_run::display_with_env_keys(
            cmd,
            env,
            [
                "BUNDLE_BIN",
                "BUNDLE_CLEAN",
                "BUNDLE_DEPLOYMENT",
                "BUNDLE_GEMFILE",
                "BUNDLE_PATH",
                "BUNDLE_WITHOUT",
            ],
        )
    };

    // ## Run `$ bundle install`
    let mut cmd = Command::new("bundle");
    cmd.env_clear() // Current process env vars already merged into env
        .args(["install"])
        .envs(env);

    let mut cmd = cmd.named_fn(display_with_env);

    log_step_stream(format!("Running {}", fmt::command(cmd.name())), |stream| {
        cmd.stream_output(stream.io(), stream.io())
    })
    .map_err(|error| fun_run::map_which_problem(error, cmd.mut_cmd(), path_env))?;

    Ok(())
}

#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq, Default)]
pub(crate) struct BundleDigest {
    env: String,
    gemfile: String,
    lockfile: String,
}

#[cfg(test)]
mod test {
    use super::*;
    use std::path::PathBuf;

    #[cfg(test)]
    #[derive(Default, Clone)]
    struct FakeContext {
        app_path: PathBuf,
        platform: FakePlatform,
    }

    #[cfg(test)]
    #[derive(Default, Clone)]
    struct FakePlatform {
        env: libcnb::Env,
    }

    impl libcnb::Platform for FakePlatform {
        fn env(&self) -> &Env {
            &self.env
        }

        fn from_path(_platform_dir: impl AsRef<Path>) -> std::io::Result<Self> {
            unimplemented!()
        }
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
BUNDLE_BIN=layer_path/bin
BUNDLE_CLEAN=1
BUNDLE_DEPLOYMENT=1
BUNDLE_GEMFILE=app_path/Gemfile
BUNDLE_PATH=layer_path
BUNDLE_WITHOUT=development:test
GEM_PATH=layer_path
        ";
        assert_eq!(expected.trim(), actual.trim());
    }

    /// Guards the current metadata deserialization
    /// If this fails you need to implement a migration from the last format
    /// to the current format.
    #[test]
    fn metadata_guard() {
        let tmpdir = tempfile::tempdir().unwrap();
        let app_path = tmpdir.path().to_path_buf();
        let gemfile = app_path.join("Gemfile");

        let mut env = Env::new();
        env.insert("SECRET_KEY_BASE", "abcdgoldfish");

        let context = FakeContext {
            platform: FakePlatform { env },
            app_path,
        };
        std::fs::write(&gemfile, "iamagemfile").unwrap();

        let target_id = TargetId::from_stack("heroku-22").unwrap();
        let metadata = BundleInstallLayerMetadata {
            distro_name: target_id.distro_name,
            distro_version: target_id.distro_version,
            cpu_architecture: target_id.cpu_architecture,
            ruby_version: ResolvedRubyVersion(String::from("3.1.3")),
            force_bundle_install_key: String::from("v1"),
            digest: MetadataDigest::new_env_files(
                &context.platform,
                &[&context.app_path.join("Gemfile")],
            )
            .unwrap(),
        };

        let actual = toml::to_string(&metadata).unwrap();
        let gemfile_path = gemfile.display();
        let toml_string = format!(
            r#"
distro_name = "ubuntu"
distro_version = "22.04"
cpu_architecture = "amd64"
ruby_version = "3.1.3"
force_bundle_install_key = "v1"

[digest]
platform_env = "c571543beaded525b7ee46ceb0b42c0fb7b9f6bfc3a211b3bbcfe6956b69ace3"

[digest.files]
"{gemfile_path}" = "32b27d2934db61b105fea7c2cb6159092fed6e121f8c72a948f341ab5afaa1ab"
"#
        )
        .trim()
        .to_string();
        assert_eq!(toml_string, actual.trim());

        let deserialized: BundleInstallLayerMetadata = toml::from_str(&toml_string).unwrap();

        assert_eq!(metadata, deserialized);
    }

    #[test]
    fn metadata_migrate_v1_to_v2() {
        let tmpdir = tempfile::tempdir().unwrap();
        let app_path = tmpdir.path().to_path_buf();
        let gemfile = app_path.join("Gemfile");

        let mut env = Env::new();
        env.insert("SECRET_KEY_BASE", "abcdgoldfish");

        let context = FakeContext {
            platform: FakePlatform { env },
            app_path,
        };
        std::fs::write(&gemfile, "iamagemfile").unwrap();

        let metadata = BundleInstallLayerMetadataV1 {
            stack: String::from("heroku-22"),
            ruby_version: ResolvedRubyVersion(String::from("3.1.3")),
            force_bundle_install_key: String::from("v1"),
            digest: MetadataDigest::new_env_files(
                &context.platform,
                &[&context.app_path.join("Gemfile")],
            )
            .unwrap(),
        };

        let actual = toml::to_string(&metadata).unwrap();
        let gemfile_path = gemfile.display();
        let toml_string = format!(
            r#"
stack = "heroku-22"
ruby_version = "3.1.3"
force_bundle_install_key = "v1"

[digest]
platform_env = "c571543beaded525b7ee46ceb0b42c0fb7b9f6bfc3a211b3bbcfe6956b69ace3"

[digest.files]
"{gemfile_path}" = "32b27d2934db61b105fea7c2cb6159092fed6e121f8c72a948f341ab5afaa1ab"
"#
        )
        .trim()
        .to_string();
        assert_eq!(toml_string, actual.trim());

        let deserialized: BundleInstallLayerMetadataV2 =
            BundleInstallLayerMetadataV2::try_from_str_migrations(&toml_string)
                .unwrap()
                .unwrap();

        let target_id = TargetId::from_stack(&metadata.stack).unwrap();
        let expected = BundleInstallLayerMetadataV2 {
            distro_name: target_id.distro_name,
            distro_version: target_id.distro_version,
            cpu_architecture: target_id.cpu_architecture,
            ruby_version: metadata.ruby_version,
            force_bundle_install_key: metadata.force_bundle_install_key,
            digest: metadata.digest,
        };
        assert_eq!(expected, deserialized);
    }
}

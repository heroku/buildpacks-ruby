use crate::{BundleWithout, RubyBuildpack, RubyBuildpackError};
use commons::fun_run::CommandWithName;
use commons::output::fmt::{self, HELP};
use commons::output::interface::SectionLogger;
use commons::output::section_log as log;
use commons::{
    display::SentenceList,
    fun_run::{self, CmdError},
    gemfile_lock::ResolvedRubyVersion,
    metadata_digest::MetadataDigest,
};
use libcnb::{
    build::BuildContext,
    data::{buildpack::StackId, layer_content_metadata::LayerTypes},
    layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder},
    layer_env::{LayerEnv, ModificationBehavior, Scope},
    Env,
};
use serde::{Deserialize, Serialize};
use std::{path::Path, process::Command};

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
    pub env: Env,
    pub without: BundleWithout,
    pub _section_log: &'a dyn SectionLogger,
    pub metadata: BundleInstallLayerMetadata,
}

#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
pub(crate) struct BundleInstallLayerMetadata {
    pub stack: StackId,
    pub ruby_version: ResolvedRubyVersion,
    pub force_bundle_install_key: String,

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
    pub digest: MetadataDigest, // Must be last for serde to be happy https://github.com/toml-rs/toml-rs/issues/142
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
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        let metadata = self.metadata.clone();
        let layer_env = self.build_layer_env(context, &layer_data.path)?;
        let env = layer_env.apply(Scope::Build, &self.env);

        match update_state(&layer_data.content_metadata.metadata, &metadata) {
            UpdateState::Run(reason) => {
                log::step(reason);

                bundle_install(&env).map_err(RubyBuildpackError::BundleInstallCommandError)?;
            }
            UpdateState::Skip(checked) => {
                let checked = SentenceList::new(&checked).join_str("or");
                let bundle_install = fmt::value("bundle install");
                let env_var = fmt::value(format!("{HEROKU_SKIP_BUNDLE_DIGEST}=1"));

                log::step(format!(
                    "Skipping {bundle_install} {}",
                    fmt::details(format!("no changes found in {checked}"))
                ));

                log::step(format!(
                    "{HELP} To force run {bundle_install} set {env_var}",
                ));
            }
        }

        LayerResultBuilder::new(metadata).env(layer_env).build()
    }

    /// Runs when with empty cache
    fn create(
        &self,
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
        &self,
        _context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        let old = &layer_data.content_metadata.metadata;
        let now = self.metadata.clone();

        let clear_and_run = Ok(ExistingLayerStrategy::Recreate);
        let keep_and_run = Ok(ExistingLayerStrategy::Update);

        match cache_state(old.clone(), now) {
            Changed::Nothing => {
                log::step("Loading cache");

                keep_and_run
            }
            Changed::Stack(_old, _now) => {
                log::step(format!("Clearing cache {}", fmt::details("stack changed")));

                clear_and_run
            }
            Changed::RubyVersion(_old, _now) => {
                log::step(format!(
                    "Clearing cache {}",
                    fmt::details("ruby version changed")
                ));

                clear_and_run
            }
        }
    }
}

/// The possible states of the cache values, used for determining `ExistingLayerStrategy`
#[derive(Debug)]
enum Changed {
    Nothing,

    /// The stack changed i.e. from heroku_20 to heroku_22
    /// When that happens we must invalidate native dependency gems
    /// because they're compiled against system dependencies
    /// i.e. https://devcenter.heroku.com/articles/stack-packages
    /// TODO: Only clear native dependencies instead of the whole cache
    Stack(StackId, StackId), // (old, now)

    /// Ruby version changed i.e. 3.0.2 to 3.1.2
    /// When that happens we must invalidate native dependency gems
    /// because they're linked to a specific compiled version of Ruby.
    /// TODO: Only clear native dependencies instead of the whole cache
    RubyVersion(ResolvedRubyVersion, ResolvedRubyVersion), // (old, now)
}

// Compare the old metadata to current metadata to determine the state of the
// cache. Based on that state, we can log and determine `ExistingLayerStrategy`
fn cache_state(old: BundleInstallLayerMetadata, now: BundleInstallLayerMetadata) -> Changed {
    let BundleInstallLayerMetadata {
        stack,
        ruby_version,
        force_bundle_install_key: _,
        digest: _, // digest state handled elsewhere
    } = now; // ensure all values are handled or we get a clippy warning

    if old.stack != stack {
        Changed::Stack(old.stack, stack)
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

    log::step_stream(format!("Running {}", fmt::command(cmd.name())), |stream| {
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
    use libcnb::data::stack_id;
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

    /// If this test fails due to a change you'll need to implement
    /// `migrate_incompatible_metadata` for the Layer trait
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

        let metadata = BundleInstallLayerMetadata {
            stack: stack_id!("heroku-22"),
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

        let deserialized: BundleInstallLayerMetadata = toml::from_str(&toml_string).unwrap();

        assert_eq!(metadata, deserialized);
    }
}

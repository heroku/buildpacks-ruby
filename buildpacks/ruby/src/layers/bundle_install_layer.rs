use crate::{BundleWithout, RubyBuildpack, RubyBuildpackError};
use commons::{
    env_command::{CommandError, EnvCommand},
    gemfile_lock::ResolvedRubyVersion,
};
use libcnb::{
    build::BuildContext,
    data::{buildpack::StackId, layer_content_metadata::LayerTypes},
    layer::{ExistingLayerStrategy, Layer, LayerData, LayerResult, LayerResultBuilder},
    layer_env::{LayerEnv, ModificationBehavior, Scope},
    Env, Platform,
};
use libherokubuildpack::log as user;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::{ffi::OsString, path::Path};

const HEROKU_SKIP_BUNDLE_DIGEST: &str = "HEROKU_SKIP_BUNDLE_DIGEST";
const FORCE_BUNDLE_INSTALL_CACHE_KEY: &str = "v1";

/// Mostly runs 'bundle install'
///
/// Creates the cache where gems live. We want 'bundle install'
/// to execute on every build (as opposed to only when the cache is empty)
///
/// To help achieve this the logic inside of `BundleInstallLayer::update` and
/// `BundleInstallLayer::create` are the same.
#[derive(Debug)]
pub(crate) struct BundleInstallLayer {
    pub env: Env,
    pub without: BundleWithout,
    pub ruby_version: ResolvedRubyVersion,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct BundleInstallLayerMetadata {
    stack: StackId,
    ruby_version: ResolvedRubyVersion,
    force_bundle_install_key: String,
    // Must be last for serde to be happy https://github.com/toml-rs/toml-rs/issues/142
    digest: BundleDigest,
}

impl BundleInstallLayer {
    /// Run every time with different diff states
    fn run_on_diff(
        &self,
        state: DiffState,
        _layer_path: &Path,
        layer_env: LayerEnv,
        metadata: BundleInstallLayerMetadata,
    ) -> Result<LayerResult<BundleInstallLayerMetadata>, RubyBuildpackError> {
        let env = layer_env.apply(Scope::Build, &self.env);
        match state {
            DiffState::None => {
                user::log_info("Running 'bundle install'");

                bundle_install(&env).map_err(RubyBuildpackError::BundleInstallCommandError)?;
            }
            DiffState::Forced(message) => {
                user::log_info(format!("Running 'bundle install', {message}"));

                bundle_install(&env).map_err(RubyBuildpackError::BundleInstallCommandError)?;
            }
            DiffState::Different(values) => {
                let changes = values.join(", ");
                user::log_info(format!(
                    "Running 'bundle install', found changes since last deploy: {changes}"
                ));

                bundle_install(&env).map_err(RubyBuildpackError::BundleInstallCommandError)?;
            }
            DiffState::Same(names) => {
                let checked = names.join(", ");
                user::log_info(format!(
                    "Skipping 'bundle install', no changes found in {checked}"
                ));
                user::log_info("Help: To skip digest change detection and force running");
                user::log_info(format!(
                    "      'bundle install' set {HEROKU_SKIP_BUNDLE_DIGEST}=1"
                ));
            }
        };

        LayerResultBuilder::new(metadata).env(layer_env).build()
    }

    fn build_metadata(
        &self,
        context: &BuildContext<RubyBuildpack>,
        _layer_path: &Path,
    ) -> Result<BundleInstallLayerMetadata, RubyBuildpackError> {
        let digest = BundleDigest::new(&context.app_dir, context.platform.env())
            .map_err(RubyBuildpackError::BundleInstallDigestError)?;
        let stack = context.stack_id.clone();
        let ruby_version = self.ruby_version.clone();
        let force_bundle_install_key = String::from(FORCE_BUNDLE_INSTALL_CACHE_KEY);

        Ok(BundleInstallLayerMetadata {
            stack,
            ruby_version,
            force_bundle_install_key,
            digest,
        })
    }

    #[allow(clippy::unnecessary_wraps)]
    fn build_layer_env(
        &self,
        context: &BuildContext<RubyBuildpack>,
        layer_path: &Path,
    ) -> Result<LayerEnv, RubyBuildpackError> {
        let out = layer_env(layer_path, &context.app_dir, &self.without);

        Ok(out)
    }

    /// Returns Some(String) if a `bundle install` has been forced
    /// where String contains the message for why.
    fn force_digest(
        old: &BundleInstallLayerMetadata,
        now: &BundleInstallLayerMetadata,
    ) -> Option<String> {
        let forced_env = std::env::var_os(HEROKU_SKIP_BUNDLE_DIGEST);

        if now.force_bundle_install_key != old.force_bundle_install_key {
            Some(String::from("buildpack author triggered internal change"))
        } else if let Some(value) = forced_env {
            let value = value.to_string_lossy();
            Some(format!("found {HEROKU_SKIP_BUNDLE_DIGEST}={value}"))
        } else {
            None
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    fn existing_cache_strategy(
        old: BundleInstallLayerMetadata,
        now: BundleInstallLayerMetadata,
    ) -> Result<CacheStrategy, RubyBuildpackError> {
        user::log_info("Found gems cache");
        match cache_state(old, now) {
            Changed::Nothing => {
                user::log_info("Found gems cache");

                Ok(CacheStrategy::KeepAndRun)
            }
            Changed::Stack(old, now) => {
                user::log_info(format!(
                    "Clearing gems cache, Stack changed from {old} to {now}"
                ));

                Ok(CacheStrategy::ClearAndRun)
            }
            Changed::RubyVersion(old, now) => {
                user::log_info(format!(
                    "Clearing gems cache, ruby version changed from {old} to {now}"
                ));

                Ok(CacheStrategy::ClearAndRun)
            }
        }
    }
}

#[derive(Debug)]
enum CacheStrategy {
    ClearAndRun,
    KeepAndRun,
}

impl Layer for BundleInstallLayer {
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
        let old = &layer_data.content_metadata.metadata;
        let layer_path = &layer_data.path;

        let metadata = self.build_metadata(context, layer_path)?;
        let layer_env = self.build_layer_env(context, layer_path)?;

        let diff = digest_state(Self::force_digest, Some(old), &metadata);
        self.run_on_diff(diff, layer_path, layer_env, metadata)
    }

    /// Runs when with empty cache
    fn create(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        let metadata = self.build_metadata(context, layer_path)?;
        let layer_env = self.build_layer_env(context, layer_path)?;
        let diff = digest_state(Self::force_digest, None, &metadata);
        self.run_on_diff(diff, layer_path, layer_env, metadata)
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
        context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        let old = &layer_data.content_metadata.metadata;
        let now = self.build_metadata(context, &layer_data.path)?;

        match Self::existing_cache_strategy(old.clone(), now)? {
            CacheStrategy::ClearAndRun => Ok(ExistingLayerStrategy::Recreate),
            CacheStrategy::KeepAndRun => Ok(ExistingLayerStrategy::Update),
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
fn bundle_install(env: &Env) -> Result<(), CommandError> {
    // ## Run `$ bundle install`
    let command = EnvCommand::new_show_keys(
        "bundle",
        &["install"],
        env,
        [
            "BUNDLE_BIN",
            "BUNDLE_CLEAN",
            "BUNDLE_DEPLOYMENT",
            "BUNDLE_GEMFILE",
            "BUNDLE_PATH",
            "BUNDLE_WITHOUT",
        ],
    );

    user::log_info(format!("\nRunning command:\n$ {command}"));

    command.stream()?;

    Ok(())
}

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
#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq, Default)]
pub(crate) struct BundleDigest {
    env: String,
    gemfile: String,
    lockfile: String,
}

#[derive(Debug)]
enum DiffState {
    /// Old digest did not exist, either this is the first time
    /// the layer is being run, or the cache was cleared and it
    /// is re-running.
    None,

    /// New and old digest are the same, vec contains names checked
    Same(Vec<String>),

    /// Difference check was skipped. String contains message for why
    Forced(String),

    /// New and old digest are different, vec contains names that don't match
    Different(Vec<String>),
}

/// Returns state of digest between runs of the buildpack
fn digest_state<F>(
    forced_fn: F,
    option_old: Option<&BundleInstallLayerMetadata>,
    now: &BundleInstallLayerMetadata,
) -> DiffState
where
    F: Fn(&BundleInstallLayerMetadata, &BundleInstallLayerMetadata) -> Option<String>,
{
    if let Some(old) = option_old {
        if let Some(message) = forced_fn(old, now) {
            DiffState::Forced(message)
        } else if let Some(diff) = now.digest.diff(&old.digest) {
            DiffState::Different(diff)
        } else {
            let names = now.digest.checked_names();
            DiffState::Same(names)
        }
    } else {
        DiffState::None
    }
}

impl BundleDigest {
    fn new(app_path: &Path, env: &Env) -> Result<BundleDigest, std::io::Error> {
        let gemfile = fs_err::read_to_string(app_path.join("Gemfile"))?;
        let lockfile = fs_err::read_to_string(app_path.join("Gemfile.lock"))?;

        Ok(BundleDigest {
            env: env_hash(env),
            gemfile: hash_from_string(&gemfile),
            lockfile: hash_from_string(&lockfile),
        })
    }

    /// Lists the differencs between two digest as a series of
    /// tuples. As (name, now == old). This is later (ab)used
    /// in order to output the names of all values checked.
    ///
    /// Since we use destructuring to ensure all values are removed
    /// we can ensure that all names are represented
    fn diff_tuples(&self, old: &BundleDigest) -> [(String, bool); 3] {
        let BundleDigest {
            env,
            gemfile,
            lockfile,
        } = self; // Ensure all fields are used or we get a clippy warning

        [
            (String::from("Gemfile"), gemfile != &old.gemfile),
            (String::from("Gemfile.lock"), lockfile != &old.lockfile),
            (
                String::from("user configured Environment variables"),
                env != &old.env,
            ),
        ]
    }

    /// Lists out all checked value names.
    fn checked_names(&self) -> Vec<String> {
        let old = BundleDigest::default();
        self.diff_tuples(&old)
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<String>>()
    }

    /// Returns Some() if differences are detected, otherwise None
    /// the contents of the string vector represent the names that
    /// are different between the two digests
    fn diff(&self, old: &BundleDigest) -> Option<Vec<String>> {
        let diff = self
            .diff_tuples(old)
            .iter()
            .filter_map(|(name, diff)| diff.then_some(name.clone()))
            .collect::<Vec<String>>();

        if diff.is_empty() {
            None
        } else {
            Some(diff)
        }
    }
}

/// Hashing helper function, give it a str and it gives you the SHA256 hash back
/// out as a string
fn hash_from_string(str: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(str);
    format!("{:x}", hasher.finalize())
}

fn env_to_string(env: &Env) -> String {
    let mut env = env
        .into_iter()
        .map(|(a, b)| (a.clone(), b.clone()))
        .collect::<Vec<(OsString, OsString)>>();

    env.sort_by(|(a, _), (b, _)| a.cmp(b));

    env.iter()
        .map(|(key, value)| {
            let mut out = OsString::new();
            out.push(key);
            out.push(OsString::from("="));
            out.push(value);
            out.to_string_lossy() // UTF-8 values see no degradation, otherwise we should be comparing equivalent strings.
                .to_string()
        })
        .collect::<Vec<String>>()
        .join("\n")
}

/// Hashing helper function, give it an Env and it gives you the SHA256 hash back
/// out as a string.
fn env_hash(env: &Env) -> String {
    let env_string = env_to_string(env);
    hash_from_string(&env_string)
}

#[cfg(test)]
mod test {
    use super::*;
    use libcnb::data::stack_id;
    use std::path::PathBuf;

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

        let actual = env_to_string(&env);
        let expected = r#"
BUNDLE_BIN=layer_path/bin
BUNDLE_CLEAN=1
BUNDLE_DEPLOYMENT=1
BUNDLE_GEMFILE=app_path/Gemfile
BUNDLE_PATH=layer_path
BUNDLE_WITHOUT=development:test
GEM_PATH=layer_path
        "#;
        assert_eq!(expected.trim(), actual.trim());
    }

    /// If this test fails due to a change you'll need to implement
    /// `migrate_incompatible_metadata` for the Layer trait
    #[test]
    fn metadata_guard() {
        let metadata = BundleInstallLayerMetadata {
            stack: stack_id!("heroku-22"),
            ruby_version: ResolvedRubyVersion(String::from("3.1.3")),
            force_bundle_install_key: String::from("v1"),
            digest: BundleDigest::default(),
        };

        let actual = toml::to_string(&metadata).unwrap();
        let expected = r#"
stack = "heroku-22"
ruby_version = "3.1.3"
force_bundle_install_key = "v1"

[digest]
env = ""
gemfile = ""
lockfile = ""
"#
        .trim();
        assert_eq!(expected, actual.trim());
    }

    #[test]
    fn test_diff_from_dir_env() {
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path();
        let env = Env::new();

        fs_err::write(dir.join("Gemfile"), "lol").unwrap();
        fs_err::write(dir.join("Gemfile.lock"), "lol").unwrap();

        let current = BundleDigest::new(dir, &env).unwrap();

        let old = BundleDigest::default();
        assert!(current.diff(&old).is_some());
        assert!(current.diff(&current).is_none());
    }

    #[test]
    fn test_diff_env() {
        let current = BundleDigest {
            env: String::from("lol"),
            ..BundleDigest::default()
        };

        let old = BundleDigest::default();
        assert_eq!(
            current.diff(&old),
            Some(vec![String::from("user configured Environment variables")])
        );
    }

    #[test]
    fn test_bundle_digest_the_same() {
        let current = BundleDigest::default();
        let old = BundleDigest::default();
        assert_eq!(current.diff(&old), None);
    }
}

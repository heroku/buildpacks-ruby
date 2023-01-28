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

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct BundleInstallLayerMetadata {
    stack: StackId,
    without: BundleWithout,
    ruby_version: ResolvedRubyVersion,
    force_bundle_install_key: String,
    force_cache_clear_bundle_install_key: String,
    // Must be last for serde to be happy https://github.com/toml-rs/toml-rs/issues/142
    digest: BundleDigest,
}

#[derive(Debug)]
pub(crate) struct BundleInstallLayer {
    pub env: Env,
    pub without: BundleWithout,
    pub ruby_version: ResolvedRubyVersion,
}

impl BundleInstallLayer {
    /// This value is saved in the cache, if you need to force a re-run of `bundle install`
    /// (without busting/deleting the gem cache), update this value
    #[allow(clippy::unused_self)]
    fn force_bundle_install_key(&self) -> String {
        String::from("v1")
    }

    /// This value is saved in the cache, if you ned to force a re-build and also
    /// bust the cache, update this value. To rebuild without busting the cache, update the
    /// `update_cache_key`
    #[allow(clippy::unused_self)]
    fn force_cache_clear_bundle_install_key(&self) -> String {
        String::from("v1")
    }

    fn build_metadata(
        &self,
        digest: BundleDigest,
        context: &BuildContext<RubyBuildpack>,
        _layer_path: &Path,
    ) -> BundleInstallLayerMetadata {
        let stack = context.stack_id.clone();
        let force_bundle_install_key = self.force_bundle_install_key();
        let force_cache_clear_bundle_install_key = self.force_cache_clear_bundle_install_key();
        let without = self.without.clone();
        let ruby_version = self.ruby_version.clone();

        BundleInstallLayerMetadata {
            stack,
            without,
            ruby_version,
            force_bundle_install_key,
            force_cache_clear_bundle_install_key,
            digest,
        }
    }

    /// Entrypoint for both update and create
    ///
    /// The `bundle install` command is run on every deploy except where `BundleDigest` determines we can
    /// skip it (based on the contents of the environment variables, Gemfile, Gemfile.lock, etc.)
    ///
    fn update_and_create(
        &self,
        context: &BuildContext<RubyBuildpack>,
        layer_path: &Path,
    ) -> Result<LayerResult<BundleInstallLayerMetadata>, RubyBuildpackError> {
        let digest = BundleDigest::new(&context.app_dir, context.platform.env())
            .map_err(RubyBuildpackError::BundleInstallDigestError)?;
        let layer_env = bundle_install(layer_path, &context.app_dir, &self.without, &self.env)
            .map_err(RubyBuildpackError::BundleInstallCommandError)?;

        LayerResultBuilder::new(self.build_metadata(digest, context, layer_path))
            .env(layer_env)
            .build()
    }
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

    fn update(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        self.update_and_create(context, &layer_data.path)
    }

    fn create(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_path: &std::path::Path,
    ) -> Result<LayerResult<Self::Metadata>, RubyBuildpackError> {
        self.update_and_create(context, layer_path)
    }

    /// We want `bundle install` to have the opportunity to run on every deployment even
    /// if the cache is good. Therefore we should never return
    fn existing_layer_strategy(
        &self,
        context: &BuildContext<Self::Buildpack>,
        layer_data: &LayerData<Self::Metadata>,
    ) -> Result<ExistingLayerStrategy, RubyBuildpackError> {
        let digest = BundleDigest::new(&context.app_dir, context.platform.env())
            .map_err(RubyBuildpackError::BundleInstallDigestError)?;
        let old = &layer_data.content_metadata.metadata;
        let now = self.build_metadata(digest, context, &layer_data.path);

        user::log_info("Found gems cache");
        match cache_state(old.clone(), now) {
            Changed::Nothing(names) => {
                user::log_info(format!(
                    "Skipping 'bundle install', no digest changes detected in: {}",
                    names.join(", ")
                ));
                user::log_info("Help: To skip digest change detection and force running");
                user::log_info("      'bundle install' set HEROKU_SKIP_BUNDLE_DIGEST=1");

                Ok(ExistingLayerStrategy::Keep)
            }
            Changed::UserForceInstallWithCache(message) => {
                user::log_info(format!("Running 'bundle install' with cache, {message}"));

                Ok(ExistingLayerStrategy::Update)
            }
            Changed::Digest(diff) => {
                user::log_info(format!(
                    "Running 'bundle install' with cache, digest changes detected: {}",
                    diff.join(", ")
                ));

                Ok(ExistingLayerStrategy::Update)
            }
            Changed::Without(old, current) => {
                user::log_info(format!(
                    "Running 'bundle install', with cache BUNDLE_WITHOUT changed from {old} to {current}"
                ));

                Ok(ExistingLayerStrategy::Update)
            }
            Changed::BuildpackForceInstallWithCache => {
                user::log_info(
                    "Running 'bundle install' with cache, due to buildpack author triggered change",
                );

                Ok(ExistingLayerStrategy::Update)
            }
            Changed::BuildpackForceCacheClear => {
                user::log_info(
                    "Clearing gems from cache, due to buildpack author triggered change",
                );

                Ok(ExistingLayerStrategy::Recreate)
            }
            Changed::Stack(old, current) => {
                user::log_info(format!(
                    "Clearing gems cache, Stack changed from {old} to {current}"
                ));

                Ok(ExistingLayerStrategy::Recreate)
            }
            Changed::RubyVersion(old, current) => {
                user::log_info(format!(
                    "Clearing gems cache, ruby version changed from {old} to {current}"
                ));

                Ok(ExistingLayerStrategy::Recreate)
            }
        }
    }
}

/// The possible states of the cache values, used for determining `ExistingLayerStrategy`
#[derive(Debug)]
enum Changed {
    /// BUNDLE_WITHOUT changed
    Without(String, String), // (old, now)

    /// Nothing changed, vector contains values that were checked i.e. Gemfile, Gemfile.lock, etc.
    Nothing(Vec<String>),

    /// The `BundleDigest` changed. Lists the value that differed between old and now
    Digest(Vec<String>),

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

    /// A user has set the env var `HEROKU_SKIP_BUNDLE_DIGEST`
    /// to indicate they want to bypass the digest
    UserForceInstallWithCache(String), // message

    /// A buildpack developer changed the static cache key in code to force
    /// a `bundle install` with the existing cache intact
    BuildpackForceInstallWithCache,

    /// A buildpack developer changed the static cache key in code to force
    /// a `bundle install` after the cache has been cleared.
    BuildpackForceCacheClear,
}

// Compare the old metadata to current metadata to determine the state of the
// cache. Based on that state, we can log and determine `ExistingLayerStrategy`
fn cache_state(old: BundleInstallLayerMetadata, now: BundleInstallLayerMetadata) -> Changed {
    let BundleInstallLayerMetadata {
        stack,
        without,
        ruby_version,
        force_bundle_install_key,
        force_cache_clear_bundle_install_key,
        digest,
    } = now; // ensure all values are used or we get a clippy warning
    let heroku_skip_digest = std::env::var_os("HEROKU_SKIP_BUNDLE_DIGEST");

    if old.stack != stack {
        Changed::Stack(old.stack, stack)
    } else if old.ruby_version != ruby_version {
        Changed::RubyVersion(old.ruby_version, ruby_version)
    } else if old.without != without {
        Changed::Without(old.without.0, without.0)
    } else if old.force_cache_clear_bundle_install_key != force_cache_clear_bundle_install_key {
        Changed::BuildpackForceCacheClear
    } else if old.force_bundle_install_key != force_bundle_install_key {
        Changed::BuildpackForceInstallWithCache
    } else if let Some(value) = heroku_skip_digest {
        Changed::UserForceInstallWithCache(format!(
            "found HEROKU_SKIP_BUNDLE_DIGEST={}",
            value.to_string_lossy()
        ))
    } else if let Some(diff) = digest.diff(&old.digest) {
        Changed::Digest(diff)
    } else {
        let checked = digest.checked_names();
        Changed::Nothing(checked)
    }
}

/// Executes the `bundle install` command and streams the results to stdout/stderr
fn bundle_install(
    layer_path: &Path,
    app_dir: &Path,
    without_default: &BundleWithout,
    env: &Env,
) -> Result<LayerEnv, CommandError> {
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
            ModificationBehavior::Append,
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
    // are not guaranteed to apply for `ExistingStrategy::Keep` cases.
    //
    // Rev the `force_bundle_install` cache key to ensure consistent
    // state (when appropriate).
    let env = layer_env.apply(Scope::Build, env);

    // ## Run `$ bundle install`
    let command = EnvCommand::new_show_keys(
        "bundle",
        &["install"],
        &env,
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

    Ok(layer_env)
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

    fn diff_tuples(&self, old: &BundleDigest) -> [(String, bool); 3] {
        [
            (String::from("Gemfile"), self.gemfile != old.gemfile),
            (String::from("Gemfile.lock"), self.lockfile != old.lockfile),
            (
                String::from("user configured Environment variables"),
                self.env != old.env,
            ),
        ]
    }

    fn checked_names(&self) -> Vec<String> {
        let old = BundleDigest::default();
        self.diff_tuples(&old)
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<String>>()
    }

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

/// Hashing helper function, give it an Env and it gives you the SHA256 hash back
/// out as a string.
fn env_hash(env: &Env) -> String {
    let mut env = env
        .into_iter()
        .map(|(a, b)| (a.clone(), b.clone()))
        .collect::<Vec<(OsString, OsString)>>();

    env.sort_by(|(a, _), (b, _)| a.cmp(b));

    let env_string = env
        .iter()
        .map(|(key, value)| {
            let mut out = OsString::new();
            out.push(key);
            out.push(OsString::from("="));
            out.push(value);
            out.to_string_lossy() // UTF-8 values see no degradation, otherwise we should be comparing equivalent strings.
                .to_string()
        })
        .collect::<Vec<String>>()
        .join("\n");

    hash_from_string(&env_string)
}

#[cfg(test)]
mod test {
    use super::*;

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

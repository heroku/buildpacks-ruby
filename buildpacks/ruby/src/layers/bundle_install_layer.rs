use crate::{BundleWithout, RubyBuildpack, RubyBuildpackError};
use commons::{
    env_command::{CommandError, EnvCommand},
    gemfile_lock::ResolvedRubyVersion,
};
use libcnb::Platform;
use libcnb::{
    build::BuildContext,
    data::{buildpack::StackId, layer_content_metadata::LayerTypes},
    layer::{ExistingLayerStrategy, Layer, LayerData, LayerResultBuilder},
    layer_env::{LayerEnv, ModificationBehavior, Scope},
    Env,
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
    /// Entrypoint for both update and create
    ///
    /// The `bundle install` command is run on every deploy except where `BundleDigest` determines we can
    /// skip it (based on the contents of the environment variables, Gemfile, Gemfile.lock, etc.)
    ///
    fn update_and_create(
        &self,
        context: &BuildContext<RubyBuildpack>,
        layer_path: &Path,
    ) -> Result<
        libcnb::layer::LayerResult<BundleInstallLayerMetadata>,
        <RubyBuildpack as libcnb::Buildpack>::Error,
    > {
        let digest = BundleDigest::new(&context.app_dir, context.platform.env())
            .map_err(RubyBuildpackError::BundleInstallDigestError)?;
        let layer_env = bundle_install(layer_path, &context.app_dir, &self.without, &self.env)
            .map_err(RubyBuildpackError::BundleInstallCommandError)?;

        LayerResultBuilder::new(BundleInstallLayerMetadata {
            stack: context.stack_id.clone(),
            digest,
            without: self.without.clone(),
            ruby_version: self.ruby_version.clone(),
        })
        .env(layer_env)
        .build()
    }
}

impl Layer for BundleInstallLayer {
    type Buildpack = RubyBuildpack;
    type Metadata = BundleInstallLayerMetadata;

    fn types(&self) -> libcnb::data::layer_content_metadata::LayerTypes {
        LayerTypes {
            build: true,
            launch: true,
            cache: true,
        }
    }

    fn update(
        &self,
        context: &libcnb::build::BuildContext<Self::Buildpack>,
        layer_data: &libcnb::layer::LayerData<Self::Metadata>,
    ) -> Result<
        libcnb::layer::LayerResult<Self::Metadata>,
        <Self::Buildpack as libcnb::Buildpack>::Error,
    > {
        self.update_and_create(context, &layer_data.path)
    }

    fn create(
        &self,
        context: &libcnb::build::BuildContext<Self::Buildpack>,
        layer_path: &std::path::Path,
    ) -> Result<
        libcnb::layer::LayerResult<Self::Metadata>,
        <Self::Buildpack as libcnb::Buildpack>::Error,
    > {
        self.update_and_create(context, layer_path)
    }

    /// We want `bundle install` to have the opportunity to run on every deployment even
    /// if the cache is good. Therefore we should never return
    fn existing_layer_strategy(
        &self,
        context: &libcnb::build::BuildContext<Self::Buildpack>,
        layer_data: &libcnb::layer::LayerData<Self::Metadata>,
    ) -> Result<libcnb::layer::ExistingLayerStrategy, <Self::Buildpack as libcnb::Buildpack>::Error>
    {
        match CacheContents::new(self, context, layer_data)?.state() {
            Changed::Nothing(names) => {
                user::log_info("Found gems cache");
                user::log_info(format!(
                    "Skipping 'bundle install', no changes detected in: {}",
                    names.join(", ")
                ));

                Ok(ExistingLayerStrategy::Keep)
            }
            Changed::ForceUpdateSkipDigest(value) => {
                user::log_info("Found gems cache");
                user::log_info(format!(
                    "Running 'bundle install', detected HEROKU_SKIP_BUNDLE_DIGEST={value}"
                ));

                Ok(ExistingLayerStrategy::Update)
            }
            Changed::Digest(diff) => {
                user::log_info("Found gems cache");
                user::log_info(format!(
                    "Running 'bundle install', changes detected: {}",
                    diff.join(", ")
                ));

                Ok(ExistingLayerStrategy::Update)
            }
            Changed::Without(old, current) => {
                user::log_info(format!("BUNDLE_WITHOUT changed from {old} to {current}"));
                user::log_info("Running 'bundle install'");

                Ok(ExistingLayerStrategy::Update)
            }
            Changed::Stack(old, current) => {
                user::log_info(format!("Stack changed from {old} to {current}"));
                user::log_info("Clearing gems from cache");

                Ok(ExistingLayerStrategy::Recreate)
            }
            Changed::RubyVersion(old, current) => {
                user::log_info(format!("Ruby version changed from {old} to {current}"));
                user::log_info("Clearing gems from cache");

                Ok(ExistingLayerStrategy::Recreate)
            }
        }
    }
}

/// The possible states of the cache values, used for determining `ExistingLayerStrategy`
#[derive(Debug)]
enum Changed {
    Without(String, String),
    Nothing(Vec<String>), // Holds the values checked i.e. Gemfile, etc.
    Digest(Vec<String>),
    Stack(StackId, StackId),
    RubyVersion(ResolvedRubyVersion, ResolvedRubyVersion),
    ForceUpdateSkipDigest(String),
}

struct CacheContents {
    old: BundleInstallLayerMetadata,
    current: BundleInstallLayerMetadata,
    skip_digest: Option<OsString>,
}

impl CacheContents {
    fn new(
        layer: &BundleInstallLayer,
        context: &BuildContext<RubyBuildpack>,
        layer_data: &LayerData<BundleInstallLayerMetadata>,
    ) -> Result<Self, RubyBuildpackError> {
        let current = BundleInstallLayerMetadata {
            digest: BundleDigest::new(&context.app_dir, context.platform.env())
                .map_err(RubyBuildpackError::BundleInstallDigestError)?,
            stack: context.stack_id.clone(),
            without: layer.without.clone(),
            ruby_version: layer.ruby_version.clone(),
        };

        let skip_digest = context.platform.env().get("HEROKU_SKIP_BUNDLE_DIGEST");
        Ok(CacheContents {
            old: layer_data.content_metadata.metadata.clone(),
            current,
            skip_digest,
        })
    }

    fn state(&self) -> Changed {
        if self.current.stack != self.old.stack {
            Changed::Stack(self.old.stack.clone(), self.current.stack.clone())
        } else if self.current.ruby_version != self.old.ruby_version {
            Changed::RubyVersion(
                self.old.ruby_version.clone(),
                self.current.ruby_version.clone(),
            )
        } else if self.old.without != self.current.without {
            Changed::Without(self.old.without.0.clone(), self.current.without.0.clone())
        } else if let Some(value) = &self.skip_digest {
            Changed::ForceUpdateSkipDigest(value.to_string_lossy().to_string())
        } else if let Some(diff) = self.current.digest.diff(&self.old.digest) {
            Changed::Digest(diff)
        } else {
            let checked = self.current.digest.checked_names();
            Changed::Nothing(checked)
        }
    }
}

/// Executes the `bundle install` command and streams the results to stdout/stderr
fn bundle_install(
    layer_path: &Path,
    app_dir: &Path,
    without_default: &BundleWithout,
    env: &Env,
) -> Result<LayerEnv, CommandError> {
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

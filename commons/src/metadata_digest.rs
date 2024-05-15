use libcnb::{Env, Platform};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
use std::fmt::Display;
use std::path::{Path, PathBuf};

use crate::display::SentenceList;

const PLATFORM_ENV_VAR: &str = "user configured environment variables";

/// Store digest data in a Layer's metadata and compare them later
///
/// Store this struct as a field in the last value of your Layer's metadata.
///
/// Example:
///
/// ```rust
/// use serde::{Deserialize, Serialize};
/// use commons::metadata_digest::MetadataDigest;
///
/// #[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
/// pub(crate) struct BundleInstallLayerMetadata {
///     ruby_version: String,
///     force_bundle_install_key: String,
///
///     /// A struct that holds the cryptographic hash of components that can
///     /// affect the result of `bundle install`. When these values do not
///     /// change between deployments we can skip re-running `bundle install` since
///     /// the outcome should not change.
///     ///
///     /// While a fully resolved `bundle install` is relatively fast, it's not
///     /// instantaneous. This check can save ~1 second on overall build time.
///     ///
///     /// This value is cached with metadata, so changing the struct
///     /// may cause metadata to be invalidated (and the cache cleared).
///     ///
///     digest: MetadataDigest, // Must be last for serde to be happy https://github.com/toml-rs/toml-rs/issues/142
/// }
/// ```
///
/// Add the platform (environment variables) and files you want to compare between builds.
/// Then in the `fn update` of your layer you can use this information to see if there are changes
/// between the last time this was run:
///
/// ```rust,ignore
/// # use serde::{Deserialize, Serialize};
/// # use libcnb::build::BuildContext;
/// # use libcnb::layer::Layer;
/// # use libcnb::layer::LayerData;
/// # use libcnb::layer::LayerResult;
/// # use libcnb::layer::LayerResultBuilder;
/// # use std::path::Path;
/// # use libcnb::data::layer_content_metadata::LayerTypes;
/// use commons::metadata_digest::MetadataDigest;
/// # #[derive(Debug)]
/// # struct FakeBuildpack;
/// #
/// # #[derive(Debug)]
/// # struct FakeBuildpackError;
/// # use libcnb::generic::{GenericMetadata, GenericPlatform};
/// # use libcnb::Buildpack;
/// #
/// # impl Buildpack for FakeBuildpack {
/// #     type Platform = GenericPlatform;
/// #     type Metadata = GenericMetadata;
/// #     type Error = FakeBuildpackError;
/// #
/// #     fn detect(
/// #         &self,
/// #         context: libcnb::detect::DetectContext<Self>,
/// #     ) -> libcnb::Result<libcnb::detect::DetectResult, Self::Error> {
/// #         todo!()
/// #     }
/// #
/// #     fn build(
/// #         &self,
/// #         context: BuildContext<Self>,
/// #     ) -> libcnb::Result<libcnb::build::BuildResult, Self::Error> {
/// #         todo!()
/// #     }
/// # }
/// #
/// # #[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
/// # struct FakeLayerMetadata {
/// #     digest: MetadataDigest,
/// # }
/// #
/// # #[derive(Debug)]
/// # struct FakeLayer {}
/// #
/// # impl Layer for FakeLayer {
/// #     type Buildpack = FakeBuildpack;
/// #
/// #     type Metadata = FakeLayerMetadata;
/// #
/// #     fn types(&self) -> LayerTypes {
/// #         todo!()
/// #     }
/// #
///       fn update(
///           &mut self,
///           context: &BuildContext<Self::Buildpack>,
///           layer_data: &LayerData<Self::Metadata>,
///       ) -> Result<LayerResult<Self::Metadata>, <Self::Buildpack as Buildpack>::Error> {
///           let digest = MetadataDigest::new_env_files(
///               &context.platform,
///               &[
///                   &context.app_dir.join("Gemfile"),
///                   &context.app_dir.join("Gemfile.lock"),
///               ],
///           )
///           .unwrap();
///           let old = &layer_data.content_metadata.metadata;
///
///           if let Some(reason) = digest.changed(&old.digest) {
///               println!("Running expensive command, {reason}")
///           } else {
///               println!("Skipping expensive command")
///           }
///           LayerResultBuilder::new(FakeLayerMetadata { digest }).build()
///       }
/// #
/// #     fn create(
/// #         &mut self,
/// #         context: &BuildContext<Self::Buildpack>,
/// #         layer_path: &Path,
/// #     ) -> Result<LayerResult<Self::Metadata>, <Self::Buildpack as libcnb::Buildpack>::Error> {
/// #         todo!()
/// #     }
/// # }
/// ```
///
/// You're likely using this as a way to speed up some expensive or slow process. Since
/// caching is hard, your users may hit edge conditions that you did not expect. It's a
/// good idea to give them an "escape valve" to opt-out of this behavior.
///
/// A common idea is a vendor prefixed environment variable such as `HEROKU_SKIP_BUNDLE_DIGEST=1`.
///
/// Make sure to announce this feature to your user when skipping the expensive command.
///
/// The other consideration is that any layers that use `ExistingStrategy::Keep` or
/// that skip an execution may also have a change to their `LayerEnv` environment variables.
///
/// It might be surprising to have a buildpack author's changes to the `LayerEnv` returned by
/// the `LayerResultBuilder` not picked up by the tool they're skipping. One way around this
/// is to store a cache key in the metadata like `force_run_command: String` you can then
/// add logic to your buildpack to re-run that command similar to the "escape valve" discussed
/// above, but triggered by buildpack author instead of the end user.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct MetadataDigest {
    platform_env: Option<PlatformEnvDigest>,
    files: Option<PathsDigest>, // Must be last for serde to be happy https://github.com/toml-rs/toml-rs/issues/142
}

impl MetadataDigest {
    /// Create new from inputs
    ///
    /// # Errors
    ///
    /// Errors if one of the files cannot be read from disk.
    pub fn new_env_files(platform: &impl Platform, files: &[&Path]) -> Result<Self, DigestError> {
        let env = PlatformEnvDigest::new(platform);
        let files = PathsDigest::new(files)?;

        Ok(MetadataDigest {
            platform_env: Some(env),
            files: Some(files),
        })
    }

    /// Show difference between old and current metadata digest
    ///
    /// If no differences: None, Otherwise Some(Changed)
    /// where Changed implements Display
    #[must_use]
    pub fn changed(&self, old: &MetadataDigest) -> Option<Changed> {
        let files = self.diff_files(old);
        let env = match self.diff_platform_env(old) {
            PlatformEnvDifference::None => false,
            PlatformEnvDifference::Changed => true,
        };

        if env || files.is_some() {
            Some(Changed {
                files,
                platform_env: env,
            })
        } else {
            None
        }
    }

    fn diff_files(&self, old: &MetadataDigest) -> Option<PathChange> {
        match (&old.files, &self.files) {
            (None, None) => None,
            (None, Some(now)) => Some(PathChange::MismatchedFiles {
                other: Vec::new(),
                current: now.sorted_files(),
            }),
            (Some(old), None) => Some(PathChange::MismatchedFiles {
                other: old.sorted_files(),
                current: Vec::new(),
            }),
            (Some(old), Some(now)) => now.change(old),
        }
    }

    fn diff_platform_env(&self, old: &MetadataDigest) -> PlatformEnvDifference {
        if old.platform_env == self.platform_env {
            PlatformEnvDifference::None
        } else {
            PlatformEnvDifference::Changed
        }
    }

    // Returns a vec of all things checked in user readable strings
    #[must_use]
    pub fn checked_list(&self) -> Vec<String> {
        let mut parts = Vec::new();

        if let Some(files) = &self.files {
            for file in &files.sorted_files() {
                let file = file.display();
                parts.push(format!("{file}"));
            }
        }
        if self.platform_env.is_some() {
            let string = String::from(PLATFORM_ENV_VAR);
            parts.push(string);
        }

        parts
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash, PartialEq, Eq)]
struct ShaString(String);

#[derive(Serialize, Deserialize, Clone, Debug, Hash, PartialEq, Eq)]
struct PlatformEnvDigest(ShaString);
impl PlatformEnvDigest {
    fn new(platform: &impl Platform) -> Self {
        let env = platform.env();

        PlatformEnvDigest(sha_from_env(env))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
struct PathsDigest(HashMap<PathBuf, ShaString>);

/// Main struct for detecting changes between two iterations
///
/// Implements a direct to user display
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Changed {
    /// Some if there was a change in files along with the type of change
    files: Option<PathChange>,
    /// True when the environment variables changed
    platform_env: bool,
}

impl Display for Changed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Changed {
            files,
            platform_env,
        } = self;

        let platform_env_string = String::from(PLATFORM_ENV_VAR);
        match files {
            Some(PathChange::MismatchedFiles { other, current }) => {
                let other = other
                    .iter()
                    .map(|f| format!("'{}'", f.display()))
                    .collect::<Vec<String>>();

                let current = current
                    .iter()
                    .map(|f| format!("'{}'", f.display()))
                    .collect::<Vec<String>>();

                let other_string = SentenceList::new(&other);
                let current_string = SentenceList::new(&current);

                if *platform_env {
                    f.write_fmt(format_args!(
                    "change detected in {platform_env_string} and tracked file(s) from {other_string} to {current_string}"
                ))
                } else if current.len() > 1 || other.len() > 1 {
                    f.write_fmt(format_args!(
                        "change detected in tracked files from {other_string} to {current_string}"
                    ))
                } else {
                    f.write_fmt(format_args!(
                        "change detected in tracked file from {other_string} to {current_string}"
                    ))
                }
            }
            Some(PathChange::ChangedFiles(files)) => {
                let mut differences = files
                    .iter()
                    .map(|f| format!("'{}'", f.display()))
                    .collect::<Vec<String>>();

                if *platform_env {
                    differences.push(platform_env_string);
                }
                let changes = crate::display::list_to_sentence(&differences);

                if differences.len() > 1 {
                    f.write_fmt(format_args!("changes detected in {changes}"))
                } else {
                    f.write_fmt(format_args!("change detected in {changes}"))
                }
            }
            None => {
                if *platform_env {
                    f.write_fmt(format_args!("change detected in {platform_env_string}"))
                } else {
                    Ok(())
                }
            }
        }
    }
}

/// The difference state between two `PathsDigest`
#[derive(Debug, Clone, Eq, PartialEq)]
enum PathChange {
    /// Path digests are tracking different files
    /// for example one tracks [Gemfile, Gemfile.lock] while the other
    /// only tracks [Gemfile]
    MismatchedFiles {
        other: Vec<PathBuf>,
        current: Vec<PathBuf>,
    },

    /// Path digests are tracking the same files but
    /// the SHAs differ indicating a change
    ChangedFiles(Vec<PathBuf>),
}

/// The difference state between two `PlatformEnvDigest`-s
#[derive(Debug, Clone, Eq, PartialEq)]
enum PlatformEnvDifference {
    /// Enviornment variable digests are teh same
    None,

    /// Environment variable digests are different
    Changed,
}

impl PathsDigest {
    /// # Errors
    ///
    /// Errors if the file cannot be read from disk.
    fn new(paths: &[&Path]) -> Result<Self, DigestError> {
        let mut out = Self::default();
        out.add_paths(paths)?;

        Ok(out)
    }

    fn change(&self, old: &PathsDigest) -> Option<PathChange> {
        let old_files = old.sorted_files();
        let now_files = self.sorted_files();

        if old_files == now_files {
            let mut diff = Vec::new();

            for (k, now_sha) in &self.0 {
                if let Some(old_sha) = old.0.get(k) {
                    if old_sha != now_sha {
                        diff.push(k.clone());
                    }
                }
            }

            if diff.is_empty() {
                None
            } else {
                Some(PathChange::ChangedFiles(diff))
            }
        } else {
            Some(PathChange::MismatchedFiles {
                other: old_files,
                current: now_files,
            })
        }
    }

    fn files(&self) -> Vec<PathBuf> {
        self.0.keys().cloned().collect()
    }

    #[must_use]
    fn sorted_files(&self) -> Vec<PathBuf> {
        let mut files = self.files();
        files.sort();

        files
    }

    fn add_paths(&mut self, paths: &[&Path]) -> Result<&mut Self, DigestError> {
        for path in paths {
            let contents = fs_err::read_to_string(path)
                .map_err(|error| DigestError::CannotReadFile(path.to_path_buf(), error))?;

            self.0
                .insert(path.to_path_buf(), sha_from_string(&contents));
        }

        Ok(self)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum DigestError {
    #[error("Attempted to read file for digest but cannot: {1}")]
    CannotReadFile(PathBuf, std::io::Error),
}

fn sha_from_env(env: &Env) -> ShaString {
    let env_string = crate::display::env_to_sorted_string(env);
    sha_from_string(&env_string)
}

/// Hashing helper function, give it a str and it gives you the SHA256 hash back
/// out as a string
fn sha_from_string(str: &str) -> ShaString {
    let mut hasher = sha2::Sha256::new();
    hasher.update(str);
    ShaString(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod test {
    use super::*;

    #[cfg(test)]
    #[derive(Default, Clone)]
    struct FakeContext {
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

    #[test]
    fn ensure_adding_fields_doesnt_bust_cache() {
        let empty_digest = MetadataDigest::default();
        let actual = toml::to_string(&empty_digest).unwrap();
        let expected = "";
        assert_eq!(expected, &actual);

        let deserialized: MetadataDigest = toml::from_str("").unwrap();
        assert_eq!(empty_digest, deserialized);
    }

    #[test]
    fn metadata_digest_same() {
        assert_eq!(
            MetadataDigest::default().changed(&MetadataDigest::default()),
            None
        );
    }

    #[test]
    fn metadata_platform_env() {
        let mut env = Env::new();
        env.insert("PATH", "lol");
        let one = MetadataDigest {
            files: None,
            platform_env: Some(PlatformEnvDigest(sha_from_env(&env))),
        };

        let mut env = Env::new();
        env.insert("COMPUTER", "programming");
        let two = MetadataDigest {
            files: None,
            platform_env: Some(PlatformEnvDigest(sha_from_env(&env))),
        };

        assert!(one.changed(&two).unwrap().platform_env);
        assert!(
            one.changed(&MetadataDigest::default())
                .unwrap()
                .platform_env
        );
    }

    #[test]
    fn metadata_digest_different_file_names() {
        let tempdir = tempfile::tempdir().unwrap();
        let dir = tempdir.path();

        let gemfile = dir.join("Gemfile");
        fs_err::write(&gemfile, "gem 'mini_histogram'").unwrap();
        let context = FakeContext::default();

        let digest = MetadataDigest::new_env_files(&context.platform, &[&gemfile]).unwrap();

        assert_eq!(
            digest
                .changed(&MetadataDigest::default())
                .unwrap()
                .files
                .unwrap(),
            PathChange::MismatchedFiles {
                other: Vec::new(),
                current: vec![gemfile]
            }
        );
    }

    #[test]
    fn metadata_digest_files_changed() {
        let tempdir = tempfile::tempdir().unwrap();
        let dir = tempdir.path();

        let gemfile = dir.join("Gemfile");
        fs_err::write(&gemfile, "gem 'mini_histogram'").unwrap();
        let context = FakeContext::default();

        let one = MetadataDigest::new_env_files(&context.platform, &[&gemfile]).unwrap();

        fs_err::write(&gemfile, "gem 'a_different_gem_here'").unwrap();
        let two = MetadataDigest::new_env_files(&context.platform, &[&gemfile]).unwrap();

        assert_eq!(
            one.changed(&two).unwrap().files.unwrap(),
            PathChange::ChangedFiles(vec![gemfile.clone()])
        );

        assert_eq!(
            format!("change detected in '{}'", gemfile.display()),
            format!("{}", one.changed(&two).unwrap())
        );
    }
}

use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TargetId {
    pub(crate) distro_name: String,
    pub(crate) distro_version: String,
    pub(crate) cpu_architecture: String,
}
const ARCH_AWARE_VERSIONS: &[&str] = &["24.04"];
const DISTRO_VERSION_STACK: &[(&str, &str, &str)] = &[
    ("ubuntu", "22.04", "heroku-22"),
    ("ubuntu", "24.04", "heroku-24"),
];

#[derive(Debug, thiserror::Error)]
pub(crate) enum TargetIdError {
    #[error("Distro name and version '{0}-{1}' is not supported. Must be one of: {options}", options = DISTRO_VERSION_STACK.iter().map(|&(name, version, _)| format!("'{name}-{version}'")).collect::<Vec<_>>().join(", "))]
    UnknownDistroNameVersionCombo(String, String),
}

impl TargetId {
    pub(crate) fn is_arch_aware(&self) -> bool {
        ARCH_AWARE_VERSIONS.contains(&self.distro_version.as_str())
    }

    pub(crate) fn stack_name(&self) -> Result<String, TargetIdError> {
        DISTRO_VERSION_STACK
            .iter()
            .find(|&&(name, version, _)| name == self.distro_name && version == self.distro_version)
            .map(|&(_, _, stack)| stack.to_owned())
            .ok_or_else(|| {
                TargetIdError::UnknownDistroNameVersionCombo(
                    self.distro_name.clone(),
                    self.distro_version.clone(),
                )
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct OsDistribution {
    pub(crate) name: String,
    pub(crate) version: String,
}

impl Display for OsDistribution {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.name, self.version)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_arch_aware_versions_are_also_known_as_a_stack() {
        for version in ARCH_AWARE_VERSIONS {
            assert!(DISTRO_VERSION_STACK.iter().any(|&(_, v, _)| &v == version));
        }
    }

    #[test]
    fn test_stack_name() {
        assert_eq!(
            String::from("heroku-22"),
            TargetId {
                cpu_architecture: String::from("amd64"),
                distro_name: String::from("ubuntu"),
                distro_version: String::from("22.04"),
            }
            .stack_name()
            .unwrap()
        );

        assert_eq!(
            String::from("heroku-24"),
            TargetId {
                cpu_architecture: String::from("amd64"),
                distro_name: String::from("ubuntu"),
                distro_version: String::from("24.04"),
            }
            .stack_name()
            .unwrap()
        );
    }
}

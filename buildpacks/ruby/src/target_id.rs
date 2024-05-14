use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct TargetId {
    pub(crate) distro_name: String,
    pub(crate) distro_version: String,
    pub(crate) cpu_architecture: String,
}

const DISTRO_VERSION_STACK: &[(&str, &str, &str)] = &[
    ("ubuntu", "20.04", "heroku-20"),
    ("ubuntu", "22.04", "heroku-22"),
];

#[derive(Debug, thiserror::Error)]
pub(crate) enum TargetIdError {
    #[error("Distro name and version {0}-{1} is not supported. Must be one of: {}", DISTRO_VERSION_STACK.iter().map(|&(name, version, _)| format!("{name}-{version}")).collect::<Vec<_>>().join(", "))]
    UnknownDistroNameVersionCombo(String, String),

    #[error("Cannot convert stack name {0} into a target OS. Must be one of: {}", DISTRO_VERSION_STACK.iter().map(|&(_, _, stack)| String::from(stack)).collect::<Vec<_>>().join(", "))]
    UnknownStack(String),
}

impl TargetId {
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

    pub(crate) fn from_stack(stack_id: &str) -> Result<Self, TargetIdError> {
        DISTRO_VERSION_STACK
            .iter()
            .find(|&&(_, _, stack)| stack == stack_id)
            .map(|&(name, version, _)| TargetId {
                cpu_architecture: String::from("amd64"),
                distro_name: name.to_owned(),
                distro_version: version.to_owned(),
            })
            .ok_or_else(|| TargetIdError::UnknownStack(stack_id.to_owned()))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_stack_name() {
        assert_eq!(
            String::from("heroku-20"),
            TargetId {
                cpu_architecture: String::from("amd64"),
                distro_name: String::from("ubuntu"),
                distro_version: String::from("20.04"),
            }
            .stack_name()
            .unwrap()
        );

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
    }

    #[test]
    fn test_from_stack() {
        assert_eq!(
            TargetId::from_stack("heroku-20").unwrap(),
            TargetId {
                cpu_architecture: String::from("amd64"),
                distro_name: String::from("ubuntu"),
                distro_version: String::from("20.04"),
            }
        );

        assert_eq!(
            TargetId::from_stack("heroku-22").unwrap(),
            TargetId {
                cpu_architecture: String::from("amd64"),
                distro_name: String::from("ubuntu"),
                distro_version: String::from("22.04"),
            }
        );
    }
}

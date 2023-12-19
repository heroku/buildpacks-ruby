use serde::de::DeserializeOwned;
use std::any::{Any, TypeId};
use std::fmt::Debug;

/// Magically migrate serialized toml structs to the latest version
///
/// Approach:
///
/// If every struct knows the type of the one that came before it, and
/// if every struct can `Into` the type that comes after it. Then,
/// We build a type chain (using traits) to recurse backwards to find the
/// first struct a given string can serialize into. Once we've found a
/// serializable struct, we serialize it, then convert to the struct ahead
/// of it via `into`.
///
/// This requires only **a tiny bit** of type inspection which is used to
/// stop the recursion. The first struct in the chain has no predecessor
/// it specifies `Before: Self`. When this is reached, we know that the
/// type cannot be serialized to any struct so `None` is returned.
///
/// Example:
///
/// ```rust
/// use commons::magic_migrate::{MigrateToml, MigrateDe};
/// use serde::{Deserialize, Serialize};
///
/// // Given a struct that is serialized somwhere
/// #[derive(Deserialize, Serialize, Debug)]
/// struct ContactV1 {
///     name: String,
/// }
///
/// // Tell Rust that it can migrate to itself
/// impl MigrateToml for ContactV1 {
///     type From = Self;
/// }
///
/// // Define the next version you of data you wish to use
/// #[derive(Deserialize, Serialize, Debug)]
/// struct ContactV2 {
///     name: String,
///     title: Option<String>,
///     first_initial: String,
/// }
///
/// // Tell rust how to convert from one to the other manually
/// impl From<ContactV1> for ContactV2 {
///     fn from(value: ContactV1) -> Self {
///         ContactV2 {
///             name: value.name.clone(),
///             title: None,
///             first_initial: value.name.chars().next().unwrap().to_string(),
///         }
///     }
/// }
///
/// // Finally, link the latest struct to the one before it
/// impl MigrateToml for ContactV2 {
///     type From = ContactV1;
/// }
///
/// // Now when we don't know what version of struct the serialized
/// // data came from
/// let toml_str = "name = 'richard'";
///
/// // We can see that doading directly might fail
/// let v2_fails = toml::from_str::<ContactV2>(toml_str);
/// assert!(v2_fails.is_err());
///
/// // We see loading into v1 then migrating to v2 succeeds!
/// let v2 = ContactV2::from_str_migrations(toml_str).unwrap();
/// assert_eq!("r", &v2.first_initial);
/// assert_eq!("richard", &v2.name);
///
/// println!("It's magic!")
/// ```

pub trait MigrateToml: From<Self::From> + Any + DeserializeOwned + Debug {
    type From: MigrateToml;
}

impl<'de, T> MigrateDe<'de> for T
where
    T: MigrateToml,
{
    type From = <Self as MigrateToml>::From;

    type Deserializer = toml::Deserializer<'de>;

    fn deserializer(input: &'de str) -> <Self as MigrateDe>::Deserializer {
        toml::Deserializer::new(input)
    }
}

/// Generic migration over any deserializer
pub trait MigrateDe<'de>: From<Self::From> + Any + DeserializeOwned + Debug {
    type From: MigrateDe<'de>;
    type Deserializer: serde::Deserializer<'de>;

    /// Tell magic migrate how you want to deserialize your strings
    /// into structs
    fn deserializer(input: &'de str) -> <Self as MigrateDe>::Deserializer;

    fn from_str_migrations(input: &'de str) -> Option<Self> {
        if let Ok(instance) = Self::deserialize(Self::deserializer(input)) {
            Some(instance)
        } else if TypeId::of::<Self>() == TypeId::of::<Self::From>() {
            return None;
        } else {
            <Self::From as MigrateDe>::from_str_migrations(input).map(Into::into)
        }
    }
}

pub trait TryMigrateDe<'de>: TryFrom<Self::TryFrom> + Any + DeserializeOwned + Debug {
    type TryFrom: TryMigrateDe<'de>;

    type Deserializer: serde::Deserializer<'de>;

    /// Tell magic migrate how you want to deserialize your strings
    /// into structs
    fn deserializer(input: &'de str) -> <Self as TryMigrateDe>::Deserializer;

    type Error: From<<Self as TryFrom<<Self as TryMigrateDe<'de>>::TryFrom>>::Error>
        + From<<<Self as TryMigrateDe<'de>>::TryFrom as TryMigrateDe<'de>>::Error>;

    #[must_use]
    fn try_from_str_migrations(
        input: &'de str,
    ) -> Option<Result<Self, <Self as TryMigrateDe<'de>>::Error>> {
        if let Ok(instance) = Self::deserialize(Self::deserializer(input)) {
            Some(Ok(instance))
        } else if TypeId::of::<Self>() == TypeId::of::<Self::TryFrom>() {
            return None;
        } else {
            <Self::TryFrom as TryMigrateDe>::try_from_str_migrations(input).map(|inner| {
                inner
                    .map_err(Into::into)
                    .and_then(|before| Self::try_from(before).map_err(Into::into))
            })
        }
    }
}

/// Failibly migrate toml structs
///
/// Same idea as `MigrateToml`, but allows for lossy conversions.
pub trait TryMigrateToml: TryFrom<Self::TryFrom> + Any + DeserializeOwned + Debug {
    type TryFrom: TryMigrateToml;

    /// Shared error enum for the migration chain
    ///
    /// Migration chains will share a common enum. The bounds on this enum dictates
    /// that all prior errors must be convertable into this current error (automatically true if it's
    /// the same enum). And that any conversion errors via `TryFrom` when attempting to change the before
    /// struct to the current struct will implement the ability to convert into the shared error enum
    /// as well.
    type Error: From<<Self as TryFrom<<Self as TryMigrateToml>::TryFrom>>::Error>
        + From<<<Self as TryMigrateToml>::TryFrom as TryMigrateToml>::Error>;
}

impl<'de, T> TryMigrateDe<'de> for T
where
    T: TryMigrateToml,
{
    type TryFrom = <Self as TryMigrateToml>::TryFrom;
    type Error = <Self as TryMigrateToml>::Error;

    type Deserializer = toml::Deserializer<'de>;

    fn deserializer(input: &'de str) -> <Self as TryMigrateDe>::Deserializer {
        toml::Deserializer::new(input)
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use super::*;
    use serde::{Deserialize, Serialize};

    // Given a struct that is stored on disk somwhere
    #[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
    struct ContactV1 {
        name: String,
    }

    // Start the migration chain by migrating from self
    impl<'de> MigrateDe<'de> for ContactV1 {
        type From = Self;

        type Deserializer = toml::Deserializer<'de>;

        fn deserializer(input: &'de str) -> <Self as MigrateDe>::Deserializer {
            toml::Deserializer::new(input)
        }
    }

    // Define the next version you of data you wish to use
    #[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
    struct ContactV2 {
        name: String,
        title: Option<String>,
        first_initial: String,
    }

    // Tell rust how to convert from one to the other manually
    impl From<ContactV1> for ContactV2 {
        fn from(value: ContactV1) -> Self {
            ContactV2 {
                name: value.name.clone(),
                title: None,
                first_initial: value.name.chars().next().unwrap().to_string(),
            }
        }
    }

    // Finally, link the latest struct to the one before it
    impl<'de> MigrateDe<'de> for ContactV2 {
        type From = ContactV1;
        type Deserializer = toml::Deserializer<'de>;

        fn deserializer(input: &'de str) -> <Self as MigrateDe>::Deserializer {
            <Self as MigrateDe>::From::deserializer(input)
        }
    }

    #[test]
    fn de_migrate() {
        let metadata = ContactV1 {
            name: String::from("richard"),
        };

        let toml_string = toml::to_string(&metadata).unwrap();
        assert_eq!("name = \"richard\"".trim(), toml_string.trim());

        let result = toml::from_str::<ContactV2>(&toml_string);
        assert!(result.is_err());

        let v2 = ContactV2::from_str_migrations(&toml_string).unwrap();
        assert_eq!(String::from("richard"), v2.name);
        assert_eq!(String::from("r"), v2.first_initial);
        assert_eq!(None, v2.title);
    }

    #[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
    struct Lolv1 {
        name: String,
    }

    #[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
    struct Lolv2 {
        title: String,
    }

    impl From<Lolv1> for Lolv2 {
        fn from(value: Lolv1) -> Self {
            Lolv2 { title: value.name }
        }
    }

    impl MigrateToml for Lolv1 {
        type From = Self;
    }

    impl MigrateToml for Lolv2 {
        type From = Lolv1;
    }

    #[test]
    fn migration() {
        let metadata = Lolv1 {
            name: String::from("richard"),
        };

        let toml_string = toml::to_string(&metadata).unwrap();
        assert_eq!("name = \"richard\"".trim(), toml_string.trim());

        let result = toml::from_str::<Lolv2>(&toml_string);
        assert!(result.is_err());

        let v2 = Lolv2::from_str_migrations(&toml_string).unwrap();
        assert_eq!(String::from("richard"), v2.title);
    }

    #[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
    struct YoloV1 {
        name: String,
    }

    #[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
    struct YoloV2 {
        title: String,
    }

    #[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
    struct YoloV3 {
        address: String,
    }

    #[derive(Debug, PartialEq, Eq)]
    enum YoloMigrationError {
        String(String),
        /// Don't love this, The default of TryFrom from one struct to itself is `Infailable`
        /// So any type that we use here must know how to convert Infailable into itself.
        ///
        /// Seems like a smell we could do this in a better/different way
        Infailable,
    }

    impl From<String> for YoloMigrationError {
        fn from(value: String) -> Self {
            YoloMigrationError::String(value)
        }
    }

    impl From<Infallible> for YoloMigrationError {
        fn from(_value: Infallible) -> Self {
            YoloMigrationError::Infailable
        }
    }

    impl TryFrom<YoloV1> for YoloV2 {
        type Error = String;

        fn try_from(value: YoloV1) -> Result<Self, Self::Error> {
            Ok(YoloV2 { title: value.name })
        }
    }

    impl TryFrom<YoloV2> for YoloV3 {
        type Error = String;

        fn try_from(_value: YoloV2) -> Result<Self, Self::Error> {
            Err(String::from(
                "Cannot build a valid address from only a title",
            ))
        }
    }

    impl TryMigrateToml for YoloV1 {
        type TryFrom = Self;
        type Error = YoloMigrationError;
    }

    impl TryMigrateToml for YoloV2 {
        type TryFrom = YoloV1;
        type Error = YoloMigrationError;
    }

    impl TryMigrateToml for YoloV3 {
        type TryFrom = YoloV2;
        type Error = YoloMigrationError;
    }

    #[test]
    fn try_migration() {
        let metadata = YoloV1 {
            name: String::from("richard"),
        };

        let toml_string = toml::to_string(&metadata).unwrap();
        assert_eq!("name = \"richard\"".trim(), toml_string.trim());

        let result = toml::from_str::<YoloV2>(&toml_string);
        assert!(result.is_err());

        let v2 = YoloV2::try_from_str_migrations(&toml_string)
            .unwrap()
            .unwrap();
        assert_eq!(String::from("richard"), v2.title);

        let v3 = YoloV3::try_from_str_migrations(&toml_string).unwrap();
        assert_eq!(
            Err(YoloMigrationError::String(String::from(
                "Cannot build a valid address from only a title"
            ))),
            v3
        );
    }
}

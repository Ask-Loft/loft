//! Names that refer to registry entries: a node's tags name tag registry entries, and a
//! Constraint's technical binding names a constraint category. A tag name is a lowercase letter
//! followed by lowercase letters, digits and hyphens (RQ-71); a category name is one or more such
//! segments joined by dots (RQ-72).

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, de};

/// A name that breaks its registry's syntax. The reader adds the file, line and column.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    #[error(
        "`{0}` is not a tag name: a lowercase letter followed by lowercase letters, digits and \
         hyphens"
    )]
    Tag(String),
    #[error(
        "`{0}` is not a category name: segments joined by dots, each a lowercase letter followed \
         by lowercase letters, digits and hyphens"
    )]
    Category(String),
    #[error(
        "`{0}` is not an extension namespace: a lowercase letter followed by lowercase letters \
         and digits"
    )]
    Namespace(String),
}

/// A lowercase letter followed by lowercase letters, digits and hyphens.
fn is_segment(text: &str) -> bool {
    let mut bytes = text.bytes();
    bytes.next().is_some_and(|first| first.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// A lowercase letter followed by lowercase letters and digits: a segment a YAML key of the text
/// form can spell.
fn is_namespace(text: &str) -> bool {
    let mut bytes = text.bytes();
    bytes.next().is_some_and(|first| first.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

macro_rules! registry_names {
    ($($(#[$doc:meta])* $name:ident, $valid:expr, $error:path;)*) => {$(
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Reads a name, refusing one that breaks the syntax.
            pub fn parse(name: &str) -> Result<Self, NameError> {
                let valid: fn(&str) -> bool = $valid;
                if valid(name) {
                    Ok(Self(name.to_owned()))
                } else {
                    Err($error(name.to_owned()))
                }
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let name = String::deserialize(deserializer)?;
                Self::parse(&name).map_err(de::Error::custom)
            }
        }

        /// A name is looked up by its text, which orders, compares and hashes as the name does.
        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    )*};
}

registry_names! {
    /// The name of a tag registry entry, as a node's tag list names it.
    TagName, is_segment, NameError::Tag;
    /// The name of a constraint category, as a technical binding names it.
    CategoryName, |name| name.split('.').all(is_segment), NameError::Category;
    /// The namespace a tool keeps its extension data under in a node's `ext`.
    Namespace, is_namespace, NameError::Namespace;
}

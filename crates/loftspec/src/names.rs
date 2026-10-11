//! Names that refer to registry entries: a node's tags name tag registry entries, and a
//! Constraint's technical binding names a constraint category.

use std::fmt;

use serde::Serialize;

macro_rules! registry_names {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {$(
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn new(name: impl Into<String>) -> Self {
                Self(name.into())
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
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
    TagName,
    /// The name of a constraint category, as a technical binding names it.
    CategoryName,
}

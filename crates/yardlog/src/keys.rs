//! The keys of Yard Log's records and the ids agents give its blocks. Each key type wraps a
//! [`Key`] and is written as it, so the types only keep the code from passing one kind of record
//! where another is meant. An id is the agent's handle for a block, unique within Yard Log; the
//! key, which `loft` mints, is its identity.

use std::fmt;

pub use loftspec::identity::{Key, KeyError};

macro_rules! record_keys {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {$(
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Key);

        impl $name {
            #[must_use]
            pub const fn new(key: Key) -> Self {
                Self(key)
            }

            /// A fresh version 7 key.
            #[must_use]
            pub fn mint() -> Self {
                Self(Key::mint())
            }

            #[must_use]
            pub const fn key(self) -> Key {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, formatter)
            }
        }
    )*};
}

record_keys! {
    /// The key of a whole Yard Log, minted when it opens and kept for life.
    YardLogKey,
    /// The key of a block of any kind, a stage included.
    BlockKey,
    /// The key of one version of a block.
    VersionKey,
    /// The key of a submission.
    SubmissionKey,
    /// The key of an answer to a question, or of the outcome of an approval or a plan.
    ResponseKey,
}

impl BlockKey {
    /// The same key as a stage's: a stage is a block.
    #[must_use]
    pub const fn stage(self) -> StageKey {
        StageKey(self.0)
    }
}

/// The key of a block that is a stage, which names its file and which its blocks name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StageKey(Key);

impl StageKey {
    #[must_use]
    pub const fn new(key: Key) -> Self {
        Self(key)
    }

    #[must_use]
    pub const fn key(self) -> Key {
        self.0
    }

    /// The stage as the block it is.
    #[must_use]
    pub const fn block(self) -> BlockKey {
        BlockKey(self.0)
    }
}

impl fmt::Display for StageKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

/// The id an agent gives a block: lowercase letters, digits and hyphens, starting with a letter.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Handle(String);

/// Why a text is not an id.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "`{0}` is not an id: an id is lowercase letters, digits and hyphens, and starts with a letter"
)]
pub struct HandleError(pub String);

impl Handle {
    /// Reads an id.
    ///
    /// # Errors
    ///
    /// The text is empty, starts with something other than a lowercase letter, or holds a
    /// character other than a lowercase letter, a digit or a hyphen.
    pub fn parse(text: &str) -> Result<Self, HandleError> {
        let mut characters = text.chars();
        let starts = characters.next().is_some_and(|c| c.is_ascii_lowercase());
        if starts && characters.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            Ok(Self(text.to_owned()))
        } else {
            Err(HandleError(text.to_owned()))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// This id with the suffix `-<n>`, which settling gives the later of two records sharing an
    /// id.
    #[must_use]
    pub fn suffixed(&self, n: u32) -> Self {
        Self(format!("{}-{n}", self.0))
    }
}

impl fmt::Display for Handle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

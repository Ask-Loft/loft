//! Keys, the identity of each record a format of Loft writes: a node, decision or registry entry
//! of the Format, and each record of Yard Log.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

/// A UUID written in lowercase hyphenated form. Any UUID version is kept as read; new keys are
/// version 7 (ADR-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key(Uuid);

/// Why a string is not a key.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    #[error("key `{0}` is written in uppercase hexadecimal; keys are lowercase")]
    Uppercase(String),
    #[error("key `{0}` is not hyphenated; keys are written as 8-4-4-4-12 hexadecimal digits")]
    NotHyphenated(String),
    #[error("key `{0}` is not a UUID")]
    NotUuid(String),
}

const HYPHENS: [usize; 4] = [8, 13, 18, 23];

impl Key {
    /// Reads a key, accepting only the lowercase hyphenated form.
    pub fn parse(text: &str) -> Result<Self, KeyError> {
        let bytes = text.as_bytes();
        let hyphenated = bytes.len() == 36
            && bytes.iter().enumerate().all(|(index, byte)| {
                (*byte == b'-') == HYPHENS.contains(&index)
                    && (*byte == b'-' || byte.is_ascii_hexdigit())
            });
        if hyphenated {
            if bytes.iter().any(u8::is_ascii_uppercase) {
                return Err(KeyError::Uppercase(text.to_owned()));
            }
            return Uuid::try_parse(text)
                .map(Self)
                .map_err(|_| KeyError::NotUuid(text.to_owned()));
        }
        if bytes.len() == 32 && bytes.iter().all(u8::is_ascii_hexdigit) {
            return Err(KeyError::NotHyphenated(text.to_owned()));
        }
        Err(KeyError::NotUuid(text.to_owned()))
    }

    /// Mints a new version 7 key.
    #[must_use]
    pub fn mint() -> Self {
        Self(Uuid::now_v7())
    }

    /// The UUID version the key carries.
    #[must_use]
    pub fn version(self) -> usize {
        self.0.get_version_num()
    }

    #[must_use]
    pub const fn uuid(self) -> Uuid {
        self.0
    }
}

impl From<Uuid> for Key {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0.hyphenated(), formatter)
    }
}

impl FromStr for Key {
    type Err = KeyError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl Serialize for Key {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

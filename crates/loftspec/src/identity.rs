//! Keys, the identity of a node, decision or registry entry (F-1), and labels, the
//! human-readable handles the publisher allocates (F-2).

use std::fmt;
use std::num::NonZeroU64;
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

/// The kind prefix of a label (AC-243).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LabelPrefix {
    Product,
    FeatureGroup,
    Feature,
    Requirement,
    Criterion,
    Component,
    Constraint,
    QualityAttribute,
    Decision,
}

impl LabelPrefix {
    pub const ALL: [Self; 9] = [
        Self::Product,
        Self::FeatureGroup,
        Self::Feature,
        Self::Requirement,
        Self::Criterion,
        Self::Component,
        Self::Constraint,
        Self::QualityAttribute,
        Self::Decision,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Product => "P",
            Self::FeatureGroup => "FG",
            Self::Feature => "F",
            Self::Requirement => "RQ",
            Self::Criterion => "AC",
            Self::Component => "CMP",
            Self::Constraint => "CON",
            Self::QualityAttribute => "QA",
            Self::Decision => "ADR",
        }
    }

    /// The prefix written as `text`, such as `RQ`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|prefix| prefix.as_str() == text)
    }
}

/// A kind prefix, a hyphen and a positive integer without leading zeros, such as `RQ-23`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Label {
    prefix: LabelPrefix,
    number: NonZeroU64,
}

/// Why a string is not a label.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LabelError {
    #[error("label `{0}` is not a prefix, a hyphen and a positive integer without leading zeros")]
    Malformed(String),
    #[error("label `{0}` has a prefix the Format does not define")]
    UnknownPrefix(String),
}

impl Label {
    #[must_use]
    pub const fn new(prefix: LabelPrefix, number: NonZeroU64) -> Self {
        Self { prefix, number }
    }

    pub fn parse(text: &str) -> Result<Self, LabelError> {
        let malformed = || LabelError::Malformed(text.to_owned());
        let (prefix, digits) = text.split_once('-').ok_or_else(malformed)?;
        let canonical_digits = !digits.is_empty()
            && digits.bytes().all(|byte| byte.is_ascii_digit())
            && !digits.starts_with('0');
        if !canonical_digits {
            return Err(malformed());
        }
        let number = digits.parse::<NonZeroU64>().map_err(|_| malformed())?;
        let prefix =
            LabelPrefix::parse(prefix).ok_or_else(|| LabelError::UnknownPrefix(text.to_owned()))?;
        Ok(Self { prefix, number })
    }

    #[must_use]
    pub const fn prefix(self) -> LabelPrefix {
        self.prefix
    }

    #[must_use]
    pub const fn number(self) -> NonZeroU64 {
        self.number
    }
}

impl fmt::Display for Label {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}-{}", self.prefix.as_str(), self.number)
    }
}

impl FromStr for Label {
    type Err = LabelError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl Serialize for Label {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Label {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

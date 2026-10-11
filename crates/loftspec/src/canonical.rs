//! Canonical bytes and domain-separated digests (RQ-12): SHA-256 over a domain string, one zero
//! byte and the RFC 8785 canonical JSON of a view.

use std::fmt;
use std::str::FromStr;

use serde::Serialize;
use sha2::{Digest as _, Sha256};

/// The purpose a digest serves. Two purposes never share a domain string, so the same view
/// hashed for two purposes yields two digests (AC-46).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Domain {
    Node,
    Edges,
    Registry,
    Decision,
    Decisions,
    Content,
    Bindings,
    State,
}

impl Domain {
    pub const ALL: [Self; 8] = [
        Self::Node,
        Self::Edges,
        Self::Registry,
        Self::Decision,
        Self::Decisions,
        Self::Content,
        Self::Bindings,
        Self::State,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Node => "loftspec/node/1",
            Self::Edges => "loftspec/edges/1",
            Self::Registry => "loftspec/registry/1",
            Self::Decision => "loftspec/decision/1",
            Self::Decisions => "loftspec/decisions/1",
            Self::Content => "loftspec/content/1",
            Self::Bindings => "loftspec/bindings/1",
            Self::State => "loftspec/state/1",
        }
    }
}

/// A SHA-256 digest, written as 64 lowercase hexadecimal digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest([u8; 32]);

/// A string that is not 64 lowercase hexadecimal digits.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not 64 lowercase hexadecimal digits")]
pub struct DigestError(pub String);

impl Digest {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The 64 lowercase hexadecimal digits, two per byte.
    fn hex(&self) -> [u8; 64] {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut text = [0; 64];
        for (pair, byte) in text.chunks_exact_mut(2).zip(self.0) {
            pair[0] = DIGITS[usize::from(byte >> 4)];
            pair[1] = DIGITS[usize::from(byte & 0x0f)];
        }
        text
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hex = self.hex();
        formatter.write_str(std::str::from_utf8(&hex).map_err(|_| fmt::Error)?)
    }
}

impl FromStr for Digest {
    type Err = DigestError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let error = || DigestError(text.to_owned());
        let lowercase_hex = text.len() == 64
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        if !lowercase_hex {
            return Err(error());
        }
        let mut bytes = [0_u8; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).map_err(|_| error())?;
        }
        Ok(Self(bytes))
    }
}

impl<'de> serde::Deserialize<'de> for Digest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

impl Serialize for Digest {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// The RFC 8785 canonical JSON of a view.
#[must_use]
pub fn canonical_bytes<T: Serialize>(view: &T) -> Vec<u8> {
    serde_json_canonicalizer::to_vec(view)
        .expect("a view of strings, integers, booleans, nulls, arrays and objects serialises")
}

/// SHA-256 over the domain string, one zero byte and the view's canonical bytes.
#[must_use]
pub fn digest<T: Serialize>(domain: Domain, view: &T) -> Digest {
    digest_bytes(domain, &canonical_bytes(view))
}

/// SHA-256 over the domain string, one zero byte and canonical bytes already computed.
#[must_use]
pub fn digest_bytes(domain: Domain, canonical: &[u8]) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_str().as_bytes());
    hasher.update([0_u8]);
    hasher.update(canonical);
    Digest(hasher.finalize().into())
}

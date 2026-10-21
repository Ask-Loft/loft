//! Canonical bytes and domain-separated digests (RQ-12): SHA-256 over a domain string, one zero
//! byte and the RFC 8785 canonical JSON of a view.

use std::fmt;
use std::io;
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
    Extensions,
    Lint,
    Conflict,
    DecisionHistory,
    OwnRecord,
}

impl Domain {
    pub const ALL: [Self; 13] = [
        Self::Node,
        Self::Edges,
        Self::Registry,
        Self::Decision,
        Self::Decisions,
        Self::Content,
        Self::Bindings,
        Self::State,
        Self::Extensions,
        Self::Lint,
        Self::Conflict,
        Self::DecisionHistory,
        Self::OwnRecord,
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
            Self::Extensions => "loftspec/extensions/1",
            Self::Lint => "loftspec/lint/1",
            Self::Conflict => "loftspec/conflict/1",
            Self::DecisionHistory => "loftspec/decision-history/1",
            Self::OwnRecord => "loftspec/own-record/1",
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
    /// The digest whose 32 bytes are `bytes`.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The 64 lowercase hexadecimal digits, two per byte.
    fn hex(&self) -> [u8; 64] {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut text = [0; 64];
        for ([high, low], byte) in text.as_chunks_mut::<2>().0.iter_mut().zip(self.0) {
            *high = DIGITS[usize::from(byte >> 4)];
            *low = DIGITS[usize::from(byte & 0x0f)];
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

/// SHA-256 over the domain string, one zero byte and the view's canonical bytes, which are
/// streamed into the hash as they are written rather than collected first.
#[must_use]
pub fn digest<T: Serialize>(domain: Domain, view: &T) -> Digest {
    let mut writer = HashWriter::new(domain);
    serde_json_canonicalizer::to_writer(view, &mut writer)
        .expect("a view of strings, integers, booleans, nulls, arrays and objects serialises");
    writer.finish()
}

/// The digest of a JSON array whose elements' canonical bytes are already computed, in order:
/// RFC 8785 writes an array as its elements' canonical bytes, comma-separated, in brackets.
#[must_use]
pub fn digest_array(domain: Domain, elements: &[Vec<u8>]) -> Digest {
    let mut writer = HashWriter::new(domain);
    writer.0.update(b"[");
    for (index, element) in elements.iter().enumerate() {
        if index > 0 {
            writer.0.update(b",");
        }
        writer.0.update(element);
    }
    writer.0.update(b"]");
    writer.finish()
}

/// The SHA-256 digest of `parts`, each followed by one zero byte. It addresses nothing in a
/// document: a converter derives from it the key of a node whose source has none.
pub(crate) fn digest_parts<'a>(parts: impl IntoIterator<Item = &'a str>) -> Digest {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0_u8]);
    }
    Digest(hasher.finalize().into())
}

/// The SHA-256 digest of `bytes` as they are, which records a source file a converter read.
pub(crate) fn digest_bytes(bytes: &[u8]) -> Digest {
    Digest(Sha256::digest(bytes).into())
}

/// A SHA-256 hash seeded with a domain string and one zero byte, into which canonical bytes are
/// written.
struct HashWriter(Sha256);

impl HashWriter {
    fn new(domain: Domain) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(domain.as_str().as_bytes());
        hasher.update([0_u8]);
        Self(hasher)
    }

    fn finish(self) -> Digest {
        Digest(self.0.finalize().into())
    }
}

impl io::Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

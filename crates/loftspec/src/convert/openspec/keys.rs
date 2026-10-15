//! Keys derived from a node's place in the source, so that one project converts to one tree:
//! the version 8 UUID made from the first 16 bytes of the SHA-256 digest of
//! `loft-openspec-key/1`, the document key, the node's kind and each part of its source
//! identity, each followed by a zero byte.

use std::collections::BTreeMap;

use uuid::Builder;

use crate::canonical::digest_parts;
use crate::identity::Key;
use crate::ids::DocumentId;

const DOMAIN: &str = "loft-openspec-key/1";

/// The kinds of node the converter creates, as their keys are derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Kind {
    Product,
    FeatureGroup,
    Feature,
    Requirement,
    Criterion,
    Section,
}

impl Kind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Product => "product",
            Self::FeatureGroup => "feature-group",
            Self::Feature => "feature",
            Self::Requirement => "requirement",
            Self::Criterion => "criterion",
            Self::Section => "section",
        }
    }
}

/// A node's kind and its source identity, with the ordinal a repeat adds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Identity {
    pub kind: Kind,
    pub parts: Vec<String>,
}

/// The keys of one conversion, counting how often each identity repeats.
#[derive(Debug)]
pub(super) struct Keys {
    document: String,
    seen: BTreeMap<(Kind, Vec<String>), usize>,
}

impl Keys {
    pub fn new(document: DocumentId) -> Self {
        Self {
            document: document.to_string(),
            seen: BTreeMap::new(),
        }
    }

    /// The key of a node of `kind` whose source identity is `parts`, and the identity its
    /// children extend: `parts`, with its ordinal from 2 when it repeats an earlier node's.
    pub fn derive(&mut self, kind: Kind, parts: Vec<String>) -> (Key, Identity) {
        let count = self.seen.entry((kind, parts.clone())).or_insert(0);
        *count += 1;
        let mut parts = parts;
        if *count > 1 {
            parts.push(count.to_string());
        }
        let digest = digest_parts(
            [DOMAIN, &self.document, kind.name()]
                .into_iter()
                .chain(parts.iter().map(String::as_str)),
        );
        let bytes: [u8; 16] = digest.as_bytes()[..16]
            .try_into()
            .expect("a SHA-256 digest is longer than 16 bytes");
        let key = Key::from(Builder::from_custom_bytes(bytes).into_uuid());
        (key, Identity { kind, parts })
    }
}

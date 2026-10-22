//! The lock record a publish writes to `loft.lock.json` beside `spec/` (RQ-19, AC-666): the
//! format and algorithm, the document key, the state view its state root was computed under
//! where that view is not 1, the state, content and binding roots, the publish sequence and
//! time, and for a document derived from a Loft v1 export the source state (RQ-20).

use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};

use crate::FORMAT;
use crate::canonical::Digest;
use crate::document::DerivedFrom;
use crate::hash::StateView;
use crate::identity::Key;
use crate::ids::{AuthoredStateId, DocumentId};
use crate::manifest::{ALGORITHM, ManifestError, check_identifiers};
use crate::publish::{PublishedAt, TimeError};

/// The lock's file name, in the directory that holds `spec/`.
pub const LOCK: &str = "loft.lock.json";

/// What a publish pins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lock {
    pub document_key: DocumentId,
    pub state_view: StateView,
    pub state_root: Digest,
    pub content_root: Digest,
    pub binding_root: Digest,
    pub publish_sequence: NonZeroU64,
    pub published_at: PublishedAt,
    pub derived_from: Option<DerivedFrom>,
}

/// Why bytes are not a valid lock record.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LockError {
    #[error("{0}")]
    Shape(String),
    #[error(transparent)]
    Identifier(#[from] ManifestError),
    #[error(transparent)]
    Time(#[from] TimeError),
    #[error(
        "the lock names state view {view}; the highest this implementation computes is state view {}",
        highest.number()
    )]
    UnknownStateView { view: u64, highest: StateView },
}

/// The lock as it is written: its fields in the Format's order, no other field.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Wire {
    format: String,
    algorithm: String,
    document_key: Key,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    state_view: Option<NonZeroU64>,
    state_root: Digest,
    content_root: Digest,
    binding_root: Digest,
    publish_sequence: NonZeroU64,
    published_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    derived_from: Option<WireDerivedFrom>,
}

/// A field that is present holds its value: `null` is no value of it.
fn present<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<NonZeroU64>, D::Error> {
    NonZeroU64::deserialize(deserializer).map(Some)
}

/// The state view a lock names; state view 1 where it names none.
fn state_view(named: Option<NonZeroU64>) -> Result<StateView, LockError> {
    let Some(view) = named else {
        return Ok(StateView::One);
    };
    StateView::from_number(view.get()).ok_or(LockError::UnknownStateView {
        view: view.get(),
        highest: StateView::HIGHEST,
    })
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireDerivedFrom {
    authored_state_id: Key,
    content_address: Digest,
    publish_sequence: u64,
}

impl Lock {
    /// Reads a lock record, refusing one that lacks a lock field, holds another field, names a
    /// format, an algorithm or a state view this implementation does not implement, or holds a
    /// malformed value.
    ///
    /// # Errors
    ///
    /// The bytes are not a valid lock record.
    pub fn read(bytes: &[u8]) -> Result<Self, LockError> {
        let wire: Wire =
            serde_json::from_slice(bytes).map_err(|error| LockError::Shape(error.to_string()))?;
        check_identifiers(&wire.format, &wire.algorithm)?;
        Ok(Self {
            document_key: DocumentId::new(wire.document_key),
            state_view: state_view(wire.state_view)?,
            state_root: wire.state_root,
            content_root: wire.content_root,
            binding_root: wire.binding_root,
            publish_sequence: wire.publish_sequence,
            published_at: wire.published_at.parse()?,
            derived_from: wire.derived_from.map(|derived| DerivedFrom {
                authored_state_id: AuthoredStateId::new(derived.authored_state_id),
                content_address: derived.content_address,
                publish_sequence: derived.publish_sequence,
            }),
        })
    }

    /// The lock's bytes: a JSON object with its fields in the Format's order, indented by two
    /// spaces and ending with one newline.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let wire = Wire {
            format: FORMAT.to_owned(),
            algorithm: ALGORITHM.to_owned(),
            document_key: self.document_key.key(),
            state_view: (self.state_view != StateView::One).then(|| {
                NonZeroU64::new(self.state_view.number()).expect("a view numbered from 1")
            }),
            state_root: self.state_root,
            content_root: self.content_root,
            binding_root: self.binding_root,
            publish_sequence: self.publish_sequence,
            published_at: self.published_at.to_string(),
            derived_from: self.derived_from.as_ref().map(|derived| WireDerivedFrom {
                authored_state_id: derived.authored_state_id.key(),
                content_address: derived.content_address,
                publish_sequence: derived.publish_sequence,
            }),
        };
        let mut bytes = serde_json::to_vec_pretty(&wire).expect("a lock serialises");
        bytes.push(b'\n');
        bytes
    }
}

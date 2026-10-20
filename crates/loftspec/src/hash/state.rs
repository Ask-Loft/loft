//! The state view and the state root under each state view the Format defines (Roots): state
//! view 1 is the state view as it stood before the lint configuration entered it, and state view
//! 2 ends with the digest of `spec/lint.yaml` where the tree holds it, so that a lock pins the
//! configuration the linter reads with the tree.

use serde_json::{Value, json};

use super::Roots;
use crate::FORMAT;
use crate::canonical::{Digest, Domain, digest};
use crate::document::{DerivedFrom, Document};
use crate::manifest::ALGORITHM;

/// A state view: what the state root of a document covers. A lock names the one its state root
/// was computed under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StateView {
    One,
    Two,
}

impl StateView {
    /// The highest state view this implementation computes.
    pub const HIGHEST: Self = Self::Two;

    /// The view's number, as a lock writes it.
    #[must_use]
    pub const fn number(self) -> u64 {
        match self {
            Self::One => 1,
            Self::Two => 2,
        }
    }

    /// The view numbered `number`; none for a view this implementation does not compute.
    #[must_use]
    pub const fn from_number(number: u64) -> Option<Self> {
        match number {
            1 => Some(Self::One),
            2 => Some(Self::Two),
            _ => None,
        }
    }
}

/// The digest of a lint configuration: its whole text, as stored, as one JSON string, under
/// `loftspec/lint/1`. A configuration that is not UTF-8, which the linter refuses to read, is
/// taken with each invalid sequence replaced.
#[must_use]
pub fn lint_digest(lint: &[u8]) -> Digest {
    digest(Domain::Lint, &String::from_utf8_lossy(lint))
}

/// The state view of a document with the roots `roots` under `view`, given the bytes of the
/// tree's `spec/lint.yaml`, none for a tree without the file.
#[must_use]
pub fn state_view(
    document: &Document,
    roots: &Roots,
    view: StateView,
    lint: Option<&[u8]>,
) -> Value {
    let mut fields = view_one(document, roots.content, roots.bindings, roots.extensions);
    if let (StateView::Two, Some(lint)) = (view, lint) {
        fields.push(json!({ "lintConfiguration": lint_digest(lint) }));
    }
    Value::Array(fields)
}

/// The state root of a document with the roots `roots` under `view`, given the bytes of the
/// tree's `spec/lint.yaml`. Under state view 1, and for a tree without the file, it is
/// `roots.state`.
#[must_use]
pub fn state_root(
    document: &Document,
    roots: &Roots,
    view: StateView,
    lint: Option<&[u8]>,
) -> Digest {
    match (view, lint) {
        (StateView::Two, Some(_)) => {
            digest(Domain::State, &state_view(document, roots, view, lint))
        }
        _ => roots.state,
    }
}

/// The state view under state view 1: format, algorithm, document key, content root, binding
/// root and the derived-from record (AC-62), then the extensions root when there is one.
pub(super) fn view_one(
    document: &Document,
    content: Digest,
    bindings: Digest,
    extensions: Option<Digest>,
) -> Vec<Value> {
    let derived_from = document.derived_from.as_ref().map(
        |DerivedFrom {
             authored_state_id,
             content_address,
             publish_sequence,
         }| {
            json!({
                "authoredStateId": authored_state_id,
                "contentAddress": content_address,
                "publishSequence": publish_sequence,
            })
        },
    );
    let mut view = vec![
        json!(FORMAT),
        json!(ALGORITHM),
        json!(document.id),
        json!(content),
        json!(bindings),
        json!(derived_from),
    ];
    view.extend(extensions.map(|extensions| json!(extensions)));
    view
}

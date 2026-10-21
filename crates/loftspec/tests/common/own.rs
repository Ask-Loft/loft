//! The own record views and digests the own record tests read, by record kind and key number.

use std::collections::BTreeMap;

use loftspec::canonical::Digest;
use loftspec::codec::{OwnRecord, RecordKind, own_records};
use loftspec::document::Document;
use loftspec::identity::Key;
use loftspec::kind::NodeKind;
use serde_json::Value;

use super::key;

pub const PRODUCT: RecordKind = RecordKind::Node(NodeKind::Product);
pub const GROUP: RecordKind = RecordKind::Node(NodeKind::FeatureGroup);
pub const FEATURE: RecordKind = RecordKind::Node(NodeKind::Feature);
pub const REQUIREMENT: RecordKind = RecordKind::Node(NodeKind::Requirement);
pub const CRITERION: RecordKind = RecordKind::Node(NodeKind::Criterion);
pub const COMPONENT: RecordKind = RecordKind::Node(NodeKind::Component);
pub const CONSTRAINT: RecordKind = RecordKind::Node(NodeKind::Constraint);
pub const QUALITY: RecordKind = RecordKind::Node(NodeKind::QualityAttribute);

/// Every own record of a document.
pub fn own(document: &Document) -> BTreeMap<(RecordKind, Key), OwnRecord> {
    own_records(document)
}

/// The own record of the `kind` record keyed `n`.
pub fn record(document: &Document, kind: RecordKind, n: u32) -> OwnRecord {
    own(document)
        .remove(&(kind, key(n)))
        .unwrap_or_else(|| panic!("an own record of {kind:?} {n}"))
}

/// The own record view of the `kind` record keyed `n`.
pub fn view(document: &Document, kind: RecordKind, n: u32) -> Value {
    record(document, kind, n).view
}

/// The own record digest of the `kind` record keyed `n`.
pub fn digest_of(document: &Document, kind: RecordKind, n: u32) -> Digest {
    record(document, kind, n).digest
}

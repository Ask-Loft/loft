//! Conflicts that tree merges of the small document write, to settle, and the settlements made
//! of them.

use loftspec::conflict::{Conflict, SettleError, Settlement, settle};
use loftspec::document::Document;

use super::merge_tree::{merged, read, requirements};
use super::{key, requirement, small_document};

/// The second and third Requirements of `Keys`, after the Requirement keyed 4.
pub const SECOND: u32 = 60;
pub const THIRD: u32 = 61;

/// The document a tree merge of the three gives.
pub fn merge(base: &Document, ours: &Document, theirs: &Document) -> Document {
    read(&merged(base, ours, theirs))
}

/// The one conflict of `document` over `field`.
pub fn over<'d>(document: &'d Document, field: &str) -> &'d Conflict {
    let mut found = document
        .conflicts
        .iter()
        .filter(|conflict| conflict.field.as_str() == field);
    let conflict = found
        .next()
        .unwrap_or_else(|| panic!("a conflict over {field}: {:?}", document.conflicts));
    assert!(found.next().is_none(), "{:?}", document.conflicts);
    conflict
}

/// `document` with its one conflict over `field` settled by `settlement`, which must settle it.
pub fn settled(document: &Document, field: &str, settlement: &Settlement) -> Document {
    settle(document, over(document, field).id(), settlement)
        .unwrap_or_else(|refusal| panic!("{refusal:?}"))
}

/// Why settling the one conflict of `document` over `field` by `settlement` is refused.
pub fn refusal(document: &Document, field: &str, settlement: &Settlement) -> SettleError {
    settle(document, over(document, field).id(), settlement).expect_err("the settlement is refused")
}

/// `document` with a Requirement keyed by each of `keys` added to the group's Feature at `at`.
pub fn with_requirements(document: &Document, at: usize, keys: &[u32]) -> Document {
    let mut document = document.clone();
    for &n in keys {
        let added = requirement(key(n), "The writer shall write a slug.", Vec::new());
        requirements(&mut document, at).push(added);
    }
    document
}

/// The small document with two more Requirements in `Keys`: 4, 60, 61.
pub fn three_requirements() -> Document {
    with_requirements(&small_document(), 0, &[SECOND, THIRD])
}

//! The rule on nodes a typed document can break: a tag listed twice (RQ-79).

use std::collections::BTreeSet;

use super::ValidationError;
use crate::document::Document;
use crate::node::Node;

pub(super) fn check(document: &Document, errors: &mut Vec<ValidationError>) {
    document.walk(&mut |node| repeated_tags(node, errors));
}

/// One error per tag a node lists more than once.
fn repeated_tags(node: Node<'_>, errors: &mut Vec<ValidationError>) {
    let (Some(id), Some(details)) = (node.id(), node.details()) else {
        return;
    };
    let mut seen = BTreeSet::new();
    let mut reported = BTreeSet::new();
    for tag in &details.tags {
        if !seen.insert(tag) && reported.insert(tag) {
            errors.push(ValidationError::RepeatedTag {
                node: id,
                tag: tag.clone(),
            });
        }
    }
}

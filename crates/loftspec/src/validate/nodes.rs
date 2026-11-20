//! The rules on nodes a typed document can break: a tag listed twice (RQ-79), a Component that
//! declares `private` beneath an `external` one (ADR-15), and a Component that both provides and
//! requires one Interface.

use std::collections::BTreeSet;

use super::ValidationError;
use crate::boundary::Boundaries;
use crate::document::{Document, Edge};
use crate::node::Node;

pub(super) fn check(document: &Document, errors: &mut Vec<ValidationError>) {
    document.walk(&mut |node| repeated_tags(node, errors));
    errors.extend(Boundaries::of(document).conflicts().iter().map(|conflict| {
        ValidationError::PrivateUnderExternal {
            component: conflict.component,
            ancestor: conflict.ancestor,
        }
    }));
    provides_and_requires(document, errors);
}

/// One error per Component and Interface that a `provides` and a `requires` edge both join, in
/// the order of their keys.
fn provides_and_requires(document: &Document, errors: &mut Vec<ValidationError>) {
    let (mut provided, mut required) = (BTreeSet::new(), BTreeSet::new());
    for edge in &document.edges {
        match *edge {
            Edge::Provides {
                component,
                interface,
            } => provided.insert((component, interface)),
            Edge::Requires {
                component,
                interface,
            } => required.insert((component, interface)),
            _ => false,
        };
    }
    for &(component, interface) in provided.intersection(&required) {
        errors.push(ValidationError::ProvidesAndRequires {
            component,
            interface,
        });
    }
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

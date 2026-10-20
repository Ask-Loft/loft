//! The files that the nodes with a file of their own are written in, each Product, Feature and
//! root Component, as the writer places them, and the files an edit would make two nodes share.

use std::collections::{BTreeMap, BTreeSet};

use crate::codec::{SpecPath, Under, node_files};
use crate::document::Document;
use crate::ids::NodeId;
use crate::node::Node;
use crate::slug::Slug;

use super::Refusal;
use super::place::{Step, at};

/// Where the steps place a child, as far as its file is concerned.
pub(super) fn under<'a>(document: &'a Document, steps: &[Step]) -> Under<'a> {
    let top = steps.first().and_then(|step| at(document, &[*step]));
    match (top, at(document, steps), steps.len()) {
        (_, _, 0) => Under::Root,
        (Some(Node::Product(product)), _, 1) => Under::Product(&product.slug),
        (Some(Node::Product(product)), Some(Node::FeatureGroup(group)), 2) => Under::Group {
            product: &product.slug,
            group: &group.slug,
        },
        _ => Under::Elsewhere,
    }
}

/// The file of `node` standing under `under`, and of every node beneath it that has one, with
/// the node each holds, as the writer places them; `slug` is the slug of a Component at the
/// root.
pub(super) fn files(
    node: Node<'_>,
    slug: Option<&Slug>,
    under: &Under<'_>,
    found: &mut Vec<(SpecPath, Option<NodeId>)>,
) {
    node_files(node, slug, under, &mut |role, node| {
        found.push((role.path(), node.id()));
    });
}

/// Refuses a node that `landed` places in a file that a node of `document` holds, other than the
/// files `leaving` its nodes leave, keyed or not, or that another node of `landed` takes.
pub(super) fn check_files(
    document: &Document,
    landed: Vec<(SpecPath, Option<NodeId>)>,
    leaving: &BTreeSet<SpecPath>,
) -> Result<(), Refusal> {
    let mut held = Vec::new();
    for product in &document.products {
        files(Node::Product(product), None, &Under::Root, &mut held);
    }
    for root in &document.components {
        let node = Node::Component(&root.component);
        files(node, Some(&root.slug), &Under::Root, &mut held);
    }
    let mut occupied: BTreeMap<SpecPath, Option<NodeId>> = held
        .into_iter()
        .filter(|(path, _)| !leaving.contains(path))
        .collect();
    for (path, id) in landed {
        if let Some(&holder) = occupied.get(&path) {
            return Err(Refusal::FileHeld { path, holder });
        }
        occupied.insert(path, id);
    }
    Ok(())
}

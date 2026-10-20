//! The files that the nodes with a file of their own are written in, each Product, Feature and
//! root Component, as the directory layout places them by their parents' slugs and their own.

use std::collections::{BTreeMap, BTreeSet};

use crate::codec::{Role, SpecPath};
use crate::document::Document;
use crate::ids::NodeId;
use crate::node::Node;
use crate::slug::Slug;

use super::Refusal;
use super::place::{Step, at};

/// What a node stands under, as far as its file is concerned.
pub(super) enum Under<'a> {
    Root,
    Product(&'a Slug),
    Group {
        product: &'a Slug,
        group: &'a Slug,
    },
    /// A node of a kind whose children are written in its own file.
    Elsewhere,
}

impl<'a> Under<'a> {
    /// Where the steps place a child.
    pub(super) fn of(document: &'a Document, steps: &[Step]) -> Self {
        let top = steps.first().and_then(|step| at(document, &[*step]));
        match (top, at(document, steps), steps.len()) {
            (_, _, 0) => Self::Root,
            (Some(Node::Product(product)), _, 1) => Self::Product(&product.slug),
            (Some(Node::Product(product)), Some(Node::FeatureGroup(group)), 2) => Self::Group {
                product: &product.slug,
                group: &group.slug,
            },
            _ => Self::Elsewhere,
        }
    }
}

/// The file of `node` standing under `under`, and of every node beneath it that has one, with
/// the node each holds; `slug` is the slug of a Component at the root.
pub(super) fn files(
    node: Node<'_>,
    slug: Option<&Slug>,
    under: &Under<'_>,
    found: &mut Vec<(SpecPath, Option<NodeId>)>,
) {
    let id = node.id();
    match (node, under) {
        (Node::Product(product), Under::Root) => {
            let role = Role::Product {
                product: product.slug.to_string(),
            };
            found.push((role.path(), id));
            let under = Under::Product(&product.slug);
            for group in &product.feature_groups {
                files(Node::FeatureGroup(group), None, &under, found);
            }
        }
        (Node::FeatureGroup(group), Under::Product(product)) => {
            let under = Under::Group {
                product,
                group: &group.slug,
            };
            for feature in &group.features {
                files(Node::Feature(feature), None, &under, found);
            }
        }
        (Node::Feature(feature), Under::Group { product, group }) => {
            let role = Role::Feature {
                product: product.to_string(),
                group: group.to_string(),
                feature: feature.slug.to_string(),
            };
            found.push((role.path(), id));
        }
        (Node::Component(_), Under::Root) => {
            if let Some(slug) = slug {
                let role = Role::Component {
                    slug: slug.to_string(),
                };
                found.push((role.path(), id));
            }
        }
        _ => {}
    }
}

/// Refuses a node that `landed` places in a file that a node of `document` other than those
/// `leaving` holds, or that another node of `landed` takes.
pub(super) fn check_files(
    document: &Document,
    landed: Vec<(SpecPath, Option<NodeId>)>,
    leaving: &BTreeSet<NodeId>,
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
        .filter(|(_, id)| id.is_none_or(|id| !leaving.contains(&id)))
        .collect();
    for (path, id) in landed {
        if let Some(&holder) = occupied.get(&path) {
            return Err(Refusal::FileHeld { path, holder });
        }
        occupied.insert(path, id);
    }
    Ok(())
}

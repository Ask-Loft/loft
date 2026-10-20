//! The files of the nodes with a file of their own, each Product, Feature and root Component:
//! the role of each node's file as the directory layout places it by its parents' slugs and its
//! own, and the Product and Feature files of a tree in document order.

use std::collections::BTreeMap;

use super::error::SpecPath;
use super::files::{MANIFEST, Role, Tree};
use super::record::Rec;
use super::schema::field;
use crate::identity::Key;
use crate::node::Node;
use crate::slug::Slug;

/// What a node stands under, as far as its file is concerned.
pub(crate) enum Under<'a> {
    Root,
    Product(&'a Slug),
    Group {
        product: &'a Slug,
        group: &'a Slug,
    },
    /// A node of a kind whose children are written in its own file.
    Elsewhere,
}

/// Gives `each` the role of the file of `node` standing under `under`, and of every node beneath
/// it that has one, with the node the file holds; `slug` is the slug of a Component at the root.
pub(crate) fn node_files<'a>(
    node: Node<'a>,
    slug: Option<&Slug>,
    under: &Under<'_>,
    each: &mut impl FnMut(Role, Node<'a>),
) {
    match (node, under) {
        (Node::Product(product), Under::Root) => {
            let role = Role::Product {
                product: product.slug.to_string(),
            };
            each(role, node);
            let under = Under::Product(&product.slug);
            for group in &product.feature_groups {
                node_files(Node::FeatureGroup(group), None, &under, each);
            }
        }
        (Node::FeatureGroup(group), Under::Product(product)) => {
            let under = Under::Group {
                product,
                group: &group.slug,
            };
            for feature in &group.features {
                node_files(Node::Feature(feature), None, &under, each);
            }
        }
        (Node::Feature(feature), Under::Group { product, group }) => {
            let role = Role::Feature {
                product: product.to_string(),
                group: group.to_string(),
                feature: feature.slug.to_string(),
            };
            each(role, node);
        }
        (Node::Component(_), Under::Root) => {
            if let Some(slug) = slug {
                let role = Role::Component {
                    slug: slug.to_string(),
                };
                each(role, node);
            }
        }
        _ => {}
    }
}

/// A Product or Feature file of a tree, with the Feature Group that lists a Feature.
pub(crate) struct Filed<'a> {
    pub path: &'a SpecPath,
    pub record: &'a Rec,
    /// None for a Product.
    pub group: Option<&'a Rec>,
}

/// The Product and Feature files of a tree in document order: the Products as the manifest
/// lists them, each followed by the Features its Feature Groups list. A file no list reaches,
/// which only a tree the reader refuses holds, is left out.
pub(crate) fn filed_in_order(tree: &Tree) -> Vec<Filed<'_>> {
    let by_key = |wanted: fn(&Role) -> bool| -> BTreeMap<Key, (&SpecPath, &Rec)> {
        tree.records(wanted)
            .filter_map(|(path, _, record)| Some((record.key()?, (path, record))))
            .collect()
    };
    let products = by_key(|role| matches!(role, Role::Product { .. }));
    let features = by_key(|role| matches!(role, Role::Feature { .. }));
    let mut filed = Vec::new();
    let manifest = tree.record(&SpecPath::new(MANIFEST));
    let listed_products = manifest.map(|manifest| listed(manifest, field::PRODUCTS));
    for product in listed_products.unwrap_or_default() {
        let Some(&(path, record)) = products.get(&product) else {
            continue;
        };
        filed.push(Filed {
            path,
            record,
            group: None,
        });
        for group in record.records(field::FEATURE_GROUPS) {
            let listed = listed(group, field::FEATURES);
            for &(path, record) in listed.iter().filter_map(|key| features.get(key)) {
                let group = Some(group);
                filed.push(Filed {
                    path,
                    record,
                    group,
                });
            }
        }
    }
    filed
}

/// The keys a list field of keys holds, in order.
pub(crate) fn listed(record: &Rec, name: &str) -> Vec<Key> {
    record
        .list(name)
        .iter()
        .filter_map(|value| Key::parse(value.text()?).ok())
        .collect()
}

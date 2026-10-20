//! A document edited in memory, written by the codec (Library entry points): no edit sets or
//! changes a slug, and each node is written in the file its parent and its slug place it in.

use std::collections::BTreeMap;

use crate::common::editing::{
    GROUP, INNER, LABELS, NESTED, PRODUCT, ROOT, SLUGS, STORAGE, editable, feature_id,
};
use crate::common::text::path;
use crate::common::{key, slug};
use loftspec::codec;
use loftspec::document::Document;
use loftspec::edit::{self, Parent, Refusal, Subtree};
use loftspec::ids::{ComponentId, FeatureGroupId, NodeId, ProductId, RequirementId};
use loftspec::slug::Slug;

fn group(n: u32) -> Parent {
    Parent::Node(FeatureGroupId::new(key(n)).into())
}

/// The slug of every node that carries one, by its ID.
fn slugs(document: &Document) -> BTreeMap<NodeId, Slug> {
    let mut slugs = BTreeMap::new();
    document.walk(&mut |node| {
        if let (Some(id), Some(slug)) = (node.id(), node.slug()) {
            slugs.insert(id, slug.clone());
        }
    });
    for root in &document.components {
        slugs.insert(root.component.id.unwrap().into(), root.slug.clone());
    }
    slugs
}

/// The document with its edges, registries and root Components sorted, since their order means
/// nothing.
fn unordered(mut document: Document) -> Document {
    document.edges.sort_by_key(|edge| format!("{edge:?}"));
    document.tags.sort_by(|a, b| a.name.cmp(&b.name));
    document.categories.sort_by(|a, b| a.name.cmp(&b.name));
    document.components.sort_by(|a, b| a.slug.cmp(&b.slug));
    document
}

/// The document written and read back, which must be the document.
fn round_trip(document: &Document) -> codec::Files {
    let files = codec::write(document);
    let read = codec::read(&files).unwrap();
    assert_eq!(unordered(read), unordered(document.clone()));
    files
}

#[test]
fn no_edit_in_memory_sets_or_changes_a_slug() {
    let before = editable();
    let mut document = before.clone();
    let slugs_feature = edit::detach(&mut document, feature_id(SLUGS)).unwrap();
    edit::insert(&mut document, group(STORAGE), 0, slugs_feature).unwrap();
    edit::move_node(&mut document, feature_id(LABELS), group(STORAGE), 0).unwrap();
    let root = edit::detach(&mut document, ComponentId::new(key(ROOT)).into()).unwrap();
    edit::insert(&mut document, Parent::ROOT, 0, root).unwrap();
    let storage = FeatureGroupId::new(key(STORAGE)).into();
    let product = Parent::Node(ProductId::new(key(PRODUCT)).into());
    edit::move_node(&mut document, storage, product, 0).unwrap();
    assert_eq!(slugs(&document), slugs(&before));
    round_trip(&document);
}

#[test]
fn a_moved_node_keeps_its_slug() {
    let mut document = editable();
    edit::move_node(&mut document, feature_id(LABELS), group(STORAGE), 0).unwrap();
    let moved = &document.products[0].feature_groups[1].features[0];
    assert_eq!(moved.slug.as_str(), "labels");
}

#[test]
fn a_node_moved_to_another_parent_is_written_in_the_file_its_new_parent_and_slug_place_it_in() {
    let before = codec::write(&editable());
    let (from, to) = (
        "products/example-product/identity/labels.yaml",
        "products/example-product/storage/labels.yaml",
    );
    assert!(before.contains_key(&path(from)) && !before.contains_key(&path(to)));
    let mut document = editable();
    edit::move_node(&mut document, feature_id(LABELS), group(STORAGE), 0).unwrap();
    let requirement = RequirementId::new(key(4)).into();
    edit::move_node(
        &mut document,
        requirement,
        Parent::Node(feature_id(SLUGS)),
        0,
    )
    .unwrap();
    let inner = ComponentId::new(key(INNER)).into();
    let root = Parent::Node(ComponentId::new(key(ROOT)).into());
    edit::move_node(&mut document, inner, root, 1).unwrap();
    let files = round_trip(&document);
    assert!(files.contains_key(&path(to)) && !files.contains_key(&path(from)));
    let text = |at: &str| String::from_utf8(files[&path(at)].clone()).unwrap();
    let requirement = format!("key: {}", key(4));
    assert!(text("products/example-product/identity/slugs.yaml").contains(&requirement));
    assert!(!text("products/example-product/identity/keys.yaml").contains(&requirement));
}

#[test]
fn a_root_component_moved_under_a_component_is_written_in_its_root_components_file_and_keeps_no_slug()
 {
    let mut document = editable();
    let store = crate::common::root(crate::common::component(key(200), "Store"));
    edit::insert(&mut document, Parent::ROOT, 1, store.into()).unwrap();
    assert!(codec::write(&document).contains_key(&path("components/store.yaml")));
    let root = Parent::Node(ComponentId::new(key(ROOT)).into());
    let store = ComponentId::new(key(200)).into();
    edit::move_node(&mut document, store, root, 0).unwrap();
    let files = round_trip(&document);
    assert!(!files.contains_key(&path("components/store.yaml")));
    let text = String::from_utf8(files[&path("components/document-model.yaml")].clone()).unwrap();
    assert!(text.contains(&format!("key: {}", key(200))));
    let back = edit::move_node(&mut document, store, Parent::ROOT, 1);
    assert_eq!(back, Err(Refusal::NoSlug(Some(ComponentId::new(key(200))))));
}

#[test]
fn an_insert_or_a_move_to_the_root_takes_the_slug_its_caller_gives_where_the_node_has_none() {
    let mut document = editable();
    let store = Subtree::new(crate::common::component(key(200), "Store"));
    edit::insert(&mut document, Parent::Root(Some(slug("shelf"))), 1, store).unwrap();
    assert_eq!(document.components[1].slug.as_str(), "shelf");
    let nested = ComponentId::new(key(NESTED)).into();
    edit::move_node(&mut document, nested, Parent::Root(Some(slug("parser"))), 2).unwrap();
    assert_eq!(document.components[2].slug.as_str(), "parser");
    let files = round_trip(&document);
    assert!(files.contains_key(&path("components/shelf.yaml")));
    assert!(files.contains_key(&path("components/parser.yaml")));
    let root = ComponentId::new(key(ROOT)).into();
    edit::move_node(&mut document, root, Parent::Root(Some(slug("ignored"))), 2).unwrap();
    assert_eq!(document.components[2].slug.as_str(), "document-model");
    let product = Subtree::new(crate::common::product(key(201), "Shelf"));
    edit::insert(
        &mut document,
        Parent::Root(Some(slug("ignored"))),
        1,
        product,
    )
    .unwrap();
    assert_eq!(document.products[1].slug.as_str(), "shelf");
}

#[test]
fn a_move_that_writes_a_keyless_node_beneath_it_in_the_file_it_held_is_taken() {
    let mut document = editable();
    document.products[0].feature_groups[0].features[0].id = None;
    let keys = path("products/example-product/identity/keys.yaml");
    let before = codec::write(&document)[&keys].clone();
    let identity = FeatureGroupId::new(key(GROUP)).into();
    let product = ProductId::new(key(PRODUCT));
    edit::move_node(&mut document, identity, Parent::Node(product.into()), 1).unwrap();
    assert_eq!(
        document.products[0].feature_groups[1].slug.as_str(),
        "identity"
    );
    edit::move_node(&mut document, product.into(), Parent::ROOT, 0).unwrap();
    assert_eq!(codec::write(&document)[&keys], before);
}

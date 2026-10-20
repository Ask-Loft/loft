//! The edits a document held in memory refuses (Library entry points): a key it does not hold,
//! a kind its parent cannot hold, a key it already carries, a position beyond the end of the
//! list, and a move beneath the node itself. A refused edit leaves the document unchanged.

use crate::common::editing::{
    GROUP, INNER, KEYS, NESTED, ROOT, STORAGE, component, editable, feature_id, group,
};
use crate::common::text::path;
use crate::common::{criterion, feature, key, requirement};
use loftspec::document::Document;
use loftspec::edit::{self, InsertRefused, Parent, Refusal, Subtree};
use loftspec::ids::{ComponentId, FeatureGroupId, NodeId, ProductId, RequirementId};
use loftspec::kind::{NodeKind, ParentKind};

const UNHELD: u32 = 0x1ff;

/// The refusal of inserting `subtree`, which must hand the subtree back and leave the document
/// unchanged.
fn insert_refused(parent: Parent, position: usize, subtree: Subtree) -> Refusal {
    let mut document = editable();
    let given = subtree.clone();
    let InsertRefused { refusal, subtree } =
        edit::insert(&mut document, parent, position, subtree).unwrap_err();
    assert_eq!(subtree, given);
    assert_eq!(document, editable());
    refusal
}

/// The refusal of moving `node`, which must leave the document unchanged.
fn move_refused(node: NodeId, parent: Parent, position: usize) -> Refusal {
    let mut document = editable();
    let refusal = edit::move_node(&mut document, node, parent, position).unwrap_err();
    assert_eq!(document, editable());
    refusal
}

/// A new Feature keyed `n`, holding nothing.
fn new_feature(n: u32) -> Subtree {
    Subtree::new(feature(key(n), "Fresh", Vec::new()))
}

#[test]
fn a_detach_of_a_key_the_document_does_not_hold_is_refused_naming_it() {
    let mut document = editable();
    let unheld = feature_id(UNHELD);
    assert_eq!(
        edit::detach(&mut document, unheld),
        Err(Refusal::UnknownNode(unheld))
    );
    let as_another_kind = RequirementId::new(key(KEYS)).into();
    assert_eq!(
        edit::detach(&mut document, as_another_kind),
        Err(Refusal::UnknownNode(as_another_kind))
    );
    assert!(
        Refusal::UnknownNode(unheld)
            .to_string()
            .contains(&key(UNHELD).to_string())
    );
    assert_eq!(document, editable());
}

#[test]
fn a_move_of_a_key_the_document_does_not_hold_is_refused_naming_it() {
    let unheld = feature_id(UNHELD);
    assert_eq!(
        move_refused(unheld, group(GROUP), 0),
        Refusal::UnknownNode(unheld)
    );
}

#[test]
fn an_insert_or_a_move_under_a_parent_the_document_does_not_hold_is_refused_naming_it() {
    let unheld: NodeId = FeatureGroupId::new(key(UNHELD)).into();
    let refusal = insert_refused(Parent::Node(unheld), 0, new_feature(200));
    assert_eq!(refusal, Refusal::UnknownParent(unheld));
    assert!(refusal.to_string().contains(&key(UNHELD).to_string()));
    let refusal = move_refused(feature_id(KEYS), Parent::Node(unheld), 0);
    assert_eq!(refusal, Refusal::UnknownParent(unheld));
}

#[test]
fn an_insert_under_a_parent_whose_kind_cannot_hold_the_nodes_kind_is_refused_naming_both() {
    let product = Parent::Node(ProductId::new(key(1)).into());
    let refusal = insert_refused(product, 0, new_feature(200));
    let expected = Refusal::Containment {
        kind: NodeKind::Feature,
        parent: ParentKind::Node(NodeKind::Product),
    };
    assert_eq!(refusal, expected);
    assert_eq!(refusal.to_string(), "a feature cannot stand under product");
    let requirement = Subtree::new(requirement(key(201), "A rule.", Vec::new()));
    let at_root = Refusal::Containment {
        kind: NodeKind::Requirement,
        parent: ParentKind::Root,
    };
    assert_eq!(insert_refused(Parent::ROOT, 0, requirement), at_root);
    let criterion = Subtree::new(criterion(key(202), "A check."));
    let under_feature = Parent::Node(feature_id(KEYS));
    let refusal = insert_refused(under_feature, 0, criterion);
    assert!(matches!(
        refusal,
        Refusal::Containment {
            kind: NodeKind::Criterion,
            ..
        }
    ));
}

#[test]
fn a_move_under_a_parent_whose_kind_cannot_hold_the_nodes_kind_is_refused_naming_both() {
    let refusal = move_refused(feature_id(KEYS), Parent::Node(component(ROOT)), 0);
    let expected = Refusal::Containment {
        kind: NodeKind::Feature,
        parent: ParentKind::Node(NodeKind::Component),
    };
    assert_eq!(refusal, expected);
    let refusal = move_refused(feature_id(KEYS), Parent::ROOT, 0);
    assert_eq!(
        refusal.to_string(),
        "a feature cannot stand under the document's root"
    );
}

#[test]
fn an_inserted_node_whose_key_a_node_of_the_document_carries_is_refused_naming_it() {
    let refusal = insert_refused(group(STORAGE), 0, new_feature(KEYS));
    assert_eq!(refusal, Refusal::KeyHeld(key(KEYS)));
    assert!(refusal.to_string().contains(&key(KEYS).to_string()));
}

#[test]
fn an_inserted_node_whose_key_a_decision_of_the_document_carries_is_refused_naming_it() {
    let refusal = insert_refused(group(STORAGE), 0, new_feature(20));
    assert_eq!(refusal, Refusal::KeyHeld(key(20)));
}

#[test]
fn an_inserted_node_beneath_which_a_node_carries_a_held_key_is_refused_naming_it() {
    let held = requirement(key(4), "A rule.", Vec::new());
    let subtree = Subtree::new(feature(key(200), "Fresh", vec![held]));
    assert_eq!(
        insert_refused(group(STORAGE), 0, subtree),
        Refusal::KeyHeld(key(4))
    );
    let twice = vec![
        requirement(key(201), "A rule.", Vec::new()),
        requirement(key(201), "The same key.", Vec::new()),
    ];
    let subtree = Subtree::new(feature(key(200), "Fresh", twice));
    assert_eq!(
        insert_refused(group(STORAGE), 0, subtree),
        Refusal::KeyHeld(key(201))
    );
}

#[test]
fn a_position_beyond_the_end_of_the_parents_list_is_refused_naming_it_and_the_length() {
    let refusal = insert_refused(group(GROUP), 4, new_feature(200));
    assert_eq!(
        refusal,
        Refusal::Position {
            position: 4,
            length: 3
        }
    );
    assert_eq!(
        refusal.to_string(),
        "position 4 is beyond the end of a list of 3"
    );
    let refusal = move_refused(feature_id(KEYS), group(STORAGE), 1);
    assert_eq!(
        refusal,
        Refusal::Position {
            position: 1,
            length: 0
        }
    );
}

#[test]
fn a_move_of_a_node_under_itself_is_refused() {
    let nested = component(NESTED);
    let refusal = move_refused(nested, Parent::Node(nested), 0);
    assert_eq!(
        refusal,
        Refusal::BeneathItself {
            node: nested,
            parent: nested
        }
    );
}

#[test]
fn a_move_of_a_node_under_a_node_beneath_it_is_refused() {
    let (root, inner) = (component(ROOT), component(INNER));
    let refusal = move_refused(root, Parent::Node(inner), 0);
    assert_eq!(
        refusal,
        Refusal::BeneathItself {
            node: root,
            parent: inner
        }
    );
    let refusal = move_refused(component(NESTED), Parent::Node(inner), 0);
    assert_eq!(
        refusal,
        Refusal::BeneathItself {
            node: component(NESTED),
            parent: inner
        }
    );
}

#[test]
fn a_refused_edit_leaves_the_document_unchanged() {
    let document: Document = editable();
    let mut edited = document.clone();
    let _ = edit::detach(&mut edited, feature_id(UNHELD));
    let _ = edit::move_node(&mut edited, feature_id(KEYS), group(GROUP), 9);
    let _ = edit::insert(&mut edited, group(GROUP), 9, new_feature(200));
    assert_eq!(edited, document);
}

#[test]
fn a_node_with_no_slug_at_the_root_and_given_none_is_refused_naming_its_key() {
    let fresh = crate::common::component(key(200), "Store");
    let refusal = insert_refused(Parent::ROOT, 0, Subtree::new(fresh));
    assert_eq!(refusal, Refusal::NoSlug(Some(ComponentId::new(key(200)))));
    assert!(refusal.to_string().contains("without a slug"), "{refusal}");
    assert!(
        refusal.to_string().contains(&key(200).to_string()),
        "{refusal}"
    );
    let refusal = move_refused(component(NESTED), Parent::ROOT, 1);
    let nested = Some(ComponentId::new(key(NESTED)));
    assert_eq!(refusal, Refusal::NoSlug(nested));
}

/// A new Feature keyed `n` and named `name`, whose slug its name derives.
fn named_feature(n: u32, name: &str) -> Subtree {
    Subtree::new(feature(key(n), name, Vec::new()))
}

#[test]
fn an_edit_that_would_write_a_node_in_a_file_another_node_holds_is_refused_naming_both() {
    let refusal = insert_refused(group(GROUP), 3, named_feature(200, "Keys"));
    let keys = "products/example-product/identity/keys.yaml";
    let expected = Refusal::FileHeld {
        path: path(keys),
        holder: Some(feature_id(KEYS)),
    };
    assert_eq!(refusal, expected);
    assert!(refusal.to_string().contains(keys), "{refusal}");
    assert!(
        refusal.to_string().contains(&key(KEYS).to_string()),
        "{refusal}"
    );
    let shadow = crate::common::root(crate::common::component(key(201), "Document model"));
    let refusal = insert_refused(Parent::ROOT, 1, Subtree::new(shadow));
    assert!(
        matches!(refusal, Refusal::FileHeld { holder: Some(holder), .. }
        if holder == component(ROOT))
    );
    let product = Subtree::new(crate::common::product(key(202), "Example Product"));
    let refusal = insert_refused(Parent::ROOT, 1, product);
    assert!(
        matches!(refusal, Refusal::FileHeld { holder: Some(holder), .. }
        if holder == ProductId::new(key(1)).into())
    );
}

#[test]
fn a_move_that_would_write_a_node_in_a_file_another_node_holds_is_refused_naming_both() {
    let mut document = editable();
    edit::insert(&mut document, group(STORAGE), 0, named_feature(200, "Keys")).unwrap();
    let before = document.clone();
    let refusal = edit::move_node(&mut document, feature_id(KEYS), group(STORAGE), 0);
    let expected = Refusal::FileHeld {
        path: path("products/example-product/storage/keys.yaml"),
        holder: Some(feature_id(200)),
    };
    assert_eq!(refusal, Err(expected));
    let named = Parent::Root(Some(crate::common::slug("document-model")));
    let refusal = edit::move_node(&mut document, component(NESTED), named, 1);
    assert!(
        matches!(refusal, Err(Refusal::FileHeld { holder: Some(holder), .. })
        if holder == component(ROOT))
    );
    assert_eq!(document, before);
    edit::move_node(&mut document, feature_id(KEYS), group(GROUP), 2).unwrap();
}

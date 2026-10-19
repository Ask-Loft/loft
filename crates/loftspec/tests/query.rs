//! Naming a node by a query (the `loft find` and `loft review` Features), over the small
//! document, whose Requirement 4 carries the label RQ-2.

use crate::common::{key, small_document};
use loftspec::ids::{NodeId, RequirementId};
use loftspec::query::{identifies, named_nodes};

fn requirement() -> NodeId {
    RequirementId::new(key(4)).into()
}

#[test]
fn a_node_is_named_by_its_key_in_any_case() {
    let document = small_document();
    let upper = key(4).to_string().to_uppercase();
    assert_eq!(named_nodes(&document, &key(4).to_string()), [requirement()]);
    assert_eq!(named_nodes(&document, &upper), [requirement()]);
}

#[test]
fn a_node_is_named_by_its_label_in_any_case() {
    let document = small_document();
    assert_eq!(named_nodes(&document, "RQ-2"), [requirement()]);
    assert_eq!(named_nodes(&document, "rq-2"), [requirement()]);
}

#[test]
fn a_slug_names_exactly_and_a_name_alone_names_nothing() {
    assert!(identifies("the-part", key(1), None, Some("the-part")));
    assert!(!identifies("The-Part", key(1), None, Some("the-part")));
    assert_eq!(
        named_nodes(&small_document(), "writer"),
        Vec::<NodeId>::new()
    );
}

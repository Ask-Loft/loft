//! A Component's `provides` and `requires` edges to an Interface: their names, their place in
//! the Component's record, what they may join, how `loft fmt` resolves them by name and how they
//! enter the edges root.

use crate::common::interfaces::{INTERFACE, interface, with_interface};
use crate::common::key;
use crate::common::text::{COMPONENT, formatted, path, replaced, text};
use loftspec::codec::{self, Files};
use loftspec::document::{Document, Edge, EdgeKind};
use loftspec::fmt::format;
use loftspec::hash::{hash_document, hash_indexed_with_views};
use loftspec::ids::{ComponentId, InterfaceId};
use loftspec::validate::{ValidationError, validate};
use serde_json::{Value, json};

const COMPONENT_KEY: u32 = 6;
const LATER: u32 = INTERFACE + 5;

fn component() -> ComponentId {
    ComponentId::new(key(COMPONENT_KEY))
}

fn interface_id(n: u32) -> InterfaceId {
    InterfaceId::new(key(n))
}

/// The document with a second Interface, keyed after the first, and the given edges.
fn with_edges(edges: impl IntoIterator<Item = Edge>) -> Document {
    let mut document = with_interface();
    document
        .interfaces
        .push(interface(key(LATER), "Analytics feed", Vec::new()));
    document.edges.extend(edges);
    document
}

fn provides(n: u32) -> Edge {
    Edge::Provides {
        component: component(),
        interface: interface_id(n),
    }
}

fn requires(n: u32) -> Edge {
    Edge::Requires {
        component: component(),
        interface: interface_id(n),
    }
}

fn hex(n: u32) -> String {
    key(n).to_string()
}

#[test]
fn the_edges_of_a_component_to_an_interface_are_named_provides_and_requires() {
    assert_eq!(EdgeKind::Provides.as_str(), "provides");
    assert_eq!(EdgeKind::Requires.as_str(), "requires");
    assert_eq!(provides(INTERFACE).role(), None);
    assert_eq!(requires(INTERFACE).role(), None);
}

#[test]
fn provides_and_requires_follow_scoped_to_in_a_component_record_sorted_by_key() {
    let document = with_edges([provides(LATER), provides(INTERFACE)]);
    let files = codec::write(&document);
    let written = text(&files, COMPONENT);
    let expected = format!("provides:\n  - {}\n  - {}\n", hex(INTERFACE), hex(LATER));
    assert!(written.contains(&expected), "{written}");
    let document = with_edges([provides(INTERFACE), requires(LATER)]);
    let written = text(&codec::write(&document), COMPONENT);
    let at = |field: &str| written.find(&format!("\n{field}:")).unwrap_or(usize::MAX);
    assert!(at("boundary") < at("provides") || at("boundary") == usize::MAX);
    assert!(at("provides") < at("requires"), "{written}");
    assert!(at("requires") < at("mentions") || at("mentions") == usize::MAX);
    let back = codec::read(&codec::write(&document)).unwrap().edges;
    assert_eq!(back.len(), document.edges.len());
    assert!(document.edges.iter().all(|edge| back.contains(edge)));
}

/// The document's files with its one `field` target, `provides` or `requires`, written as `to`.
fn linking(field: &str, to: &str) -> Files {
    let edge = if field == "provides" {
        provides(INTERFACE)
    } else {
        requires(INTERFACE)
    };
    let files = codec::write(&with_edges([edge]));
    let from = format!("{field}:\n  - {}\n", hex(INTERFACE));
    replaced(&files, COMPONENT, &from, &format!("{field}:\n  - {to}\n"))
}

/// The document's files with its one `provides` target replaced by the key `n`.
fn providing(n: u32) -> Files {
    linking("provides", &hex(n))
}

fn read_errors(files: &Files) -> String {
    let errors = codec::read(files).expect_err("refused");
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_provides_or_requires_edge_whose_target_is_not_an_interface_is_an_error() {
    for field in ["provides", "requires"] {
        let errors = read_errors(&linking(field, &hex(1)));
        assert!(errors.contains("another kind"), "{field}: {errors}");
        assert!(errors.contains(COMPONENT), "{field}: {errors}");
    }
}

#[test]
fn a_provides_or_requires_edge_that_carries_a_role_is_an_error() {
    for field in ["provides", "requires"] {
        let carrying = format!("node: {}\n    role: outcome", hex(INTERFACE));
        let errors = read_errors(&linking(field, &carrying));
        assert!(errors.contains(COMPONENT), "{field}: {errors}");
    }
}

#[test]
fn a_nested_component_may_provide_and_require_an_interface() {
    let nested = ComponentId::new(key(INTERFACE + 0x30));
    let mut document = with_edges([]);
    let mut inner = document.components[0].component.clone();
    inner.id = Some(nested);
    inner.name = "Inner".to_owned();
    inner.components = Vec::new();
    inner.constraints = Vec::new();
    inner.quality_attributes = Vec::new();
    document.components[0].component.components.push(inner);
    document.edges.extend([
        Edge::Provides {
            component: nested,
            interface: interface_id(INTERFACE),
        },
        Edge::Requires {
            component: nested,
            interface: interface_id(LATER),
        },
    ]);
    let back = codec::read(&codec::write(&document)).unwrap();
    assert!(document.edges.iter().all(|edge| back.edges.contains(edge)));
    assert_eq!(validate(&back), vec![]);
}

#[test]
fn a_provides_or_requires_target_that_no_element_carries_is_an_error_naming_the_key() {
    let errors = read_errors(&providing(0x1fe));
    assert!(errors.contains(&hex(0x1fe)), "{errors}");
}

#[test]
fn a_component_that_both_provides_and_requires_one_interface_is_an_error() {
    let document = with_edges([provides(INTERFACE), requires(INTERFACE), requires(LATER)]);
    let errors = validate(&document);
    assert_eq!(
        errors,
        vec![ValidationError::ProvidesAndRequires {
            component: component(),
            interface: interface_id(INTERFACE),
        }]
    );
    assert!(errors[0].to_string().contains("both provides and requires"));
    assert_eq!(errors[0].keys(), vec![key(COMPONENT_KEY), key(INTERFACE)]);
    assert_eq!(
        validate(&with_edges([provides(INTERFACE), requires(LATER)])),
        vec![]
    );
}

/// The document's files with its `field` target written by `name`.
fn by_name(field: &str, name: &str) -> Files {
    let edge = if field == "provides" {
        provides(INTERFACE)
    } else {
        requires(INTERFACE)
    };
    let files = codec::write(&with_edges([edge]));
    let from = format!("{field}:\n  - {}\n", hex(INTERFACE));
    let to = format!("{field}:\n  - name: \"{name}\"\n");
    replaced(&files, COMPONENT, &from, &to)
}

#[test]
fn a_provides_or_requires_target_written_by_name_takes_the_key_of_the_interface() {
    for field in ["provides", "requires"] {
        let files = formatted(&by_name(field, "Document lookup"));
        let expected = format!("{field}:\n  - {}\n", hex(INTERFACE));
        assert!(text(&files, COMPONENT).contains(&expected), "{field}");
        assert!(codec::read(&files).is_ok(), "{field}");
    }
}

#[test]
fn a_name_that_only_another_kind_carries_matches_no_interface() {
    let files = by_name("provides", "Document model");
    let errors = format(&files).expect_err("no Interface has the name");
    let message = errors.iter().map(ToString::to_string).collect::<String>();
    assert!(message.contains("Document model"), "{message}");
    assert!(message.contains("Interface"), "{message}");
    assert_eq!(
        files[&path(COMPONENT)],
        by_name("provides", "Document model")[&path(COMPONENT)]
    );
}

fn edges_view(document: &Document) -> Value {
    let (_, views) = hash_indexed_with_views(&document.index().unwrap());
    views
        .roots
        .into_iter()
        .find(|(name, _, _)| *name == "edges")
        .unwrap()
        .2
}

#[test]
fn provides_and_requires_enter_the_edges_root_with_their_type_and_no_role() {
    let document = with_edges([provides(INTERFACE), requires(LATER)]);
    let tuples = edges_view(&document);
    let typed = |kind: &str| {
        tuples
            .as_array()
            .unwrap()
            .iter()
            .filter(|tuple| tuple[1] == json!(kind))
            .map(|tuple| tuple[3].clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(typed("provides"), vec![Value::Null]);
    assert_eq!(typed("requires"), vec![Value::Null]);
    let without = hash_document(&with_edges([])).unwrap().roots.edges;
    assert_ne!(hash_document(&document).unwrap().roots.edges, without);
}

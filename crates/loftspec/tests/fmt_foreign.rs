//! References into another document written by label or by name: `loft fmt` replaces each with
//! the key of the one node of the copies under its handle that carries the label or has the
//! name, and refuses one that matches nothing or more than one node, changing no file.

use crate::common::copies::{HANDLE, OWNER, copy_at, requiring, state_root};
use crate::common::interfaces::{INTERFACE, MODEL, interface, provides, with_interface};
use crate::common::text::{path, text};
use crate::common::{criterion, key, requirement, slug};
use loftspec::codec::{self, Files, Problem, copy_of};
use loftspec::document::{Document, Edge};
use loftspec::fmt::format;
use loftspec::ids::{CriterionId, DocumentId, ElementId};
use loftspec::publish::allocate;
use loftspec::yaml::Location;

const COMPONENT: &str = "components/document-model.yaml";
const FEATURE: &str = "products/example-product/identity/keys.yaml";

/// [`requiring`] whose root Component provides the copied Interface.
fn providing() -> Files {
    let mut document = requiring();
    document.edges.push(provides(MODEL, INTERFACE));
    codec::write(&document)
}

/// The files with the copied Interface's key in `provides` written as `written`.
fn written_as(files: &Files, written: &str) -> Files {
    let keyed = format!("key: {}\n", key(INTERFACE));
    let component = text(files, COMPONENT);
    assert!(component.contains(&keyed), "{component}");
    let mut changed = files.clone();
    let component = component.replace(&keyed, &format!("{written}\n"));
    changed.insert(path(COMPONENT), component.into_bytes());
    changed
}

/// The files `loft fmt` writes from `files`.
fn formatted(files: &Files) -> Files {
    let mut out = files.clone();
    out.extend(format(files).unwrap().changed);
    out
}

#[test]
fn a_foreign_reference_by_label_takes_the_key_its_copy_binds_to_the_label() {
    let files = providing();
    let by_label = written_as(&files, "label: IF-1");
    assert!(codec::read(&by_label).is_err(), "a strict read refuses it");
    assert_eq!(
        text(&formatted(&by_label), COMPONENT),
        text(&files, COMPONENT)
    );
}

#[test]
fn a_foreign_reference_by_name_takes_the_key_of_the_node_with_that_name() {
    let files = providing();
    let by_name = written_as(&files, "name: \"Document lookup\"");
    assert_eq!(
        text(&formatted(&by_name), COMPONENT),
        text(&files, COMPONENT)
    );
}

#[test]
fn a_foreign_label_or_name_that_matches_nothing_under_the_handle_is_refused_where_it_stands() {
    for written in ["label: IF-9", "name: \"Book lookup\""] {
        let files = written_as(&providing(), written);
        let errors = format(&files).unwrap_err();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(
            matches!(errors[0].problem, Problem::NameUnresolved { .. }),
            "{written}"
        );
        assert_eq!(errors[0].sites[0].path, path(COMPONENT));
        assert!(matches!(errors[0].sites[0].location, Some(Location::At(_))));
        assert!(errors[0].to_string().contains(HANDLE), "{}", errors[0]);
    }
}

/// The other document with a second Interface whose criterion has the first one's name.
fn twice_named() -> Document {
    let mut document = with_interface();
    document.id = DocumentId::new(key(OWNER));
    let second = requirement(
        key(INTERFACE + 0x11),
        "The lookup shall answer a document by its label.",
        vec![criterion(
            key(INTERFACE + 0x12),
            "The answer names the document.",
        )],
    );
    let other = interface(key(INTERFACE + 0x10), "Label lookup", vec![second]);
    document.interfaces.push(other);
    allocate(&mut document);
    document
}

#[test]
fn a_foreign_name_that_matches_more_than_one_node_is_refused_naming_each_key() {
    let owner = twice_named();
    let files = codec::write(&owner);
    let label_lookup = key(INTERFACE + 0x10);
    let second = copy_of(&slug(HANDLE), &files, label_lookup, 3, state_root(3)).unwrap();
    let mut document = requiring();
    document.copies = vec![copy_at(&owner, HANDLE, 3), second];
    document.edges.push(Edge::Mentions {
        source: ElementId::Node(CriterionId::new(key(5)).into()),
        target: ElementId::Node(CriterionId::new(key(INTERFACE + 2)).into()),
    });
    let files = codec::write(&document);
    let keyed = format!("key: {}\n", key(INTERFACE + 2));
    let feature =
        text(&files, FEATURE).replace(&keyed, "name: \"The answer names the document.\"\n");
    let mut ambiguous = files;
    ambiguous.insert(path(FEATURE), feature.into_bytes());
    let errors = format(&ambiguous).unwrap_err();
    assert_eq!(errors.len(), 1, "{errors:?}");
    let message = errors[0].to_string();
    for each in [key(INTERFACE + 2), key(INTERFACE + 0x12)] {
        assert!(message.contains(&each.to_string()), "{message}");
    }
}

#[test]
fn no_file_changes_when_a_foreign_reference_matches_nothing_or_more_than_one_node() {
    let files = written_as(&providing(), "label: IF-9");
    assert!(format(&files).is_err());
    let files = written_as(&providing(), "label: IF-1");
    let changed = format(&files).unwrap().changed;
    assert_eq!(changed.keys().collect::<Vec<_>>(), [&path(COMPONENT)]);
}

#[test]
fn provides_by_name_looks_only_at_copied_interfaces() {
    let files = written_as(&providing(), "name: \"The answer names the document.\"");
    let errors = format(&files).unwrap_err();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        matches!(errors[0].problem, Problem::NameUnresolved { .. }),
        "{errors:?}"
    );
    assert!(errors[0].to_string().contains("Interface"), "{}", errors[0]);
    let mut owner = crate::common::copies::owner();
    let answer = &mut owner.interfaces[0].requirements[0].criteria[0].name;
    "Document lookup".clone_into(answer);
    let mut document = requiring();
    document.copies = vec![copy_at(&owner, HANDLE, 3)];
    document.edges.push(provides(MODEL, INTERFACE));
    let files = codec::write(&document);
    let by_name = written_as(&files, "name: \"Document lookup\"");
    assert_eq!(
        text(&formatted(&by_name), COMPONENT),
        text(&files, COMPONENT)
    );
}

#[test]
fn a_foreign_reference_with_both_a_label_and_a_name_is_refused_where_it_stands() {
    let files = providing();
    let both = written_as(&files, "label: IF-1\n    name: \"Document lookup\"");
    let errors = format(&both).unwrap_err();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        matches!(errors[0].problem, Problem::Shape(_)),
        "{}",
        errors[0]
    );
    assert_eq!(errors[0].sites[0].path, path(COMPONENT));
    assert!(matches!(errors[0].sites[0].location, Some(Location::At(_))));
    let message = errors[0].to_string();
    assert!(
        message.contains("`label`") && message.contains("`name`"),
        "{message}"
    );
}

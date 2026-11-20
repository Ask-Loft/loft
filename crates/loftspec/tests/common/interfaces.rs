//! A document that holds an Interface, for the tests of the Interface kind and its edges.

use loftspec::document::Document;
use loftspec::identity::Key;
use loftspec::ids::InterfaceId;
use loftspec::slug::Slug;
use loftspec::tree::{Details, Interface, Requirement};

use super::{criterion, key, requirement, small_document};

/// The Interface's key in [`with_interface`].
pub const INTERFACE: u32 = 0x1c0;

/// An Interface keyed `id`, named `name`, holding `requirements`.
pub fn interface(id: Key, name: &str, requirements: Vec<Requirement>) -> Interface {
    Interface {
        id: Some(InterfaceId::new(id)),
        slug: Slug::derive(name, "interface"),
        name: name.to_owned(),
        details: Details::with_body("What a reader of the document may rely on."),
        requirements,
        constraints: Vec::new(),
        quality_attributes: Vec::new(),
    }
}

/// The small document with one Interface, "Document lookup", holding one Requirement and its
/// criterion.
pub fn with_interface() -> Document {
    let mut document = small_document();
    let promise = requirement(
        key(INTERFACE + 1),
        "The lookup shall answer a document by its key.",
        vec![criterion(
            key(INTERFACE + 2),
            "The answer names the document.",
        )],
    );
    document.interfaces = vec![interface(key(INTERFACE), "Document lookup", vec![promise])];
    document
}

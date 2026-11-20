//! A document that holds an Interface, for the tests of the Interface kind and its edges.

use loftspec::document::{Document, Edge};
use loftspec::identity::Key;
use loftspec::ids::{ComponentId, InterfaceId};
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

/// The small document's root Component, "Document model", by its key number.
pub const MODEL: u32 = 6;

/// The `provides` edge from the Component keyed `component` to the Interface keyed `interface`.
pub fn provides(component: u32, interface: u32) -> Edge {
    Edge::Provides {
        component: ComponentId::new(key(component)),
        interface: InterfaceId::new(key(interface)),
    }
}

/// The `requires` edge from the Component keyed `component` to the Interface keyed `interface`.
pub fn requires(component: u32, interface: u32) -> Edge {
    Edge::Requires {
        component: ComponentId::new(key(component)),
        interface: InterfaceId::new(key(interface)),
    }
}

/// The key number of the second Interface, "Zeta feed", of [`provided_and_required`].
pub const LATER: u32 = INTERFACE + 0x40;

/// The small document whose root Component provides "Document lookup" and requires a second
/// Interface, "Zeta feed", which holds nothing.
pub fn provided_and_required() -> Document {
    let mut document = with_interface();
    document
        .interfaces
        .push(interface(key(LATER), "Zeta feed", Vec::new()));
    document
        .edges
        .extend([provides(MODEL, INTERFACE), requires(MODEL, LATER)]);
    document
}

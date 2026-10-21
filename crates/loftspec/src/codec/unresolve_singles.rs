//! The records of a document's single files as its tree holds them: the manifest, each binding
//! of `spec/labels.yaml` and each entry of the tag, category and term registries.

use super::record::{Rec, Val};
use super::schema::{Kind, field};
use super::unresolve::{keys, records};
use crate::bindings::Binding;
use crate::document::{CategoryEntry, Document, TagEntry, TermEntry};
use crate::identity::{Key, LabelPrefix};

pub(super) fn manifest(document: &Document) -> Rec {
    let mut record = Rec::new(Kind::Manifest);
    record.set_text(field::FORMAT, crate::FORMAT);
    record.set_text(field::ALGORITHM, crate::manifest::ALGORITHM);
    record.set_text(field::DOCUMENT_KEY, document.id.key().to_string());
    if let Some(branch) = &document.publish_branch {
        record.set_text(field::PUBLISH_BRANCH, branch.as_str());
    }
    if document.connected {
        record.set_text(field::CONNECTED, "true");
    }
    let products = document.products.iter().filter_map(|product| product.id);
    record.set(
        field::PRODUCTS,
        keys(products.map(crate::ids::ProductId::key)),
    );
    if let Some(source) = &document.derived_from {
        let mut derived = Rec::new(Kind::DerivedFrom);
        derived.set_text(
            "authoredStateId",
            source.authored_state_id.key().to_string(),
        );
        derived.set_text(field::CONTENT_ADDRESS, source.content_address.to_string());
        derived.set_text(field::PUBLISH_SEQUENCE, source.publish_sequence.to_string());
        record.set(field::DERIVED_FROM, Val::Record(derived));
    }
    record
}

pub(super) fn labels(document: &Document) -> Rec {
    let mut counters = Rec::new(Kind::Counters);
    for prefix in LabelPrefix::ALL {
        counters.set_text(prefix.as_str(), document.counters.next(prefix).to_string());
    }
    let bindings = document.bindings.iter().map(binding_record);
    let mut record = Rec::new(Kind::Labels);
    record.set(field::COUNTERS, Val::Record(counters));
    record.set(field::BINDINGS, records(bindings));
    record
}

/// A binding's record, as `spec/labels.yaml` holds it.
pub(super) fn binding_record(binding: &Binding) -> Rec {
    let mut record = Rec::new(Kind::Binding);
    record.set_text(field::KEY, binding.element.key().to_string());
    record.set_text(field::LABEL, binding.label.to_string());
    if binding.departed {
        record.set_text(field::DEPARTED, "true");
    }
    record.set(
        field::ALIASES,
        Val::List(binding.aliases.iter().map(Val::scalar).collect()),
    );
    record.set(
        field::WITHDRAWN,
        Val::List(binding.withdrawn.iter().map(Val::scalar).collect()),
    );
    if let Some(state) = binding.published_state {
        record.set_text(field::PUBLISHED_STATE, state.as_str());
    }
    if let Some(history) = binding.published_history {
        record.set_text(field::PUBLISHED_HISTORY, history.to_string());
    }
    record
}

pub(super) fn tag_record(tag: &TagEntry) -> Rec {
    entry(
        Kind::TagEntry,
        tag.id.key(),
        tag.name.as_str(),
        &tag.definition,
    )
}

pub(super) fn category_record(category: &CategoryEntry) -> Rec {
    let name = category.name.as_str();
    entry(
        Kind::CategoryEntry,
        category.id.key(),
        name,
        &category.definition,
    )
}

pub(super) fn term_record(term: &TermEntry) -> Rec {
    let mut record = entry(Kind::TermEntry, term.id.key(), &term.name, &term.definition);
    if !term.aliases.is_empty() {
        let aliases = term.aliases.iter().map(Val::scalar).collect();
        record.set(field::ALIASES, Val::List(aliases));
    }
    record
}

fn entry(kind: Kind, key: Key, name: &str, definition: &str) -> Rec {
    let mut record = Rec::new(kind);
    record.set_text(field::KEY, key.to_string());
    record.set_text(field::NAME, name);
    record.set_text(field::DEFINITION, definition);
    record
}

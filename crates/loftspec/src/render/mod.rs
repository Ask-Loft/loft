//! The renderer (CMP-8, D-11 FG-6): the readable Markdown projection and the canonical JSON
//! export of a document, each stamped with the state root it was rendered from. Both are derived
//! and deterministic; no role reads them (F-26, ADR-12). The renderer yields bytes and depends on
//! no clock, locale or environment (CON-10); writing them belongs to a store.

mod decisions;
mod json;
mod markdown;

use std::collections::BTreeMap;

use crate::document::Document;
use crate::hash::{DocumentHashes, hash_indexed_with_views};
use crate::ids::ElementId;
use crate::index::IndexErrors;

/// What one document renders to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Projections {
    pub markdown: String,
    pub json: Vec<u8>,
}

/// Renders a document's Markdown projection and canonical JSON export.
///
/// # Errors
///
/// A document whose references do not resolve has no state root to stamp, and renders nothing.
pub fn render(document: &Document) -> Result<Projections, IndexErrors> {
    let indexed = document.index()?;
    let (hashes, views) = hash_indexed_with_views(&indexed);
    let labels = Labels::of(document);
    Ok(Projections {
        markdown: markdown::render(document, &hashes, &labels),
        json: json::render(document, &hashes, &views, &labels),
    })
}

/// How the projections name an element: its live label, or its key where it has none (AC-433).
pub(crate) struct Labels(BTreeMap<ElementId, String>);

impl Labels {
    fn of(document: &Document) -> Self {
        Self(
            document
                .bindings
                .iter()
                .map(|binding| (binding.element, binding.label.to_string()))
                .collect(),
        )
    }

    /// The element's live label, if it has one.
    pub(crate) fn label(&self, element: impl Into<ElementId>) -> Option<&str> {
        self.0.get(&element.into()).map(String::as_str)
    }

    /// The element's live label, or its key.
    pub(crate) fn name(&self, element: impl Into<ElementId>) -> String {
        let element = element.into();
        self.label(element)
            .map_or_else(|| element.key().to_string(), str::to_owned)
    }
}

/// The state root a projection is stamped with.
fn state_root(hashes: &DocumentHashes) -> String {
    hashes.roots.state.to_string()
}

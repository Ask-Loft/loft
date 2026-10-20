//! The renderer (CMP-8, FG-6): the readable Markdown projection and the canonical JSON
//! export of a document, each stamped with the state root it was rendered from. Both are derived
//! and deterministic; no role reads them (F-26, ADR-12). The renderer yields bytes and depends on
//! no clock, locale or environment (CON-10); writing them belongs to a store.

mod decisions;
mod json;
mod markdown;
pub mod openspec;

use std::collections::BTreeMap;

use crate::document::Document;
use crate::hash::{StateView, hash_indexed_with_views, state_root};
use crate::ids::ElementId;
use crate::index::IndexErrors;

/// What one document renders to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Projections {
    pub markdown: String,
    pub json: Vec<u8>,
}

/// Renders a document's Markdown projection and canonical JSON export, for a tree without
/// `spec/lint.yaml`.
///
/// # Errors
///
/// A document whose references do not resolve has no state root to stamp, and renders nothing.
pub fn render(document: &Document) -> Result<Projections, IndexErrors> {
    render_tree(document, None)
}

/// Renders the Markdown projection and canonical JSON export of a tree's document, given the
/// text of the tree's `spec/lint.yaml`: each is stamped with the tree's state root, under state
/// view 2.
///
/// # Errors
///
/// A document whose references do not resolve has no state root to stamp, and renders nothing.
pub fn render_tree(document: &Document, lint: Option<&str>) -> Result<Projections, IndexErrors> {
    let indexed = document.index()?;
    let (hashes, views) = hash_indexed_with_views(&indexed);
    let state = state_root(document, &hashes.roots, StateView::Two, lint).to_string();
    let labels = Labels::of(document);
    Ok(Projections {
        markdown: markdown::render(document, &state, &labels),
        json: json::render(document, &state, &views, &labels),
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

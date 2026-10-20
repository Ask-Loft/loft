//! The review packer's pages (The review packet): the packet of a tree and a base held in
//! memory, as the JSON of its units, each page taking every next whole unit that fits under a
//! limit of characters. Every caller pages alike, so the same versions, node and limit give
//! pages of the same bytes.

use std::num::NonZeroUsize;

use serde_json::Value;

use super::json::Render;
use super::{Unkeyed, pack};
use crate::check::{Placed, placed};
use crate::codec::Sites;
use crate::document::Document;
use crate::ids::NodeId;
use crate::validate::conflict_errors;

/// A version as the review packer is given it: the document, and where its keys are written,
/// which is empty for a document that no tree holds.
#[derive(Debug, Clone, Copy)]
pub struct Version<'a> {
    pub document: &'a Document,
    pub sites: &'a Sites,
}

/// A unit's JSON on its page, and the characters of its compact JSON, which a unit marked
/// `oversized` counts without the mark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagedUnit {
    pub unit: Value,
    pub chars: usize,
}

/// The page asked for, its number counted from 1, and the number of pages, one at least.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub units: Vec<PagedUnit>,
    pub number: usize,
    pub of: usize,
}

/// Why the review packer makes no page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The tree holds nodes without a key.
    Unkeyed(Vec<Unkeyed>),
    /// The tree holds conflict records, each placed at the site of the record that holds it.
    Conflicted(Vec<Placed>),
    /// The page asked for is past the last of `of` pages.
    PastLast { of: usize },
}

/// Page `number` of the review packet of `tree`, against `base` where one is given and only at
/// or below `node` where one is, its pages filled under `limit` characters.
///
/// # Errors
///
/// The tree holds a conflict record or a node without a key, or the page is past the last.
pub fn page(
    tree: Version<'_>,
    base: Option<Version<'_>>,
    node: Option<NodeId>,
    limit: usize,
    number: NonZeroUsize,
) -> Result<Page, Refused> {
    let conflicts = conflict_errors(tree.document);
    if !conflicts.is_empty() {
        return Err(Refused::Conflicted(placed(conflicts, tree.sites)));
    }
    let packet =
        pack(tree.document, base.map(|base| base.document), node).map_err(Refused::Unkeyed)?;
    let render = Render {
        sites: tree.sites,
        base_sites: base.map(|base| base.sites),
    };
    let pages = paginate(render.packet(&packet), limit);
    let of = pages.len().max(1);
    let number = number.get();
    if number > of {
        return Err(Refused::PastLast { of });
    }
    let units = pages.into_iter().nth(number - 1).unwrap_or_default();
    Ok(Page { units, number, of })
}

/// Whole units into pages: each page takes every next unit that fits, and a unit longer than
/// the limit stands alone, marked `oversized`.
fn paginate(units: Vec<Value>, limit: usize) -> Vec<Vec<PagedUnit>> {
    let mut pages: Vec<Vec<PagedUnit>> = Vec::new();
    let mut taken = 0;
    for mut unit in units {
        let chars = serde_json::to_string(&unit)
            .expect("a unit serializes")
            .chars()
            .count();
        if pages.is_empty() || taken + chars > limit {
            pages.push(Vec::new());
            taken = 0;
        }
        if chars > limit {
            unit["oversized"] = Value::Bool(true);
        }
        pages
            .last_mut()
            .expect("a page was opened")
            .push(PagedUnit { unit, chars });
        taken += chars;
    }
    pages
}

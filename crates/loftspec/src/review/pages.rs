//! The review packer's pages (The review packet): the packet of a tree and a base held in
//! memory, as the JSON of its units, each page taking every next whole unit that fits under a
//! limit of characters. Every caller pages alike, so the same versions, node and limit give
//! pages of the same bytes.

use std::io;
use std::num::NonZeroUsize;

use serde_json::Value;

use super::json::Render;
use super::{Unkeyed, pack};
use crate::check::{Placed, conflicted};
use crate::codec::Sites;
use crate::document::Document;
use crate::ids::NodeId;

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
    conflicted(tree.document, tree.sites).map_err(Refused::Conflicted)?;
    let packet =
        pack(tree.document, base.map(|base| base.document), node).map_err(Refused::Unkeyed)?;
    let render = Render {
        sites: tree.sites,
        base_sites: base.map(|base| base.sites),
    };
    let number = number.get();
    let (units, of) = paginate(render.packet(&packet), limit, number);
    let of = of.max(1);
    if number > of {
        return Err(Refused::PastLast { of });
    }
    Ok(Page { units, number, of })
}

/// Whole units into pages, page `number` of them kept, with the number of pages: each page takes
/// every next unit that fits, and a unit longer than the limit stands alone, marked `oversized`.
fn paginate(units: Vec<Value>, limit: usize, number: usize) -> (Vec<PagedUnit>, usize) {
    let mut page = Vec::new();
    let mut of = 0;
    let mut taken = 0;
    for mut unit in units {
        let chars = chars_of(&unit);
        if of == 0 || taken + chars > limit {
            of += 1;
            taken = 0;
        }
        if of == number {
            if chars > limit {
                unit["oversized"] = Value::Bool(true);
            }
            page.push(PagedUnit { unit, chars });
        }
        taken += chars;
    }
    (page, of)
}

/// The characters of a unit's compact JSON, counted as it is written.
fn chars_of(unit: &Value) -> usize {
    let mut chars = Chars(0);
    serde_json::to_writer(&mut chars, unit).expect("a unit serializes");
    chars.0
}

/// A sink that counts the characters of the UTF-8 written to it: every byte but a continuation
/// byte starts one.
struct Chars(usize);

impl io::Write for Chars {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 += bytes.iter().filter(|byte| **byte & 0xC0 != 0x80).count();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

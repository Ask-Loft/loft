//! `loft interface update [<handle>[:<interface>]]`: moves the pin of each copy it names to its
//! remote's head, or the one named copy to the publish sequence `--at` names, and reports how
//! each promise changed. A copy the head no longer holds stays at its pin, and a reference the
//! move leaves naming a node no copy holds is named; either exits 1.

mod moves;

use std::collections::BTreeSet;
use std::num::NonZeroU64;
use std::path::Path;

use loftspec::copy::Copy;
use loftspec::document::Document;
use loftspec::slug::Slug;
use loftspec_store::journal::{self, Changes};
use serde_json::{Value, json};

use super::reach::Remotes;
use super::references::references;
use super::{Interface, Named, Report, connected, document, named, refused};
use crate::stop::Stop;
use moves::Move;
pub(super) use moves::foreign_name;

pub(super) fn run(
    root: &Path,
    target: Option<&str>,
    at: Option<NonZeroU64>,
    browser: bool,
) -> Result<Report, Stop> {
    let (_, document) = document(root)?;
    connected(&document)?;
    let selected = select(&document, target, at.is_some())?;
    let mut remotes = Remotes::new(browser);
    let mut moves = Vec::new();
    for copy in selected {
        moves.push(Move::of(
            &mut remotes,
            &document,
            copy,
            at.map(NonZeroU64::get),
        )?);
    }
    let dangling = dangling(&document, &moves);
    write(root, &moves)?;
    Ok(report(&moves, &dangling))
}

/// The copies `target` names: every copy, those under a handle, or one copy.
fn select<'a>(
    document: &'a Document,
    target: Option<&str>,
    at: bool,
) -> Result<Vec<&'a Copy>, Stop> {
    let one = target.is_some_and(|target| target.contains(':'));
    if at && !one {
        return Err(refused(
            "`--at` moves one copy: name it as <handle>:<interface>",
        ));
    }
    let Some(target) = target else {
        return Ok(document.copies.iter().collect());
    };
    if !one {
        let handle = Slug::parse(target)
            .ok()
            .filter(|handle| (document.documents.iter()).any(|entry| entry.handle == *handle))
            .ok_or_else(|| refused(format!("`documents` does not list {target}")))?;
        return Ok((document.copies.iter())
            .filter(|copy| copy.handle == handle)
            .collect());
    }
    let Named { handle, interface } = named(target)?;
    let copy = (document.copies.iter()).find(|copy| {
        let key = copy.keys()[0];
        copy.handle == handle
            && match interface {
                Interface::Key(named) => named == key,
                Interface::Label(named) => copy.labels.get(&key) == Some(&named),
            }
    });
    copy.map(|copy| vec![copy]).ok_or_else(|| {
        refused(format!(
            "the document holds no copy of {target}: `loft interface add` takes one"
        ))
    })
}

/// A reference that a move leaves naming a node its copy no longer holds: the element, the
/// field and the foreign label it last named.
struct Dangling {
    element: String,
    field: &'static str,
    label: String,
}

/// Each reference of `document` to a node that a move takes out of its copy.
fn dangling(document: &Document, moves: &[Move<'_>]) -> Vec<Dangling> {
    let mut found = Vec::new();
    for change in moves {
        let Move::Moved { from, to, .. } = change else {
            continue;
        };
        let kept: BTreeSet<_> = to.keys().into_iter().collect();
        let gone: BTreeSet<_> = (from.keys().into_iter())
            .filter(|key| !kept.contains(key))
            .collect();
        for reference in references(document, &gone) {
            found.push(Dangling {
                element: reference.element,
                field: reference.field,
                label: moves::foreign_name(from, reference.key),
            });
        }
    }
    found
}

/// Writes each moved copy, and removes its previous file where its path changed, in one
/// journaled save.
fn write(root: &Path, moves: &[Move<'_>]) -> Result<(), Stop> {
    let mut files = loftspec::codec::Files::new();
    let mut removed = Vec::new();
    for change in moves {
        if let Move::Moved { from, to, .. } = change {
            files.insert(to.path(), to.bytes.clone());
            if from.path() != to.path() {
                removed.push(from.path());
            }
        }
    }
    if !files.is_empty() {
        journal::save(root, &Changes::of_tree(files, &removed))?;
    }
    Ok(())
}

fn report(moves: &[Move<'_>], dangling: &[Dangling]) -> Report {
    let withdrawn = moves
        .iter()
        .any(|change| matches!(change, Move::Withdrawn { .. }));
    let rewritten = moves
        .iter()
        .any(|change| matches!(change, Move::Moved { .. }));
    let outcome = if !dangling.is_empty() {
        "dangling"
    } else if withdrawn {
        "withdrawn"
    } else if rewritten {
        "updated"
    } else {
        "current"
    };
    let mut lines: Vec<String> = moves.iter().flat_map(Move::lines).collect();
    lines.extend(dangling.iter().map(|reference| {
        format!(
            "{} `{}` names {}, which its copy no longer holds",
            reference.element, reference.field, reference.label
        )
    }));
    if rewritten {
        lines.push("Commit spec/: its copies moved.".to_owned());
    }
    let dangling: Vec<Value> = (dangling.iter())
        .map(|reference| {
            json!({
                "element": reference.element,
                "field": reference.field,
                "label": reference.label,
            })
        })
        .collect();
    Report {
        outcome,
        fields: json!({
            "copies": moves.iter().map(Move::json).collect::<Vec<_>>(),
            "dangling": dangling,
        }),
        lines,
        failed: matches!(outcome, "dangling" | "withdrawn"),
    }
}

//! `loft review` (the `loft review` Feature): the review packet of the tree, against the tree
//! where HEAD left a base revision where `--base` is given, narrowed to a node by `--node`, and
//! paged by whole units. The packet is JSON; the text is an index of the page. It writes
//! nothing and judges nothing (the decision "The review packet carries context and no
//! judgement").

use std::num::NonZeroUsize;
use std::path::Path;

use loftspec::check::hashed;
use loftspec::codec;
use loftspec::document::Document;
use loftspec::hash::{StateView, state_root};
use loftspec::ids::NodeId;
use loftspec::query::named_nodes;
use loftspec::review::{Page, PagedUnit, Refused, Version, page};
use loftspec_store::repository::GitCommand;
use serde_json::{Value, json};

use crate::finish::Finished;
use crate::json;
use crate::out::outln;
use crate::stop::{self, Stop};
use crate::{base, version};

/// The schema `--format json` writes to, by name and version.
const SCHEMA: &str = "loft-review/1";

/// What `loft review` was asked for.
pub struct Options<'a> {
    pub base: Option<&'a str>,
    pub node: Option<&'a str>,
    pub max_chars: usize,
    pub page: NonZeroUsize,
}

pub fn run(root: &Path, options: &Options, as_json: bool) -> Finished {
    let (provenance, page) = match attempt(root, options) {
        Ok(paged) => paged,
        Err(stop) => return stop.exit(SCHEMA, as_json),
    };
    let outcome = if page.units.is_empty() {
        "empty"
    } else {
        "packed"
    };
    if as_json {
        let units: Vec<Value> = page.units.into_iter().map(|paged| paged.unit).collect();
        let fields = json!({
            "provenance": provenance,
            "page": { "number": page.number, "of": page.of, "maxChars": options.max_chars },
            "units": units,
        });
        json::print(&json::result(SCHEMA, outcome, fields, Vec::new()));
    } else {
        for paged in &page.units {
            outln!("{}", index_line(paged));
        }
        outln!("page {} of {}", page.number, page.of);
    }
    Finished::success(outcome)
}

/// The page the review packer gives of the tree under `root`, against the tree where HEAD left
/// the base revision where one is given, with where it came from.
fn attempt(root: &Path, options: &Options) -> Result<(Value, Page), Stop> {
    let files = stop::read_files(root)?;
    let (document, sites) = codec::read_sited(&files)?;
    let roots = hashed(&document, &sites)
        .map_err(|errors| Stop::invalid(&errors))?
        .roots;
    let lint = codec::lint_configuration(&files).map_err(|error| vec![error])?;
    let state_root = state_root(&document, &roots, StateView::Two, lint);
    let node = options
        .node
        .map(|query| named(&document, query))
        .transpose()?;
    let base = options
        .base
        .map(|revision| base::read_tree(&GitCommand::default(), root, revision))
        .transpose()?;
    let tree = Version {
        document: &document,
        sites: &sites,
    };
    let against = base.as_ref().map(|base| Version {
        document: &base.document,
        sites: &base.sites,
    });
    let page = page(tree, against, node, options.max_chars, options.page)
        .map_err(|why| refused(why, options.page))?;
    let provenance = json!({
        "loft": version::VERSION,
        "stateRoot": state_root.to_string(),
        "base": base.as_ref().map(|base| {
            json!({ "revision": base.revision, "commit": base.commit })
        }),
        "node": options.node,
    });
    Ok((provenance, page))
}

/// What stops a review the review packer refuses, page `number` being asked for. A tree read
/// from its files holds every key, and is hashed, which refuses a conflict record, before it is
/// packed.
fn refused(refused: Refused, number: NonZeroUsize) -> Stop {
    match refused {
        Refused::PastLast { of } => {
            let pages = if of == 1 { "page" } else { "pages" };
            let message =
                format!("page {number} is past the last page; the packet has {of} {pages}");
            Stop::one("page", message)
        }
        Refused::Conflicted(errors) => Stop::invalid(&errors),
        Refused::Unkeyed(_) => unreachable!("a tree read from its files holds every key"),
    }
}

/// The one node `query` names by its key, its slug or its label, the key and the label in any
/// case.
fn named(document: &Document, query: &str) -> Result<NodeId, Stop> {
    match named_nodes(document, query).as_slice() {
        [id] => Ok(*id),
        [] => Err(Stop::one("node", format!("`{query}` names no node"))),
        many => {
            let keys: Vec<String> = many.iter().map(|id| id.key().to_string()).collect();
            let message = format!("`{query}` names more than one node: {}", keys.join(", "));
            Err(Stop::one("node", message))
        }
    }
}

/// A unit as one line of the index: its scope's label and name, its number of assertions and
/// its number of characters.
fn index_line(paged: &PagedUnit) -> String {
    let unit = &paged.unit;
    let scope = &unit["scope"]["node"];
    let label = scope["label"].as_str().unwrap_or("-");
    let name = scope["name"].as_str().unwrap_or_default();
    let count = unit["assertions"].as_array().map_or(0, Vec::len);
    format!(
        "{label}  {name}  {count} assertions  {} characters",
        paged.chars
    )
}

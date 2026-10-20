//! `loft find <query>` (the `loft find` Feature): the nodes and decisions of the tree that a
//! query identifies, by a label, slug or key equal to it or a name that contains it, each with
//! the key a reference to it carries and where it is written. It reads the tree and writes
//! nothing; a query that matches nothing exits 1.

use std::path::Path;

use loftspec::codec::{self, Site, Sites};
use loftspec::document::Document;
use loftspec::identity::{Key, Label};
use loftspec::ids::ElementId;
use loftspec::query::{element_fields, identifies};
use serde_json::{Value, json};

use crate::finish::Finished;
use crate::json;
use crate::out::{errln, outln};
use crate::stop;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-find/1";

/// An element the query identifies.
struct Match {
    key: Key,
    label: Option<Label>,
    kind: &'static str,
    site: Site,
    name: String,
}

pub fn run(root: &Path, query: &str, as_json: bool) -> Finished {
    let read = stop::read_files(root).and_then(|files| Ok(codec::read_sited(&files)?));
    let (document, sites) = match read {
        Ok(read) => read,
        Err(stop) => return stop.exit(SCHEMA, as_json),
    };
    let found = find(&document, &sites, query);
    let outcome = if found.is_empty() { "none" } else { "found" };
    if as_json {
        let matches: Vec<Value> = found.iter().map(match_json).collect();
        let fields = json!({ "matches": matches });
        json::print(&json::result(SCHEMA, outcome, fields, Vec::new()));
    } else {
        for each in &found {
            outln!("{}", line(each));
        }
    }
    if found.is_empty() {
        errln!("nothing matches \"{query}\"");
        return Finished::failure(outcome);
    }
    Finished::success(outcome)
}

/// Every node, then every decision, that `query` identifies, in document order.
fn find(document: &Document, sites: &Sites, query: &str) -> Vec<Match> {
    let wanted = query.to_lowercase();
    let mut found = Vec::new();
    let mut consider = |element: ElementId, kind: &'static str, slug: Option<&str>, name: &str| {
        let key = element.key();
        let label = document.bindings.label_of(element);
        let identified =
            identifies(query, key, label, slug) || name.to_lowercase().contains(&wanted);
        if identified {
            found.push(Match {
                key,
                label,
                kind,
                site: sites
                    .get(key)
                    .cloned()
                    .expect("every element read from a tree has a site"),
                name: name.to_owned(),
            });
        }
    };
    document.walk(&mut |node| {
        if let Some(id) = node.id() {
            let slug = node.slug().map(loftspec::slug::Slug::as_str);
            consider(id.into(), node.kind().as_str(), slug, &node.name());
        }
    });
    for decision in &document.decisions {
        let slug = Some(decision.slug.as_str());
        consider(decision.id.into(), "decision", slug, &decision.title);
    }
    found
}

/// A match as one line: key, label, kind, site and name, two spaces apart.
fn line(found: &Match) -> String {
    let label = found
        .label
        .map_or_else(|| "-".to_owned(), |label| label.to_string());
    format!(
        "{}  {label}  {}  {}  {}",
        found.key, found.kind, found.site, found.name
    )
}

fn match_json(found: &Match) -> Value {
    Value::Object(element_fields(
        found.key,
        found.label,
        found.kind,
        Some(&found.site),
        &found.name,
    ))
}

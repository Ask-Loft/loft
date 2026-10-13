//! `loft find <query>` (the `loft find` Feature): the nodes and decisions of the tree that a
//! query identifies, by a label, slug or key equal to it or a name that contains it, each with
//! the key a reference to it carries and where it is written. It reads the tree and writes
//! nothing; a query that matches nothing exits 1.

use std::path::Path;
use std::process::ExitCode;

use loftspec::codec::{self, Site, Sites};
use loftspec::document::Document;
use loftspec::ids::ElementId;
use loftspec_store::FileStore;
use serde_json::{Map, Value, json};

use crate::json;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-find/1";

/// An element the query identifies.
struct Match {
    key: String,
    label: Option<String>,
    kind: &'static str,
    site: Site,
    name: String,
}

pub fn run(root: &Path, query: &str, as_json: bool) -> ExitCode {
    let files = match FileStore::open(root).and_then(|store| store.read_files()) {
        Ok(files) => files,
        Err(error) => return json::stop(SCHEMA, vec![json::store_error(&error)], &error, as_json),
    };
    let (document, sites) = match codec::read_sited(&files) {
        Ok(read) => read,
        Err(errors) => {
            if as_json {
                json::print(&json::failure(SCHEMA, json::read_errors(&errors)));
            }
            crate::report(&errors);
            return ExitCode::FAILURE;
        }
    };
    let found = find(&document, &sites, query);
    if as_json {
        let outcome = if found.is_empty() { "none" } else { "found" };
        let matches: Vec<Value> = found.iter().map(match_json).collect();
        let fields = json!({ "matches": matches });
        json::print(&json::result(SCHEMA, outcome, fields, Vec::new()));
    } else {
        for each in &found {
            println!("{}", line(each));
        }
    }
    if found.is_empty() {
        eprintln!("nothing matches \"{query}\"");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Every node, then every decision, that `query` identifies, in document order.
fn find(document: &Document, sites: &Sites, query: &str) -> Vec<Match> {
    let wanted = query.to_lowercase();
    let mut found = Vec::new();
    let mut consider = |element: ElementId, kind: &'static str, slug: Option<&str>, name: &str| {
        let key = element.key();
        // Every element read from a tree carries a key, and so a site.
        let Some(site) = sites.get(key) else {
            return;
        };
        let text = key.to_string();
        let label = document
            .bindings
            .label_of(element)
            .map(|label| label.to_string());
        let identified = label
            .as_deref()
            .is_some_and(|label| label.eq_ignore_ascii_case(query))
            || slug == Some(query)
            || text == wanted
            || name.to_lowercase().contains(&wanted);
        if identified {
            found.push(Match {
                key: text,
                label,
                kind,
                site: site.clone(),
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
    let label = found.label.as_deref().unwrap_or("-");
    format!(
        "{}  {label}  {}  {}  {}",
        found.key, found.kind, found.site, found.name
    )
}

fn match_json(found: &Match) -> Value {
    let mut fields = Map::new();
    fields.insert("key".to_owned(), json!(found.key));
    fields.insert("label".to_owned(), json!(found.label));
    fields.insert("kind".to_owned(), json!(found.kind));
    fields.extend(json::site(&found.site));
    fields.insert("name".to_owned(), json!(found.name));
    Value::Object(fields)
}

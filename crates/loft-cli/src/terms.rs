//! `loft terms` (the `loft terms` Feature): the term report of the tree, the groups of phrases
//! that may name one concept and the subjects of each Product's statements, for the author to
//! decide in the term list. A group is a candidate, not a defect, so the command exits 0 whatever
//! it reports, and it writes nothing.

use std::fmt::Write;
use std::path::Path;

use loftspec::codec::{self, LINT, ReadError, SpecPath};
use loftspec::document::Document;
use loftspec::identity::Key;
use loftspec::lint::report::{Group, Mark, ProductSubjects};
use loftspec::lint::{LintConfig, RuleId, TermReport, term_report};
use serde_json::{Value, json};

use crate::finish::Finished;
use crate::json;
use crate::out::outln;
use crate::stop::{self, Stop};

/// The schema `--format json` writes to, by name and version.
const SCHEMA: &str = "loft-terms/1";

pub fn run(root: &Path, as_json: bool) -> Finished {
    let read = stop::read_files(root).and_then(|files| {
        let config = LintConfig::read(files.get(&SpecPath::new(LINT)).map(Vec::as_slice))
            .map_err(|errors| config_errors(&errors))?;
        Ok((codec::read(&files)?, config))
    });
    let (document, config) = match read {
        Ok(read) => read,
        Err(stop) => return stop.exit(SCHEMA, as_json),
    };
    let report = term_report(&document, &config);
    let labels = Labels(&document);
    if as_json {
        json::print(&document_json(&report, &labels));
    } else {
        outln!("{}", text(&report, &document, &labels).trim_end());
    }
    Finished::success("reported")
}

/// A lint configuration in error stops the report, as it stops a check.
fn config_errors(errors: &[ReadError]) -> Stop {
    Stop {
        errors: errors
            .iter()
            .map(|error| json::read_error("lint-config", error))
            .collect(),
        lines: errors.iter().map(ToString::to_string).collect(),
    }
}

/// The label of each node a report names.
struct Labels<'a>(&'a Document);

impl Labels<'_> {
    fn of(&self, key: Key) -> Option<String> {
        let document = self.0;
        document
            .bindings
            .iter()
            .find(|binding| binding.element.key() == key)
            .map(|binding| binding.label.to_string())
    }

    /// The label, or the key of a node without one.
    fn named(&self, key: Key) -> String {
        self.of(key).unwrap_or_else(|| key.to_string())
    }
}

fn text(report: &TermReport, document: &Document, labels: &Labels<'_>) -> String {
    let mut out = String::new();
    for group in &report.groups {
        let _ = writeln!(out, "{} {}: {}", group.severity, group.rule, group.message);
        for phrase in &group.phrases {
            let nodes: Vec<String> = phrase.nodes.iter().map(|key| labels.named(*key)).collect();
            let _ = writeln!(
                out,
                "  {} {} ({})",
                phrase.text,
                phrase.count,
                nodes.join(", ")
            );
        }
    }
    for product in &report.products {
        let name = document
            .products
            .iter()
            .find(|each| each.id.map(loftspec::ids::ProductId::key) == product.product)
            .map_or("", |each| each.name.as_str());
        let label = product
            .product
            .map_or_else(|| "-".to_owned(), |key| labels.named(key));
        let _ = writeln!(out, "subjects of {label} · {name}");
        for subject in &product.subjects {
            let mark = match &subject.mark {
                Some(Mark::Term) => " (term)".to_owned(),
                Some(Mark::Alias(term)) => format!(" (alias of `{term}`)"),
                Some(Mark::Node) => " (node)".to_owned(),
                None => String::new(),
            };
            let _ = writeln!(out, "  {} {}{mark}", subject.text, subject.count);
        }
    }
    let counts: Vec<String> = RuleId::TERM_REPORT
        .iter()
        .map(|rule| {
            let count = report
                .groups
                .iter()
                .filter(|group| group.rule == *rule)
                .count();
            format!("{rule} {count}")
        })
        .collect();
    let _ = writeln!(out, "groups: {}", counts.join(", "));
    out
}

fn document_json(report: &TermReport, labels: &Labels<'_>) -> Value {
    let groups: Vec<Value> = report
        .groups
        .iter()
        .map(|group| group_json(group, labels))
        .collect();
    let products: Vec<Value> = report
        .products
        .iter()
        .map(|product| product_json(product, labels))
        .collect();
    json::result(
        SCHEMA,
        "reported",
        json!({ "groups": groups, "products": products }),
        Vec::new(),
    )
}

fn group_json(group: &Group, labels: &Labels<'_>) -> Value {
    let phrases: Vec<Value> = group
        .phrases
        .iter()
        .map(|phrase| {
            let nodes: Vec<Value> = phrase
                .nodes
                .iter()
                .map(|key| json!({ "key": key, "label": labels.of(*key) }))
                .collect();
            json!({ "text": phrase.text, "count": phrase.count, "nodes": nodes })
        })
        .collect();
    json!({
        "rule": group.rule.as_str(),
        "severity": group.severity.as_str(),
        "message": group.message,
        "phrases": phrases,
    })
}

fn product_json(product: &ProductSubjects, labels: &Labels<'_>) -> Value {
    let subjects: Vec<Value> = product
        .subjects
        .iter()
        .map(|subject| {
            let (mark, term) = match &subject.mark {
                Some(Mark::Term) => (Some("term"), None),
                Some(Mark::Alias(term)) => (Some("alias"), Some(term.as_str())),
                Some(Mark::Node) => (Some("node"), None),
                None => (None, None),
            };
            json!({ "text": subject.text, "count": subject.count, "mark": mark, "term": term })
        })
        .collect();
    json!({
        "key": product.product,
        "label": product.product.and_then(|key| labels.of(key)),
        "subjects": subjects,
    })
}

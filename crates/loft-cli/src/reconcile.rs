//! `loft reconcile` (the `loft reconcile` Feature): `loft.trace.yaml` compared with the tree and
//! with the files that hold the tests, each reconciliation finding one line with an exit code a
//! CI job can gate on, and the links a reviewer has checked accepted. It writes only the trace
//! file, and only when it accepts a link; a node that cannot be accepted writes nothing.

use std::collections::HashMap;
use std::path::Path;
use std::process::ExitCode;

use loftspec::check::hashed;
use loftspec::codec::{self, Sites};
use loftspec::document::Document;
use loftspec::hash::DocumentHashes;
use loftspec::identity::{Key, Label};
use loftspec::ids::NodeId;
use loftspec::lint::Severity;
use loftspec::trace::{
    self, Reconciled, TRACE, Trace, TraceError, TraceFinding, TraceRule, accept, accept_stale,
    reconcile,
};
use loftspec::yaml::{Location, Position};
use loftspec_store::FileStore;
use loftspec_store::file::ProjectFiles;
use serde_json::{Map, Value, json};

use crate::json;
use crate::stop::Stop;

/// The schema `--format json` writes to, by name and version.
const SCHEMA: &str = "loft-reconcile/1";

/// What the command was asked to do beyond reporting.
pub struct Options<'a> {
    pub fail_on: Option<Severity>,
    /// The nodes whose links to accept, each by its key or label.
    pub accept: &'a [String],
    /// Whether to accept every unreviewed or stale link that can be accepted.
    pub accept_stale: bool,
}

/// The tree and the trace file, read.
struct Read {
    store: FileStore,
    document: Document,
    sites: Sites,
    hashes: DocumentHashes,
    trace: Trace,
}

pub fn run(root: &Path, options: &Options<'_>, as_json: bool) -> ExitCode {
    let (read, accepted) = match read(root).and_then(|read| accepted(read, options)) {
        Ok(done) => done,
        Err(stop) => return stop.exit(SCHEMA, as_json),
    };
    let reconciled = reconcile(
        &read.document,
        &read.hashes,
        &read.trace,
        &read.store.test_files(),
    );
    let failed = options.fail_on.is_some_and(|threshold| {
        reconciled
            .findings
            .iter()
            .any(|finding| finding.severity >= threshold)
    });
    let report = Report::of(&read.document, &read.sites);
    if as_json {
        let outcome = if failed { "failed" } else { "passed" };
        json::print(&report.document(&reconciled, &accepted, outcome));
    } else {
        report.print(&reconciled, &accepted);
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn read(root: &Path) -> Result<Read, Stop> {
    let store = FileStore::open(root)?;
    let (document, sites) = codec::read_sited(&store.read_files()?)?;
    let hashes = hashed(&document, &sites).map_err(|errors| Stop::invalid(&errors))?;
    let trace = trace::read(store.trace()?.as_deref()).map_err(|errors| trace_stop(&errors))?;
    Ok(Read {
        store,
        document,
        sites,
        hashes,
        trace,
    })
}

/// Accepts the links the options name and writes the trace file when any is accepted, giving
/// the nodes whose links it accepted.
fn accepted(mut read: Read, options: &Options<'_>) -> Result<(Read, Vec<Key>), Stop> {
    let files = read.store.test_files();
    let keys = if options.accept_stale {
        accept_stale(&mut read.trace, &read.document, &read.hashes, &files)
    } else if options.accept.is_empty() {
        Vec::new()
    } else {
        let keys = named(&read.document, options.accept)?;
        accept_named(&mut read, &keys, &files)?;
        keys
    };
    if !keys.is_empty() {
        let text = trace::write(&read.trace, &read.document);
        read.store.write_trace(text)?;
    }
    Ok((read, keys))
}

fn accept_named(read: &mut Read, keys: &[Key], files: &ProjectFiles) -> Result<(), Stop> {
    accept(&mut read.trace, keys, &read.document, &read.hashes, files).map_err(|refusals| {
        let report = Report::of(&read.document, &read.sites);
        let messages = refusals
            .iter()
            .map(|refusal| format!("cannot accept {}", report.name(refusal.key())))
            .zip(refusals.iter().map(ToString::to_string));
        Stop::all(
            "accept",
            messages
                .map(|(name, why)| format!("{name}: {why}"))
                .collect(),
        )
    })
}

/// The key of each node named by its key or its label; a name that is neither stops the command.
fn named(document: &Document, names: &[String]) -> Result<Vec<Key>, Stop> {
    let mut unknown = Vec::new();
    let keys = names
        .iter()
        .filter_map(|name| {
            let key = Key::parse(name).ok().or_else(|| {
                let label = Label::parse(name).ok()?;
                Some(document.bindings.resolve(label)?.key())
            });
            if key.is_none() {
                unknown.push(format!(
                    "cannot accept {name}: no node of the tree carries it"
                ));
            }
            key
        })
        .collect();
    if unknown.is_empty() {
        Ok(keys)
    } else {
        Err(Stop::all("accept", unknown))
    }
}

/// The trace file's errors, each at its line and column.
fn trace_stop(errors: &[TraceError]) -> Stop {
    Stop {
        errors: errors
            .iter()
            .map(|error| {
                let at = match error.location {
                    Some(Location::At(position)) => Some(position),
                    _ => None,
                };
                let mut fields = trace_site(at);
                fields.insert("kind".to_owned(), json!("trace"));
                fields.insert("message".to_owned(), json!(error.problem.to_string()));
                fields.insert("sites".to_owned(), json!([Value::Object(trace_site(at))]));
                Value::Object(fields)
            })
            .collect(),
        lines: errors.iter().map(ToString::to_string).collect(),
    }
}

/// A place in the trace file as a site's fields.
fn trace_site(position: Option<Position>) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert("path".to_owned(), json!(TRACE));
    if let Some(position) = position {
        fields.insert("line".to_owned(), json!(position.line));
        fields.insert("column".to_owned(), json!(position.column));
    }
    fields
}

/// What the report needs to name a node: its label and where it is written.
struct Report<'a> {
    ids: HashMap<Key, NodeId>,
    document: &'a Document,
    sites: &'a Sites,
}

impl<'a> Report<'a> {
    fn of(document: &'a Document, sites: &'a Sites) -> Self {
        let mut ids = HashMap::new();
        document.walk(&mut |node| {
            if let Some(id) = node.id() {
                ids.insert(id.key(), id);
            }
        });
        Self {
            ids,
            document,
            sites,
        }
    }

    fn label(&self, key: Key) -> Option<String> {
        let id = self.ids.get(&key)?;
        self.document
            .bindings
            .label_of(*id)
            .map(|label| label.to_string())
    }

    /// A node by its label, or its key while it has none.
    fn name(&self, key: Key) -> String {
        self.label(key).unwrap_or_else(|| key.to_string())
    }

    /// Where a finding is: its node's site, or its link's place in the trace file.
    fn site(&self, finding: &TraceFinding) -> Map<String, Value> {
        match self.sites.get(finding.node) {
            Some(site) => json::site(site),
            None => trace_site(finding.link),
        }
    }

    fn site_text(&self, finding: &TraceFinding) -> String {
        match (self.sites.get(finding.node), finding.link) {
            (Some(site), _) => site.to_string(),
            (None, Some(position)) => format!("{TRACE}:{position}"),
            (None, None) => TRACE.to_owned(),
        }
    }

    fn print(&self, reconciled: &Reconciled, accepted: &[Key]) {
        for key in accepted {
            println!("accepted {}", self.name(*key));
        }
        for finding in &reconciled.findings {
            let label = self.label(finding.node).unwrap_or_else(|| "-".to_owned());
            println!(
                "{}: {} {}: {label}: {}",
                self.site_text(finding),
                finding.severity,
                finding.rule,
                finding.message
            );
        }
        println!("{}", summary(reconciled));
    }

    fn document(&self, reconciled: &Reconciled, accepted: &[Key], outcome: &str) -> Value {
        let findings: Vec<Value> = reconciled
            .findings
            .iter()
            .map(|finding| {
                let mut fields = self.site(finding);
                fields.insert("rule".to_owned(), json!(finding.rule.as_str()));
                fields.insert("severity".to_owned(), json!(finding.severity.as_str()));
                fields.insert("key".to_owned(), json!(finding.node.to_string()));
                fields.insert("label".to_owned(), json!(self.label(finding.node)));
                fields.insert("message".to_owned(), json!(finding.message));
                if let Some(test) = &finding.test {
                    fields.insert(
                        "test".to_owned(),
                        json!({ "file": test.file, "name": test.name }),
                    );
                }
                Value::Object(fields)
            })
            .collect();
        let accepted: Vec<String> = accepted.iter().map(ToString::to_string).collect();
        let fields = json!({
            "findings": findings,
            "inScope": reconciled.in_scope,
            "linked": reconciled.linked,
            "accepted": accepted,
        });
        json::result(SCHEMA, outcome, fields, Vec::new())
    }
}

/// The counts: nodes in scope, how many a link names, and the findings of each rule.
fn summary(reconciled: &Reconciled) -> String {
    let counts: Vec<String> = TraceRule::ALL
        .iter()
        .map(|rule| {
            let count = reconciled
                .findings
                .iter()
                .filter(|finding| finding.rule == *rule)
                .count();
            format!("{rule} {count}")
        })
        .collect();
    format!(
        "{} of {} nodes in scope linked; {}",
        reconciled.linked,
        reconciled.in_scope,
        counts.join(", ")
    )
}

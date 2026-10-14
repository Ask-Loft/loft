//! `loft reconcile` (the `loft reconcile` Feature): `loft.trace.yaml` compared with the tree and
//! with the files that hold the tests, each reconciliation finding one line with an exit code a
//! CI job can gate on, and the links a reviewer has checked accepted. It writes only the trace
//! file, and only when it accepts a link; a node that cannot be accepted writes nothing.

use std::fmt;
use std::path::Path;
use std::process::ExitCode;

use loftspec::check::hashed;
use loftspec::codec::{self, Site, Sites};
use loftspec::document::Document;
use loftspec::hash::DocumentHashes;
use loftspec::identity::{Key, Label};
use loftspec::lint::Severity;
use loftspec::trace::{
    self, Reconciled, TRACE, Trace, TraceFinding, TraceRule, accept, accept_stale, reconcile,
};
use loftspec::yaml::Location;
use loftspec_store::FileStore;
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
    let mut read = match read(root) {
        Ok(read) => read,
        Err(stop) => return stop.exit(SCHEMA, as_json),
    };
    let accepted = match accepted(&mut read, options) {
        Ok(accepted) => accepted,
        Err(stop) => return stop.exit(SCHEMA, as_json),
    };
    let reconciled = reconcile(
        &read.document,
        &read.hashes,
        &read.trace,
        &read.store.test_files(),
    );
    let severities = reconciled.findings.iter().map(TraceFinding::severity);
    let failed = json::fails_on(options.fail_on, severities);
    let report = Report {
        document: &read.document,
        sites: &read.sites,
    };
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
    let trace = trace::read(store.trace()?.as_deref())?;
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
fn accepted(read: &mut Read, options: &Options<'_>) -> Result<Vec<Key>, Stop> {
    let files = read.store.test_files();
    let keys = if options.accept_stale {
        accept_stale(&mut read.trace, &read.document, &read.hashes, &files)
    } else if options.accept.is_empty() {
        Vec::new()
    } else {
        let keys = named(&read.document, options.accept)?;
        accept(&mut read.trace, &keys, &read.document, &read.hashes, &files).map_err(
            |refusals| {
                let report = Report {
                    document: &read.document,
                    sites: &read.sites,
                };
                let messages = refusals.iter().map(|refusal| {
                    format!("cannot accept {}: {refusal}", report.name(refusal.key()))
                });
                Stop::all("accept", messages.collect())
            },
        )?;
        keys
    };
    if !keys.is_empty() {
        let text = trace::write(&read.trace, &read.document);
        read.store.write_trace(text)?;
    }
    Ok(keys)
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

/// Where a finding is: its node's site, or, for a node the tree does not carry, its link's place
/// in the trace file.
enum Place<'a> {
    Node(&'a Site),
    Link(Option<Location>),
}

impl Place<'_> {
    fn fields(&self) -> Map<String, Value> {
        match self {
            Self::Node(site) => json::site(site),
            Self::Link(location) => json::site_at(&TRACE, *location),
        }
    }
}

impl fmt::Display for Place<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Node(site) => site.fmt(formatter),
            Self::Link(Some(location)) => write!(formatter, "{TRACE}:{location}"),
            Self::Link(None) => formatter.write_str(TRACE),
        }
    }
}

/// What the report needs to name a node: its label and where it is written.
struct Report<'a> {
    document: &'a Document,
    sites: &'a Sites,
}

impl Report<'_> {
    fn label(&self, key: Key) -> Option<String> {
        let label = self.document.bindings.label_of_key(key)?;
        Some(label.to_string())
    }

    /// A node by its label, or its key while it has none.
    fn name(&self, key: Key) -> String {
        self.label(key).unwrap_or_else(|| key.to_string())
    }

    fn place(&self, finding: &TraceFinding) -> Place<'_> {
        match self.sites.get(finding.node) {
            Some(site) => Place::Node(site),
            None => Place::Link(finding.link.map(Location::At)),
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
                self.place(finding),
                finding.severity(),
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
                let mut fields = self.place(finding).fields();
                fields.insert("rule".to_owned(), json!(finding.rule.as_str()));
                fields.insert("severity".to_owned(), json!(finding.severity().as_str()));
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

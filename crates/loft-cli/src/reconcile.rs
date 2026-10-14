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
use loftspec::ids::ElementId;
use loftspec::lint::Severity;
use loftspec::trace::{
    self, Reconciled, TRACE, TestFiles, Trace, TraceFinding, TraceRule, accept, accept_stale,
    reconcile,
};
use loftspec::yaml::Location;
use loftspec_store::FileStore;
use serde_json::{Map, Value, json};

use crate::json;
use crate::out::outln;
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
        accept_named(read, options.accept, &files)?
    };
    if !keys.is_empty() {
        let text = trace::write(&read.trace, &read.document);
        read.store.write_trace(text)?;
    }
    Ok(keys)
}

/// Accepts the links of the nodes `names` gives, each by its key or its label; any name that
/// names no node, or whose link cannot be accepted, stops the command with every refusal in the
/// order of the names. The trace file is then not written, so links accepted in `read` stay
/// unwritten.
fn accept_named(
    read: &mut Read,
    names: &[String],
    files: &impl TestFiles,
) -> Result<Vec<Key>, Stop> {
    let resolved: Vec<(&String, Result<Key, &str>)> = names
        .iter()
        .map(|name| (name, resolve(&read.document, name)))
        .collect();
    let keys: Vec<Key> = resolved.iter().filter_map(|(_, key)| key.ok()).collect();
    let refusals = accept(&mut read.trace, &keys, &read.document, &read.hashes, files)
        .err()
        .unwrap_or_default();
    let messages: Vec<String> = resolved
        .iter()
        .filter_map(|(name, key)| {
            let why = match key {
                Err(why) => (*why).to_owned(),
                Ok(key) => refusals
                    .iter()
                    .find(|refusal| refusal.key() == *key)?
                    .to_string(),
            };
            Some(format!("cannot accept {name}: {why}"))
        })
        .collect();
    if messages.is_empty() {
        Ok(keys)
    } else {
        Err(Stop::all("accept", messages))
    }
}

/// The key a name gives: the name itself when it is a key, which the reconciler then checks, or
/// the node its label names.
fn resolve(document: &Document, name: &str) -> Result<Key, &'static str> {
    if let Ok(key) = Key::parse(name) {
        return Ok(key);
    }
    let label = Label::parse(name).map_err(|_| "it is neither a key nor a label")?;
    document
        .bindings
        .resolve(label)
        .map(ElementId::key)
        .ok_or("no node of the tree carries this label")
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
            outln!("accepted {}", self.name(*key));
        }
        for finding in &reconciled.findings {
            let label = self.label(finding.node).unwrap_or_else(|| "-".to_owned());
            outln!(
                "{}: {} {}: {label}: {}",
                self.place(finding),
                finding.severity(),
                finding.rule,
                finding.message
            );
        }
        outln!("{}", summary(reconciled));
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

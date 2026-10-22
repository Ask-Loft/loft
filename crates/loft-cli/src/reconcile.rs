//! `loft reconcile` (the `loft reconcile` Feature): `loft.trace.yaml` compared with the tree and
//! with the files that hold the tests, each reconciliation finding one line with an exit code a
//! CI job can gate on, and the links a reviewer has checked accepted. Each test file is parsed by
//! the test locator. It writes only the trace file, and only when it accepts a link; a node that
//! cannot be accepted writes nothing.

use std::fmt;
use std::path::Path;

use loftspec::canonical::Digest;
use loftspec::check::hashed;
use loftspec::codec::{self, Site, Sites, site_fields};
use loftspec::document::Document;
use loftspec::hash::DocumentHashes;
use loftspec::identity::{Key, Label};
use loftspec::ids::ElementId;
use loftspec::lint::Severity;
use loftspec::trace::{
    self, Evidence, Found, Reconciled, TRACE, TestFiles, Trace, TraceFinding, TraceRule, accept,
    accept_stale, reconcile,
};
use loftspec::yaml::Location;
use loftspec_store::FileStore;
use serde_json::{Map, Value, json};

use crate::finish::Finished;
use crate::json;
use crate::locate::Parsed;
use crate::out::outln;
use crate::stop::Stop;

/// The schema `--format json` writes to, by name and version.
const SCHEMA: &str = "loft-reconcile/3";

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

pub fn run(root: &Path, options: &Options<'_>, as_json: bool) -> Finished {
    let mut read = match read(root) {
        Ok(read) => read,
        Err(stop) => return stop.exit(SCHEMA, as_json),
    };
    let files = Parsed::new(read.store.test_files());
    let Accepted {
        keys: accepted,
        recorded,
    } = match accepted(&mut read, options, &files) {
        Ok(accepted) => accepted,
        Err(stop) => return stop.exit(SCHEMA, as_json),
    };
    let reconciled = reconcile(&read.document, &read.hashes, &read.trace, &files);
    let severities = reconciled.findings.iter().map(TraceFinding::severity);
    let failed = json::fails_on(options.fail_on, severities);
    let report = Report {
        document: &read.document,
        sites: &read.sites,
    };
    let outcome = if failed { "failed" } else { "passed" };
    if as_json {
        json::print(&report.document(&read.trace, &reconciled, &accepted, recorded, outcome));
    } else {
        report.print(&reconciled, &accepted, recorded);
    }
    if failed {
        Finished::failure(outcome)
    } else {
        Finished::success(outcome)
    }
}

/// The document `loft reconcile --format json` prints for the tree under `root`, with no
/// `--fail-on` and nothing accepted, as a `loft` view of Yard Log records it. It writes nothing.
pub fn json_document(root: &Path) -> Result<Value, Stop> {
    let read = read(root)?;
    let files = Parsed::new(read.store.test_files());
    let reconciled = reconcile(&read.document, &read.hashes, &read.trace, &files);
    let report = Report {
        document: &read.document,
        sites: &read.sites,
    };
    Ok(report.document(&read.trace, &reconciled, &[], 0, "passed"))
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

/// The nodes whose links were accepted, and the number of tests whose body was recorded.
struct Accepted {
    keys: Vec<Key>,
    recorded: usize,
}

/// Accepts the links the options name, and writes the trace file when any is accepted or a body
/// is recorded.
fn accepted(
    read: &mut Read,
    options: &Options<'_>,
    files: &impl TestFiles,
) -> Result<Accepted, Stop> {
    let before = read.trace.clone();
    let keys = if options.accept_stale {
        accept_stale(&mut read.trace, &read.document, &read.hashes, files)
    } else if options.accept.is_empty() {
        Vec::new()
    } else {
        accept_named(read, options.accept, files)?
    };
    let recorded = recorded(&before, &read.trace);
    if !keys.is_empty() || recorded > 0 {
        let text = trace::write(&read.trace, &read.document);
        read.store.write_trace(text)?;
    }
    Ok(Accepted { keys, recorded })
}

/// The number of tests that had no `body` in `before` and have one in `after`, the same trace
/// with links accepted.
fn recorded(before: &Trace, after: &Trace) -> usize {
    let tests = |trace: &Trace| -> Vec<Option<Digest>> {
        trace
            .links
            .iter()
            .flat_map(|link| match &link.evidence {
                Evidence::Tests(tests) => tests.iter().map(|test| test.body).collect(),
                Evidence::Unverified(_) => Vec::new(),
            })
            .collect()
    };
    tests(before)
        .into_iter()
        .zip(tests(after))
        .filter(|(before, after)| before.is_none() && after.is_some())
        .count()
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
            Self::Node(site) => site.fields(),
            Self::Link(location) => site_fields(&TRACE, *location),
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

    fn print(&self, reconciled: &Reconciled, accepted: &[Key], recorded: usize) {
        for key in accepted {
            outln!("accepted {}", self.name(*key));
        }
        if recorded > 0 {
            let tests = if recorded == 1 { "test" } else { "tests" };
            outln!("recorded the body of {recorded} {tests}");
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

    fn document(
        &self,
        trace: &Trace,
        reconciled: &Reconciled,
        accepted: &[Key],
        recorded: usize,
        outcome: &str,
    ) -> Value {
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
            "recorded": recorded,
            "tests": tests(trace, reconciled),
        });
        json::result(SCHEMA, outcome, fields, Vec::new())
    }
}

/// Each test of each link, in the order of the trace file: its node, file and name, whether it
/// was located, and the first and last line of each of its locations.
fn tests(trace: &Trace, reconciled: &Reconciled) -> Vec<Value> {
    let mut listed = Vec::new();
    for (link, found) in trace.links.iter().zip(&reconciled.found) {
        let Evidence::Tests(tests) = &link.evidence else {
            continue;
        };
        for (test, found) in tests.iter().zip(found) {
            let mut fields = json!({
                "key": link.node.to_string(),
                "file": test.file,
                "name": test.name,
                "located": matches!(found, Found::Located { .. }),
            });
            if let Found::Located { lines, .. } = found {
                let locations: Vec<Value> = lines
                    .iter()
                    .map(|lines| json!({ "firstLine": lines.first, "lastLine": lines.last }))
                    .collect();
                fields["locations"] = json!(locations);
            }
            listed.push(fields);
        }
    }
    listed
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

//! `loft status` (the `loft status` Feature): where the tree stands, for an agent host or a
//! person between edits. It reports the lock and whether the tree matches it, the state of the
//! repository that holds it, and, with `--base <revision>`, the nodes added, changed and removed
//! since HEAD left the revision. It writes nothing and exits 0 whatever it finds; a tree that
//! does not read, a conflict record and a base it cannot read stop it with exit 1.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use loftspec::codec::{Site, Sites};
use loftspec::diff::content_changes;
use loftspec::document::Document;
use loftspec::identity::Key;
use loftspec::ids::NodeId;
use loftspec::publish::{LOCK, Lock, Operation, ReadTree, RepoState, Root, first_difference};
use loftspec_store::repository::{GitCommand, Repository as _};
use loftspec_store::{FileStore, StoreError};
use serde_json::{Map, Value, json};

use crate::base;
use crate::finish::Finished;
use crate::json;
use crate::opened::Opened;
use crate::out::outln;
use crate::stop::Stop;

/// The schema `--format json` writes to, by name and version.
const SCHEMA: &str = "loft-status/2";
const OUTCOME: &str = "reported";

/// Where the tree stands: what `loft status` reports.
pub struct Report {
    lock: Option<LockReport>,
    repository: RepoState,
    base: Option<BaseReport>,
}

/// The lock and the first root of the tree that differs from it, where one does.
struct LockReport {
    lock: Lock,
    differs: Option<Root>,
}

/// The nodes added, changed and removed since HEAD left `revision` at `commit`.
struct BaseReport {
    revision: String,
    commit: String,
    added: Vec<Listed>,
    changed: Vec<Listed>,
    removed: Vec<Listed>,
}

/// A node as the report lists it.
struct Listed {
    key: String,
    label: Option<String>,
    kind: &'static str,
    site: Site,
    name: String,
}

pub fn run(root: &Path, base: Option<&str>, as_json: bool) -> Finished {
    let report = match attempt(root, base) {
        Ok(report) => report,
        Err(stop) => return stop.exit(SCHEMA, as_json),
    };
    if as_json {
        json::print(&report.document());
    } else {
        for line in lines(&report) {
            outln!("{line}");
        }
    }
    Finished::success(OUTCOME)
}

fn attempt(root: &Path, base: Option<&str>) -> Result<Report, Stop> {
    let opened = Opened::open(root)?;
    let tree = ReadTree::read(&opened.files)?;
    let git = GitCommand::default();
    let mut report = Report::of(&opened.store, &tree, || repository(&git, root))?;
    if let Some(revision) = base {
        report.compare(&base::read_tree(&git, root, revision)?, &tree);
    }
    Ok(report)
}

/// The state of the repository that holds the tree under `root`.
///
/// # Errors
///
/// The tree is not inside a git repository, or git fails.
pub fn repository(git: &GitCommand, root: &Path) -> Result<RepoState, Stop> {
    git.state(root).map_err(repository_error)
}

impl Report {
    /// Where `tree`, read from `store`, stands: its lock and whether it matches it, and the state
    /// of the repository `repository` gives.
    ///
    /// # Errors
    ///
    /// The tree does not validate, its lock does not read, or `repository` fails.
    pub fn of(
        store: &FileStore,
        tree: &ReadTree,
        repository: impl FnOnce() -> Result<RepoState, Stop>,
    ) -> Result<Self, Stop> {
        // The tree's own state root, which a publish would record, is under the highest view.
        let (roots, state) = tree.roots().map_err(|errors| Stop::invalid(&errors))?;
        let lock = store
            .lock()
            .map_err(|error| lock_error(&error))?
            .map(|lock| {
                let differs = first_difference(&lock, &roots, state).map(|(root, ..)| root);
                LockReport { lock, differs }
            });
        Ok(Self {
            lock,
            repository: repository()?,
            base: None,
        })
    }

    /// Adds the nodes `tree` added, changed and removed since `base`.
    pub fn compare(&mut self, base: &base::Tree, tree: &ReadTree) {
        self.base = Some(base_report(base, tree.document(), tree.sites()));
    }

    /// The paths of the files that hold a node added or changed since the base, where the report
    /// compares with one.
    pub fn changed_paths(&self) -> BTreeSet<String> {
        let Some(base) = &self.base else {
            return BTreeSet::new();
        };
        base.added
            .iter()
            .chain(&base.changed)
            .map(|listed| listed.site.path.to_string())
            .collect()
    }

    /// The document `loft status --format json` prints.
    pub fn document(&self) -> Value {
        json::result(SCHEMA, OUTCOME, fields(self), Vec::new())
    }
}

fn lock_error(error: &StoreError) -> Stop {
    let kind = if matches!(error, StoreError::Lock(..)) {
        "lock"
    } else {
        "store"
    };
    Stop::one(kind, error.to_string())
}

fn repository_error(error: StoreError) -> Stop {
    match error {
        StoreError::NotARepository(root) => Stop::one(
            "store",
            format!(
                "{}: not inside a git repository; `loft status` reports the state of one",
                root.display()
            ),
        ),
        error => error.into(),
    }
}

/// The nodes `document` added, changed and removed since `base`.
fn base_report(base: &base::Tree, document: &Document, sites: &Sites) -> BaseReport {
    let changes = content_changes(&base.document, document);
    let now = Nodes::of(document, sites);
    let then = Nodes::of(&base.document, &base.sites);
    BaseReport {
        revision: base.revision.clone(),
        commit: base.commit.clone(),
        added: changes.added.iter().map(|id| now.listed(*id)).collect(),
        changed: changes.changed.iter().map(|id| now.listed(*id)).collect(),
        removed: changes.removed.iter().map(|id| then.listed(*id)).collect(),
    }
}

/// The keyed nodes of a document by key, with what the report lists of each.
struct Nodes<'a> {
    document: &'a Document,
    sites: &'a Sites,
    by_key: BTreeMap<Key, (&'static str, String)>,
}

impl<'a> Nodes<'a> {
    fn of(document: &'a Document, sites: &'a Sites) -> Self {
        let mut by_key = BTreeMap::new();
        document.walk(&mut |node| {
            if let Some(id) = node.id() {
                by_key.insert(id.key(), (node.kind().as_str(), node.name().into_owned()));
            }
        });
        Self {
            document,
            sites,
            by_key,
        }
    }

    fn listed(&self, id: NodeId) -> Listed {
        let key = id.key();
        let (kind, name) = self.by_key[&key].clone();
        Listed {
            key: key.to_string(),
            label: self
                .document
                .bindings
                .label_of(id)
                .map(|label| label.to_string()),
            kind,
            site: self
                .sites
                .get(key)
                .cloned()
                .expect("every element read from a tree has a site"),
            name,
        }
    }
}

fn fields(report: &Report) -> Value {
    let lock = report.lock.as_ref().map(|report| {
        json!({
            "publishSequence": report.lock.publish_sequence.get(),
            "publishedAt": report.lock.published_at.to_string(),
            "stateRoot": report.lock.state_root.to_string(),
            "matches": report.differs.is_none(),
            "differs": report.differs.map(|root| format!("{}Root", root.name())),
        })
    });
    let stopped = report
        .repository
        .stopped
        .as_ref()
        .map(|stopped| operation(stopped.operation));
    let mut fields = Map::new();
    fields.insert("lock".to_owned(), json!(lock));
    fields.insert(
        "repository".to_owned(),
        json!({
            "branch": report.repository.branch,
            "modified": report.repository.modified,
            "stopped": stopped,
        }),
    );
    if let Some(base) = &report.base {
        let nodes = |listed: &[Listed]| listed.iter().map(listed_json).collect::<Vec<_>>();
        fields.insert(
            "base".to_owned(),
            json!({
                "revision": base.revision,
                "commit": base.commit,
                "added": nodes(&base.added),
                "changed": nodes(&base.changed),
                "removed": nodes(&base.removed),
            }),
        );
    }
    Value::Object(fields)
}

const fn operation(operation: Operation) -> &'static str {
    match operation {
        Operation::Merge => "merge",
        Operation::Rebase => "rebase",
    }
}

fn listed_json(listed: &Listed) -> Value {
    let mut fields = Map::new();
    fields.insert("key".to_owned(), json!(listed.key));
    fields.insert("label".to_owned(), json!(listed.label));
    fields.insert("kind".to_owned(), json!(listed.kind));
    fields.extend(listed.site.fields());
    fields.insert("name".to_owned(), json!(listed.name));
    Value::Object(fields)
}

/// The report as lines of text.
fn lines(report: &Report) -> Vec<String> {
    let mut lines = vec![match &report.lock {
        None => format!("lock: none; the tree has not been published ({LOCK})"),
        Some(LockReport { lock, differs }) => {
            let at = format!(
                "lock: publish {} at {}, state root {}",
                lock.publish_sequence, lock.published_at, lock.state_root
            );
            match differs {
                None => format!("{at}; the tree matches it"),
                Some(root) => format!("{at}; the tree's {root} differs"),
            }
        }
    }];
    let repository = &report.repository;
    lines.push(match &repository.branch {
        Some(branch) => format!("branch: {branch}"),
        None => "branch: none; HEAD is detached".to_owned(),
    });
    if let Some(stopped) = &repository.stopped {
        lines.push(format!("stopped: a {}", operation(stopped.operation)));
    }
    for path in &repository.modified {
        lines.push(format!("uncommitted: {path}"));
    }
    if let Some(base) = &report.base {
        lines.push(format!(
            "base: {} at {}: {} added, {} changed, {} removed",
            base.revision,
            base.commit,
            base.added.len(),
            base.changed.len(),
            base.removed.len()
        ));
        for (change, listed) in [
            ("added", &base.added),
            ("changed", &base.changed),
            ("removed", &base.removed),
        ] {
            lines.extend(listed.iter().map(|each| listed_line(change, each)));
        }
    }
    lines
}

/// A listed node as one line: its change, key, label, kind, site and name, two spaces apart.
fn listed_line(change: &str, listed: &Listed) -> String {
    let label = listed.label.as_deref().unwrap_or("-");
    format!(
        "{change}  {}  {label}  {}  {}  {}",
        listed.key, listed.kind, listed.site, listed.name
    )
}

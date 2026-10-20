//! `loft publish` (F-41): the publisher run over the tree inside a git repository. It prints
//! each label it allocated with its key, each label it withdrew, each citation of a withdrawn
//! label it rewrote, the new publish sequence and state root, and in a stopped merge or rebase
//! the git command that finishes it; that nothing changed; or why it refused (RQ-145). A
//! refusal writes nothing and exits 1. It runs no git command that changes the repository.

use std::path::Path;

use loftspec::check::Placed;
use loftspec::codec::ReadError;
use loftspec::publish::{
    Allocated, Lock, Made, Operation, Publication, PublishedAt, ReadTree, Refusal, Relabel, publish,
};
use loftspec_store::clock::{Clock as _, SystemClock};
use loftspec_store::repository::{GitCommand, Repository as _};
use loftspec_store::{FileStore, StoreError};
use serde_json::{Value, json};

use crate::finish::Finished;
use crate::json;
use crate::out::outln;
use crate::stop::Stop;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-publish/2";

/// What `loft publish` reports: a lock it wrote or that stands, and what the publish did.
struct Report {
    lock: Lock,
    outcome: &'static str,
    given_back: Vec<Allocated>,
    allocated: Vec<Allocated>,
    rewritten: Vec<Relabel>,
    withdrawn: Vec<Relabel>,
    finish: Option<Operation>,
}

/// The refusal of a connected document, which `loft publish` cannot send to its remote.
const CONNECTED: &str = "the document is connected: a connected document is published only \
                         through its remote, and its labels come from its remote";

/// Why a publish did not happen.
enum Failure {
    Read(Vec<ReadError>),
    Invalid(Vec<Placed>),
    Refused(String),
}

impl<E: ToString> From<E> for Failure {
    fn from(error: E) -> Self {
        Self::Refused(error.to_string())
    }
}

pub fn run(root: &Path, as_json: bool) -> Finished {
    match attempt(root) {
        Ok(report) => {
            if as_json {
                json::print(&published(&report));
            } else {
                print(&report);
            }
            Finished::success(report.outcome)
        }
        Err(failure) => stop(failure).exit(SCHEMA, as_json),
    }
}

/// Reads the tree, then the repository, the previous lock and the time, runs the publisher and
/// writes what it gives in one save. In a stopped merge or rebase a lock that is not a lock
/// record, such as one holding conflict markers, is read as none.
fn attempt(root: &Path) -> Result<Report, Failure> {
    let store = FileStore::open(root)?;
    let files = store.read_files()?;
    let tree = ReadTree::read(&files).map_err(Failure::Read)?;
    let repo = GitCommand::default().state(root)?;
    let previous = match store.lock() {
        Err(StoreError::Lock(..)) if repo.stopped.is_some() => None,
        lock => lock?,
    };
    let at = PublishedAt::from_unix_seconds(SystemClock::from_env().now()?)
        .map_err(|error| Failure::Refused(format!("SOURCE_DATE_EPOCH: {error}")))?;
    // `loft publish` never publishes as the document's remote.
    let publication =
        publish(tree, Some(&repo), previous.as_ref(), at, Made::NotAsRemote).map_err(refusal)?;
    let report = match publication {
        Publication::Published {
            lock,
            files,
            given_back,
            allocated,
            rewritten,
            withdrawn,
            finish,
            ..
        } => {
            store.publish(files, &lock)?;
            Report {
                lock,
                outcome: "published",
                given_back,
                allocated,
                rewritten,
                withdrawn,
                finish,
            }
        }
        Publication::Unchanged(lock) => Report {
            lock,
            outcome: "unchanged",
            given_back: Vec::new(),
            allocated: Vec::new(),
            rewritten: Vec::new(),
            withdrawn: Vec::new(),
            finish: None,
        },
        // Without a remote to send it to, a connected document's publish goes no further.
        Publication::Prepared { .. } => return Err(Failure::Refused(CONNECTED.to_owned())),
    };
    Ok(report)
}

fn refusal(refusal: Refusal) -> Failure {
    match refusal {
        Refusal::Invalid(errors) => Failure::Invalid(errors),
        Refusal::Modified(paths) => Failure::Refused(format!(
            "uncommitted changes under spec/: {}; commit them before publishing",
            paths.join(", ")
        )),
        Refusal::Branch {
            current: Some(current),
            publish,
        } => Failure::Refused(format!(
            "on branch `{current}`, but the publish branch is `{publish}`"
        )),
        Refusal::Branch {
            current: None,
            publish,
        } => Failure::Refused(format!(
            "HEAD is detached, not on the publish branch `{publish}`"
        )),
        Refusal::Unmerged(paths) => Failure::Refused(format!(
            "unmerged paths other than spec/labels.yaml and loft.lock.json: {}; resolve them \
             before publishing",
            paths.join(", ")
        )),
        Refusal::UnmergedBindings => Failure::Refused(
            "spec/labels.yaml is unmerged but holds no conflict record, so `loft merge-driver` \
             did not merge it: git runs the driver only when `loft` is on git's search path, \
             and the driver writes nothing when it reports an error. Fix what stopped it, run \
             `git checkout -m spec/labels.yaml` to merge the file again, and publish again"
                .to_owned(),
        ),
        Refusal::Lock { revision, message } => Failure::Refused(format!(
            "the lock at {revision} is not a lock record: {message}"
        )),
        Refusal::Connected => Failure::Refused(format!(
            "{CONNECTED}, so a stopped merge or rebase settles no labels of it"
        )),
    }
}

fn print(report: &Report) {
    let Report { lock, .. } = report;
    if report.outcome == "unchanged" {
        outln!(
            "nothing changed since publish sequence {}, state root {}",
            lock.publish_sequence,
            lock.state_root
        );
        return;
    }
    for given in &report.given_back {
        outln!("{} {} (given back)", given.label, given.element.key());
    }
    for allocated in &report.allocated {
        outln!("{} {}", allocated.label, allocated.element.key());
    }
    for withdrawn in &report.withdrawn {
        outln!(
            "withdrew {} from {}, which now holds {}",
            withdrawn.withdrawn,
            withdrawn.element,
            withdrawn.label
        );
    }
    for rewritten in &report.rewritten {
        outln!(
            "rewrote {} as {} in {}",
            rewritten.withdrawn,
            rewritten.label,
            rewritten.element
        );
    }
    outln!(
        "published sequence {}, state root {}",
        lock.publish_sequence,
        lock.state_root
    );
    if let Some(operation) = report.finish {
        outln!("to finish: {}", finishing(operation));
    }
}

/// A failed publish as the errors that stop the command.
fn stop(failure: Failure) -> Stop {
    match failure {
        Failure::Read(errors) => errors.into(),
        Failure::Invalid(errors) => {
            let mut stop = Stop::invalid(&errors);
            let count = errors.len();
            stop.lines.push(format!(
                "publish refused: the validator reports {count} errors"
            ));
            stop
        }
        Failure::Refused(reason) => Stop {
            errors: vec![json::error("refused", &reason, &[])],
            lines: vec![format!("publish refused: {reason}")],
        },
    }
}

/// The git command that finishes a stopped merge or rebase once a publish settled it.
const fn finishing(operation: Operation) -> &'static str {
    match operation {
        Operation::Merge => "git add -A && git commit --no-edit",
        Operation::Rebase => "git add -A && git rebase --continue",
    }
}

fn published(report: &Report) -> Value {
    let mut fields = json!({
        "givenBack": labels(&report.given_back),
        "allocated": labels(&report.allocated),
        "rewritten": relabels(&report.rewritten),
        "withdrawn": relabels(&report.withdrawn),
        "publishSequence": report.lock.publish_sequence.get(),
        "stateRoot": report.lock.state_root.to_string(),
    });
    if let Some(operation) = report.finish {
        fields["finish"] = json!(finishing(operation));
    }
    json::result(SCHEMA, report.outcome, fields, Vec::new())
}

/// Labels a publish gave, each with its key.
fn labels(given: &[Allocated]) -> Vec<Value> {
    given
        .iter()
        .map(|given| {
            json!({
                "label": given.label.to_string(),
                "key": given.element.key().to_string(),
            })
        })
        .collect()
}

/// Labels a publish replaced, each with the element's key, the label it withdrew and the label
/// that took its place.
fn relabels(relabels: &[Relabel]) -> Vec<Value> {
    relabels
        .iter()
        .map(|relabel| {
            json!({
                "key": relabel.element.to_string(),
                "withdrawn": relabel.withdrawn.to_string(),
                "label": relabel.label.to_string(),
            })
        })
        .collect()
}

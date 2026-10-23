//! The `loft` views a `loft` block records (the Record a Loft view Feature), computed by this
//! `loft`'s own library over the tree that holds Yard Log, never by another `loft`. Each report
//! holds the document the matching command prints, as it prints it, under the command's name:
//! `status` for a `spec-change` or a `publish` view, `check` for `findings`, `reconcile` for
//! `trace`; a `decision` view holds the decision as the tree holds it. A list longer than
//! [`MOST`] items keeps its first [`MOST`], and the report counts what it left out.

mod decision;
mod graph;
mod trace;

use std::path::Path;

use loftspec::publish::{ReadTree, RepoState};
use loftspec_store::repository::{GitCommand, MergeBase, Repository as _};
use serde_json::{Map, Value, json};
use yardlog::Time;
use yardlog::body::{Recorded, View};
use yardlog::datum::Datum;
use yardlog::input::LoftRequest;

use crate::opened::Opened;
use crate::stop::Stop;
use crate::{base, check, status, version};

/// The most items of one list a report keeps.
pub const MOST: usize = 1_000;

/// Computes the `loft` views of a batch over the tree under `tree`, the directory the command
/// runs in, whose `spec/` the matching commands run there read; at `at`, comparing with `base`
/// where a view names no base revision.
pub struct Views<'a> {
    pub tree: &'a Path,
    pub base: &'a str,
    pub at: Time,
}

impl Views<'_> {
    /// The report of the view a block asks for, and where and when it was computed.
    ///
    /// # Errors
    ///
    /// The tree holds no specification that reads, or the view cannot be computed; the error
    /// says why.
    pub fn report(&self, request: &LoftRequest) -> Result<(Recorded, Datum), String> {
        let reported = self.reported(request);
        let (recorded, mut report) = reported.map_err(|stop| stop.lines.join("; "))?;
        keep_most(&mut report, request.view);
        Ok((recorded, Datum::from(&report)))
    }

    /// Where and when the report is computed, and the report of the view `request` asks for:
    /// the repository's state is asked for once, and the tree is read once, for both.
    fn reported(&self, request: &LoftRequest) -> Result<(Recorded, Value), Stop> {
        let git = GitCommand::default();
        let state = git.state(self.tree)?;
        let recorded = self.recorded(&git, &state);
        let opened = Opened::open(self.tree)?;
        let base = request.base.as_deref().unwrap_or(self.base);
        let report = match request.view {
            View::SpecChange => {
                let tree = ReadTree::read(&opened.files)?;
                let based = base::read_tree(&git, self.tree, base)?;
                let mut status = status::Report::of(&opened.store, &tree, || Ok(state))?;
                status.compare(&based, &tree);
                json!({
                    "status": status.document(),
                    "graph": graph::report(&based.document, tree.document()),
                })
            }
            View::Publish => self.publish(&git, &opened, state)?,
            View::Findings => {
                let based = base::read(&git, self.tree, base)?;
                let baseline = check::Baseline {
                    revision: base,
                    commit: &based.commit,
                    files: &based.files,
                };
                let check = check::Report::of(&opened.files, Some(baseline))?;
                let (document, _) = loftspec::codec::read_sited(&opened.files)?;
                let nodes = graph::named(&document, &check.subjects());
                json!({ "check": check.document(check.outcome(None)), "nodes": nodes })
            }
            View::Trace => trace::report(&trace::Over {
                git: &git,
                root: self.tree,
                opened: &opened,
                state,
                base,
            })?,
            View::Decision => {
                let named = request.decision.as_deref().ok_or_else(|| {
                    Stop::one("view", "a `decision` view names its decision".to_owned())
                })?;
                decision::report(&opened.files, named)?
            }
        };
        Ok((recorded, report))
    }

    /// The lock and whether the tree matches it, compared with the commit that last wrote the
    /// lock where one did, and the nodes and decisions that carry a label, counted by kind.
    fn publish(&self, git: &GitCommand, opened: &Opened, state: RepoState) -> Result<Value, Stop> {
        // git has just told the repository's state, so a `git log` that fails here is one on a
        // branch with no commit yet, which no commit of the lock precedes: the view then compares
        // with nothing, as for a lock no commit wrote.
        let written = git.lock_written(self.tree).ok().flatten();
        let tree = ReadTree::read(&opened.files)?;
        let mut status = status::Report::of(&opened.store, &tree, || Ok(state))?;
        if let Some(written) = written {
            status.compare(&base::read_tree(git, self.tree, &written)?, &tree);
        }
        Ok(json!({ "status": status.document(), "labels": graph::labels(tree.document()) }))
    }

    /// Where and when the report is computed: the commit, the branch, whether `spec/` held
    /// changes not committed, as `state` gives them, and this `loft`'s version.
    fn recorded(&self, git: &GitCommand, state: &RepoState) -> Recorded {
        // `git merge-base HEAD HEAD` names HEAD's commit; a branch with no commit has none.
        let commit = match git.merge_base(self.tree, "HEAD") {
            Ok(MergeBase::Commit(commit)) => Some(commit),
            Ok(MergeBase::Unrelated) | Err(_) => None,
        };
        Recorded {
            commit,
            branch: state.branch.clone(),
            dirty: !state.modified.is_empty(),
            loft: version::VERSION.to_owned(),
            at: self.at,
        }
    }
}

/// The lists of each view's report that [`keep_most`] shortens, by their path in the report.
fn lists(view: View) -> &'static [&'static [&'static str]] {
    match view {
        View::SpecChange => &[
            &["status", "base", "added"],
            &["status", "base", "changed"],
            &["status", "base", "removed"],
            &["graph", "nodes"],
            &["graph", "decisions"],
            &["graph", "edges"],
        ],
        View::Publish => &[
            &["status", "base", "added"],
            &["status", "base", "changed"],
            &["status", "base", "removed"],
        ],
        View::Findings => &[
            &["check", "findings"],
            &["check", "resolved"],
            &["check", "errors"],
        ],
        View::Trace => &[
            &["reconcile", "findings"],
            &["reconcile", "tests"],
            &["features"],
        ],
        View::Decision => &[],
    }
}

/// Keeps the first [`MOST`] items of each long list of a report, and records each list's whole
/// count and the number of its items left out, under `counts` and `leftOut` by the list's path.
fn keep_most(report: &mut Value, view: View) {
    let mut counts = Map::new();
    let mut left_out = Map::new();
    for path in lists(view) {
        let Some(Value::Array(items)) = path
            .iter()
            .try_fold(&mut *report, |at, name| at.get_mut(*name))
        else {
            continue;
        };
        let name = path.join(".");
        counts.insert(name.clone(), json!(items.len()));
        if items.len() > MOST {
            left_out.insert(name, json!(items.len() - MOST));
            items.truncate(MOST);
        }
    }
    report["counts"] = Value::Object(counts);
    report["leftOut"] = Value::Object(left_out);
}

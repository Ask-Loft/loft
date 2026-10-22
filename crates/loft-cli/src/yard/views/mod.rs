//! The `loft` views a `loft` block records (the Record a Loft view Feature), computed by this
//! `loft`'s own library over the tree that holds Yard Log, never by another `loft`. Each report
//! holds the document the matching command prints, as it prints it, under the command's name:
//! `status` for a `spec-change` or a `publish` view, `check` for `findings`, `reconcile` for
//! `trace`; a `decision` view holds the decision as the tree holds it. A list longer than
//! [`MOST`] items keeps its first [`MOST`], and the report counts what it left out.

mod decision;
mod graph;
mod trace;

use std::path::{Path, PathBuf};

use loftspec_store::repository::{GitCommand, MergeBase, Repository as _};
use serde_json::{Map, Value, json};
use yardlog::Time;
use yardlog::body::{Recorded, View};
use yardlog::datum::Datum;
use yardlog::input::LoftRequest;
use yardlog::keys::Handle;

use crate::stop::Stop;
use crate::{check, status, version};

/// The most items of one list a report keeps.
pub const MOST: usize = 1_000;

/// The directory whose `spec/` the views read: the nearest of `dir` and the directories above it,
/// up to the worktree's `top`, that holds a `spec/` directory, as `loft` itself is run where its
/// specification lies, below the top of the worktree or at it; `dir` where none does, so that
/// the view says it finds no specification.
#[must_use]
pub fn spec_root(dir: &Path, top: &Path) -> PathBuf {
    let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_owned());
    let top = top.canonicalize().unwrap_or_else(|_| top.to_owned());
    dir.ancestors()
        .take_while(|each| each.starts_with(&top))
        .find(|each| each.join("spec").is_dir())
        .unwrap_or(&dir)
        .to_owned()
}

/// Computes the `loft` views of a batch over the tree under `tree`, at `at`, comparing with
/// `base` where a view names no base revision.
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
    pub fn report(
        &self,
        _block: &Handle,
        request: &LoftRequest,
    ) -> Result<(Recorded, Datum), String> {
        let reported = self
            .recorded()
            .and_then(|recorded| Ok((recorded, self.compute(request)?)));
        let (recorded, mut report) = reported.map_err(|stop| stop.lines.join("; "))?;
        keep_most(&mut report, request.view);
        Ok((recorded, Datum::from(&report)))
    }

    /// The report of the view `request` asks for.
    fn compute(&self, request: &LoftRequest) -> Result<Value, Stop> {
        let base = request.base.as_deref().unwrap_or(self.base);
        Ok(match request.view {
            View::SpecChange => json!({
                "status": status::json_document(self.tree, Some(base))?,
                "graph": self.graph(base)?,
            }),
            View::Publish => self.publish()?,
            View::Findings => {
                let check = check::json_document(self.tree, Some(base))?;
                let files = crate::stop::read_files(self.tree)?;
                let (document, _) = loftspec::codec::read_sited(&files)?;
                let nodes = graph::named_by_findings(&document, &check);
                json!({ "check": check, "nodes": nodes })
            }
            View::Trace => trace::report(self.tree, base)?,
            View::Decision => {
                let named = request.decision.as_deref().ok_or_else(|| {
                    Stop::one("view", "a `decision` view names its decision".to_owned())
                })?;
                decision::report(self.tree, named)?
            }
        })
    }

    /// The lock and whether the tree matches it, compared with the commit that last wrote the
    /// lock where one did, and the nodes and decisions that carry a label, counted by kind.
    fn publish(&self) -> Result<Value, Stop> {
        let written = GitCommand::default().lock_written(self.tree).ok().flatten();
        let status = status::json_document(self.tree, written.as_deref())?;
        let files = crate::stop::read_files(self.tree)?;
        let (document, _) = loftspec::codec::read_sited(&files)?;
        Ok(json!({ "status": status, "labels": graph::labels(&document) }))
    }

    /// The diagram of the change from where HEAD left `revision` to the tree.
    fn graph(&self, revision: &str) -> Result<Value, Stop> {
        let files = crate::stop::read_files(self.tree)?;
        let (document, _) = loftspec::codec::read_sited(&files)?;
        let base = crate::base::read_tree(&GitCommand::default(), self.tree, revision)?;
        Ok(graph::report(&base.document, &document))
    }

    /// Where and when the report is computed: the commit, the branch, whether `spec/` held
    /// changes not committed, and this `loft`'s version.
    fn recorded(&self) -> Result<Recorded, Stop> {
        let git = GitCommand::default();
        let state = git.state(self.tree)?;
        // `git merge-base HEAD HEAD` names HEAD's commit; a branch with no commit has none.
        let commit = match git.merge_base(self.tree, "HEAD") {
            Ok(MergeBase::Commit(commit)) => Some(commit),
            Ok(MergeBase::Unrelated) | Err(_) => None,
        };
        Ok(Recorded {
            commit,
            branch: state.branch,
            dirty: !state.modified.is_empty(),
            loft: version::VERSION.to_owned(),
            at: self.at,
        })
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

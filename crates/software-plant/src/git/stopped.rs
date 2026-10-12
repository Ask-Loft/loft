//! A merge or a rebase that stopped on a conflict, as the git simulator scripts it: the commit
//! being merged or replayed, the branch a rebase rebases, the lock each revision holds, and the
//! index entries of each unmerged path.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Reply, Repository};

/// The calls with which Loft reads a stopped merge or rebase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StoppedCall {
    /// `git rev-parse -q --verify MERGE_HEAD`
    MergeHead,
    /// `git rev-parse -q --verify REBASE_HEAD`
    RebaseHead,
    /// `git rev-parse --git-path <path>`
    GitPath,
    /// `git ls-files -u -z --full-name -- :/`
    Unmerged,
    /// `git cat-file blob <revision>:./loft.lock.json`
    LockAt,
}

/// What stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
    Merge,
    Rebase,
}

/// A stopped merge or rebase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Stopped {
    pub operation: Operation,
    /// The commit `MERGE_HEAD` or `REBASE_HEAD` names.
    pub head: String,
    /// What `rebase-merge/head-name` holds during a rebase: `refs/heads/<branch>`, or
    /// `detached HEAD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_name: Option<String>,
    /// The text of `loft.lock.json` at each revision that holds one.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub locks: BTreeMap<String, String>,
    /// The index entries of the unmerged paths, in the order git lists them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unmerged: Vec<IndexEntry>,
}

/// One index entry of an unmerged path, its path from the top level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IndexEntry {
    pub mode: String,
    pub object: String,
    pub stage: u8,
    pub path: String,
}

/// The call the arguments make and its operand, after any `-C` pairs were taken off.
pub(super) fn parse<'a>(rest: &[&'a str]) -> Option<(StoppedCall, Vec<&'a str>)> {
    match rest {
        ["rev-parse", "-q", "--verify", "MERGE_HEAD"] => Some((StoppedCall::MergeHead, vec![])),
        ["rev-parse", "-q", "--verify", "REBASE_HEAD"] => Some((StoppedCall::RebaseHead, vec![])),
        ["rev-parse", "--git-path", path] => Some((StoppedCall::GitPath, vec![*path])),
        ["ls-files", "-u", "-z", "--full-name", "--", ":/"] => {
            Some((StoppedCall::Unmerged, vec![]))
        }
        ["cat-file", "blob", object] => {
            let revision = object.strip_suffix(":./loft.lock.json")?;
            Some((StoppedCall::LockAt, vec![revision]))
        }
        _ => None,
    }
}

/// Answers a call in a repository, stopped or not.
pub(super) fn answer(
    call: StoppedCall,
    operands: &[&str],
    repository: &Repository,
    git_dir: Option<&str>,
) -> Reply {
    let stopped = repository.stopped.as_ref();
    let head = |operation| {
        stopped
            .filter(|stopped| stopped.operation == operation)
            .map_or_else(
                || Reply::fail(1, ""),
                |stopped| Reply::ok(format!("{}\n", stopped.head)),
            )
    };
    match call {
        StoppedCall::MergeHead => head(Operation::Merge),
        StoppedCall::RebaseHead => head(Operation::Rebase),
        StoppedCall::GitPath => {
            let base = git_dir.map_or_else(
                || "../".repeat(repository.prefix.matches('/').count()) + ".git",
                str::to_owned,
            );
            Reply::ok(format!("{base}/{}\n", operands[0]))
        }
        StoppedCall::Unmerged => Reply::ok(unmerged(stopped)),
        StoppedCall::LockAt => {
            let revision = operands[0];
            match stopped.and_then(|stopped| stopped.locks.get(revision)) {
                Some(lock) => Reply::ok(lock.clone()),
                None => Reply::fail(
                    128,
                    format!(
                        "fatal: path '{}loft.lock.json' does not exist in '{revision}'\n",
                        repository.prefix
                    ),
                ),
            }
        }
    }
}

fn unmerged(stopped: Option<&Stopped>) -> Vec<u8> {
    let entries = stopped.map_or(&[][..], |stopped| stopped.unmerged.as_slice());
    entries
        .iter()
        .flat_map(|entry| {
            format!(
                "{} {} {}\t{}\0",
                entry.mode, entry.object, entry.stage, entry.path
            )
            .into_bytes()
        })
        .collect()
}

//! A merge or a rebase that stopped on a conflict, as the git simulator scripts it: the commit
//! being merged or replayed, the branch a rebase rebases, the lock each revision holds, and the
//! index entries of each unmerged path.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Reply, Repository};

/// The lock's path from the working directory, after a revision.
const LOCK_AT: &str = ":./loft.lock.json";

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
    /// `git rev-parse -q --verify <revision>:./loft.lock.json`
    LockObject,
    /// `git cat-file blob <object>`
    Blob,
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
    /// The lock at each revision that holds one.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub locks: BTreeMap<String, LockBlob>,
    /// The index entries of the unmerged paths, in the order git lists them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unmerged: Vec<IndexEntry>,
}

/// The lock a revision holds: its object id and its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LockBlob {
    pub object: String,
    pub text: String,
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
        ["rev-parse", "-q", "--verify", object] => {
            let revision = object.strip_suffix(LOCK_AT)?;
            Some((StoppedCall::LockObject, vec![revision]))
        }
        ["cat-file", "blob", object] => Some((StoppedCall::Blob, vec![*object])),
        _ => None,
    }
}

/// Answers a call in a repository, stopped or not.
pub(super) fn answer(call: StoppedCall, operands: &[&str], repository: &Repository) -> Reply {
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
            let base = repository
                .git_dir
                .clone()
                .unwrap_or_else(|| "../".repeat(repository.prefix.matches('/').count()) + ".git");
            Reply::ok(format!("{base}/{}\n", operands[0]))
        }
        StoppedCall::Unmerged => Reply::ok(unmerged(stopped)),
        StoppedCall::LockObject => stopped
            .and_then(|stopped| stopped.locks.get(operands[0]))
            .map_or_else(
                || Reply::fail(1, ""),
                |lock| Reply::ok(format!("{}\n", lock.object)),
            ),
        StoppedCall::Blob => stopped
            .and_then(|stopped| {
                let mut locks = stopped.locks.values();
                locks.find(|lock| lock.object == operands[0])
            })
            .map_or_else(
                || {
                    Reply::fail(
                        128,
                        format!("fatal: Not a valid object name {}\n", operands[0]),
                    )
                },
                |lock| Reply::ok(lock.text.clone()),
            ),
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

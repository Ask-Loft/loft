//! The base commit that `loft status --base <revision>` compares the tree with, as the git
//! simulator scripts it: the commit `git merge-base HEAD <revision>` gives each revision, and
//! the files that commit holds under `spec/`, each with the object id git stores it under.

use serde::{Deserialize, Serialize};

use super::{GitScenario, Reply, Repository, scope, within};

/// The lock beside `spec/`, whose last writer a publish view reads.
const LOCK: &str = "loft.lock.json";

/// The calls with which Loft reads the tree at a base commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BaseCall {
    /// `git merge-base HEAD <revision>`
    MergeBase,
    /// `git ls-tree -r -z <commit> -- <pathspec>…`
    ListTree,
    /// `git cat-file --batch`, the object ids on standard input
    Batch,
    /// `git log -1 --format=%H -- loft.lock.json`
    LockWritten,
}

/// What `git merge-base HEAD <revision>` gives a revision, and the files of that commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Base {
    /// The commit, or none where the revision and HEAD share no history.
    pub commit: Option<String>,
    /// The commit's files, in the order git lists them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<BaseFile>,
}

/// One file of a base commit: its mode, its object id, its path from the top level and its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BaseFile {
    pub mode: String,
    pub object: String,
    pub path: String,
    pub text: String,
}

impl GitScenario {
    /// The same repository in which `git merge-base HEAD <revision>` gives `commit`, which holds
    /// no file until [`Self::file_at`] adds one.
    #[must_use]
    pub fn base(mut self, revision: &str, commit: &str) -> Self {
        let base = Base {
            commit: Some(commit.to_owned()),
            files: Vec::new(),
        };
        self.repository_mut()
            .bases
            .insert(revision.to_owned(), base);
        self
    }

    /// The same repository in which `commit` is the last commit to write `loft.lock.json`.
    #[must_use]
    pub fn lock_written_at(mut self, commit: &str) -> Self {
        self.repository_mut().lock_written = Some(commit.to_owned());
        self
    }

    /// The same repository in which `revision` and HEAD share no history.
    #[must_use]
    pub fn unrelated(mut self, revision: &str) -> Self {
        let base = Base {
            commit: None,
            files: Vec::new(),
        };
        self.repository_mut()
            .bases
            .insert(revision.to_owned(), base);
        self
    }

    /// The same repository with one more file in the commit that `revision` gives: its path
    /// from the top level and its text, stored as the object `object`.
    #[must_use]
    pub fn file_at(mut self, revision: &str, path: &str, object: &str, text: &str) -> Self {
        let base = self
            .repository_mut()
            .bases
            .get_mut(revision)
            .expect("a base for the revision");
        base.files.push(BaseFile {
            mode: "100644".to_owned(),
            object: object.to_owned(),
            path: path.to_owned(),
            text: text.to_owned(),
        });
        self
    }
}

/// The call the arguments make and its operands, after any `-C` pairs were taken off.
pub(super) fn parse<'a>(rest: &[&'a str]) -> Option<(BaseCall, Vec<&'a str>)> {
    match rest {
        ["merge-base", "HEAD", revision] => Some((BaseCall::MergeBase, vec![*revision])),
        ["ls-tree", "-r", "-z", commit, "--", pathspecs @ ..] if !pathspecs.is_empty() => {
            let mut operands = vec![*commit];
            operands.extend_from_slice(pathspecs);
            Some((BaseCall::ListTree, operands))
        }
        ["cat-file", "--batch"] => Some((BaseCall::Batch, Vec::new())),
        ["log", "-1", "--format=%H", "--", LOCK] => Some((BaseCall::LockWritten, Vec::new())),
        _ => None,
    }
}

/// git's answer to a revision or a commit it cannot find.
fn not_an_object(name: &str) -> Reply {
    Reply::fail(128, format!("fatal: Not a valid object name {name}\n"))
}

pub(super) fn answer(
    call: BaseCall,
    operands: &[&str],
    repository: &Repository,
    input: &[u8],
) -> Reply {
    match call {
        BaseCall::MergeBase => match repository.bases.get(operands[0]) {
            None => not_an_object(operands[0]),
            Some(Base { commit: None, .. }) => Reply::fail(1, ""),
            Some(Base {
                commit: Some(commit),
                ..
            }) => Reply::ok(format!("{commit}\n")),
        },
        BaseCall::ListTree => list_tree(repository, operands[0], &operands[1..]),
        BaseCall::Batch => Reply::ok(batch(repository, input)),
        BaseCall::LockWritten => Reply::ok(
            repository
                .lock_written
                .as_ref()
                .map_or_else(String::new, |commit| format!("{commit}\n")),
        ),
    }
}

/// The `-z` records of the commit's files under the pathspecs, each path from the working
/// directory.
fn list_tree(repository: &Repository, commit: &str, pathspecs: &[&str]) -> Reply {
    let Some(base) = repository
        .bases
        .values()
        .find(|base| base.commit.as_deref() == Some(commit))
    else {
        return not_an_object(commit);
    };
    let scopes: Vec<String> = pathspecs
        .iter()
        .map(|pathspec| scope(&repository.prefix, pathspec))
        .collect();
    let mut out = Vec::new();
    for file in &base.files {
        if !scopes.iter().any(|scope| within(&file.path, scope)) {
            continue;
        }
        let path = file
            .path
            .strip_prefix(&repository.prefix)
            .unwrap_or(&file.path);
        out.extend_from_slice(format!("{} blob {}\t{path}\0", file.mode, file.object).as_bytes());
    }
    Reply::ok(out)
}

/// Each object whose id is a line of `input`, after a header of its id, type and size, or a
/// line that says it is missing.
fn batch(repository: &Repository, input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for object in String::from_utf8_lossy(input).lines() {
        let found = repository
            .bases
            .values()
            .flat_map(|base| &base.files)
            .find(|file| file.object == object);
        match found {
            Some(file) => {
                out.extend_from_slice(format!("{object} blob {}\n", file.text.len()).as_bytes());
                out.extend_from_slice(file.text.as_bytes());
                out.push(b'\n');
            }
            None => out.extend_from_slice(format!("{object} missing\n").as_bytes()),
        }
    }
    out
}

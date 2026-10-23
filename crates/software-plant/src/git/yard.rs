//! The calls with which Yard Client finds Yard Log of a worktree, as the git simulator scripts
//! them: the top of the worktree, the repository's common directory, whether git ignores a
//! path, a name of git's configuration, and whether a revision holds a path.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{GitScenario, NOT_A_REPOSITORY, Reply, Repository};

/// The calls of Yard Client's store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum YardCall {
    /// `git rev-parse --show-toplevel`
    TopLevel,
    /// `git rev-parse --git-common-dir`
    CommonDir,
    /// `git rev-parse --show-toplevel --git-common-dir --git-path <name>`
    Locate,
    /// `git check-ignore -q <path>`
    Ignored,
    /// `git config --get <name>`
    Config,
    /// `git rev-parse -q --verify <revision>:<path>`
    ObjectAt,
}

/// What a repository holds for Yard Client's calls.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Yard {
    /// What `--git-common-dir` prints; the `.git` that `--git-path` is under when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub common_dir: Option<String>,
    /// The paths git ignores, each as `check-ignore` is given it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignored: Vec<String>,
    /// The object id of each `<revision>:<path>` a revision holds.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub objects: BTreeMap<String, String>,
}

impl Yard {
    pub(super) fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

impl GitScenario {
    /// The same repository whose common directory `--git-common-dir` prints as `dir`, as in a
    /// linked worktree, whose git directory lies under it.
    #[must_use]
    pub fn common_dir(mut self, dir: &str, git_dir: &str) -> Self {
        let repository = self.repository_mut();
        repository.yard.common_dir = Some(dir.to_owned());
        repository.git_dir = Some(git_dir.to_owned());
        self
    }

    /// The same repository, in which git ignores `path` as `check-ignore` is given it.
    #[must_use]
    pub fn ignoring(mut self, path: &str) -> Self {
        self.repository_mut().yard.ignored.push(path.to_owned());
        self
    }

    /// The same repository, in which `revision` holds `path` as the object `object`.
    #[must_use]
    pub fn holding(mut self, revision: &str, path: &str, object: &str) -> Self {
        self.repository_mut()
            .yard
            .objects
            .insert(format!("{revision}:{path}"), object.to_owned());
        self
    }
}

/// The call the arguments make and its operand, after any `-C` pairs were taken off.
pub(super) fn parse<'a>(rest: &[&'a str]) -> Option<(YardCall, Vec<&'a str>)> {
    match rest {
        ["rev-parse", "--show-toplevel"] => Some((YardCall::TopLevel, vec![])),
        ["rev-parse", "--git-common-dir"] => Some((YardCall::CommonDir, vec![])),
        [
            "rev-parse",
            "--show-toplevel",
            "--git-common-dir",
            "--git-path",
            name,
        ] => Some((YardCall::Locate, vec![*name])),
        ["check-ignore", "-q", path] => Some((YardCall::Ignored, vec![*path])),
        ["config", "--get", name] => Some((YardCall::Config, vec![*name])),
        ["rev-parse", "-q", "--verify", object] if object.contains(':') => {
            Some((YardCall::ObjectAt, vec![*object]))
        }
        _ => None,
    }
}

/// The directory a call runs in: the last `-C` it names, or the current directory.
pub(super) fn directory(args: &[String]) -> PathBuf {
    let named = args
        .windows(2)
        .filter(|pair| pair[0] == "-C")
        .map(|pair| PathBuf::from(&pair[1]))
        .next_back();
    named.unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

/// Answers a call outside any repository: git reads its configuration there too.
pub(super) fn outside(call: YardCall) -> Reply {
    match call {
        YardCall::Config => Reply::fail(1, ""),
        _ => Reply::fail(128, NOT_A_REPOSITORY),
    }
}

/// What `rev-parse --show-toplevel` prints from `dir`: the top of the worktree, resolved on disk.
fn top_level(repository: &Repository, dir: &Path) -> String {
    let mut top = dir.canonicalize().unwrap_or_else(|_| dir.to_owned());
    for _ in 0..repository.prefix.matches('/').count() {
        top.pop();
    }
    top.display().to_string()
}

/// What `rev-parse --git-common-dir` prints: the path the scenario gives it, or `.git` at the top.
fn common_dir(repository: &Repository) -> String {
    repository
        .yard
        .common_dir
        .clone()
        .unwrap_or_else(|| "../".repeat(repository.prefix.matches('/').count()) + ".git")
}

/// Answers a call in a repository, from the directory `dir` the call runs in.
pub(super) fn answer(
    call: YardCall,
    operands: &[&str],
    repository: &Repository,
    dir: &Path,
) -> Reply {
    match call {
        YardCall::TopLevel => Reply::ok(format!("{}\n", top_level(repository, dir))),
        YardCall::CommonDir => Reply::ok(format!("{}\n", common_dir(repository))),
        YardCall::Locate => Reply::ok(format!(
            "{}\n{}\n{}\n",
            top_level(repository, dir),
            common_dir(repository),
            super::stopped::git_path(repository, operands[0])
        )),
        YardCall::Ignored => {
            let ignored = repository
                .yard
                .ignored
                .iter()
                .any(|path| path == operands[0]);
            if ignored {
                Reply::ok("")
            } else {
                Reply::fail(1, "")
            }
        }
        YardCall::Config => match repository.config.get(operands[0]) {
            Some(value) => Reply::ok(format!("{value}\n")),
            None => Reply::fail(1, ""),
        },
        YardCall::ObjectAt => match repository.yard.objects.get(operands[0]) {
            Some(object) => Reply::ok(format!("{object}\n")),
            None => Reply::fail(1, ""),
        },
    }
}

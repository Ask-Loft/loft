//! What Yard Client asks of git, through `loft-ports`' runner: where the worktree's top is, which
//! branch it is on, where git keeps its own files for it and for the repository, whether it
//! ignores `.yard/`, who the person is, whether a revision holds a path, and which local branch
//! a branch was cut from. Nothing else in this crate runs git.

use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

use loft_ports::git::{Git, first_line, if_found, outside_a_repository, succeeded, yes_or_no};

use crate::YardStoreError;
use crate::place::GIT_DIR_NAME;

/// The top of the worktree, as an absolute path.
pub const TOP_LEVEL: [&str; 2] = ["rev-parse", "--show-toplevel"];
pub use loft_ports::git::{CURRENT_BRANCH, GIT_PATH};
/// The repository's common directory, from the working directory.
pub const COMMON_DIR: [&str; 2] = ["rev-parse", "--git-common-dir"];
/// The top of the worktree, the common directory and Yard's directory under the worktree's own
/// git directory, one per line, in one call.
pub const LOCATE: [&str; 5] = [
    "rev-parse",
    "--show-toplevel",
    "--git-common-dir",
    "--git-path",
    GIT_DIR_NAME,
];
/// Whether git ignores a path, completed by it; exit 1 when it does not.
pub const CHECK_IGNORE: [&str; 2] = ["check-ignore", "-q"];
/// A name's value in git's configuration, from every scope; exit 1 when it is unset.
pub const CONFIG_GET: [&str; 2] = ["config", "--get"];
/// The object a `<revision>:<path>` names; exit 1, with no message, when there is none.
pub const VERIFY: [&str; 3] = ["rev-parse", "-q", "--verify"];
/// The subject of each entry of a reference's reflog, newest first, completed by the reference
/// and `--`; exit 128, `bad revision`, for a reference with no reflog.
pub const REFLOG: [&str; 3] = ["reflog", "show", "--format=%gs"];
/// Whether a full reference name exists, completed by it; exit 1 when it does not.
pub const SHOW_REF: [&str; 3] = ["show-ref", "--verify", "-q"];

/// The subject git gives the reflog entry that created a branch, before what it was created from.
const CREATED_FROM: &str = "branch: Created from ";
/// The start of git's message for a reference that has no reflog, as of an unborn branch.
const BAD_REVISION: &str = "fatal: bad revision";

/// `.yard/` from the top of the worktree, as `check-ignore` is given it.
pub const YARD_PATHSPEC: &str = ":/.yard/";

/// The person, as git's configuration names them for the repository.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Person {
    pub name: Option<String>,
    pub email: Option<String>,
}

/// Git, asked Yard Client's questions.
#[derive(Default)]
pub struct YardGit {
    git: Git,
}

impl YardGit {
    /// Runs the git command that `command` starts, with the arguments of each call added.
    pub fn from_command(command: impl Fn() -> Command + 'static) -> Self {
        Self {
            git: Git::from_command(command),
        }
    }

    fn run(&self, dir: &Path, call: &[&str]) -> Result<Output, YardStoreError> {
        let output = self.git.run(dir, call)?;
        if outside_a_repository(&output) {
            return Err(YardStoreError::NotARepository(dir.to_owned()));
        }
        Ok(output)
    }

    /// The first line a call printed, where it succeeded.
    fn line(&self, dir: &Path, call: &[&str]) -> Result<String, YardStoreError> {
        Ok(first_line(&succeeded(call, self.run(dir, call)?)?))
    }

    /// Whether a call that answers with its exit code alone says yes (0) or no (1).
    fn yes(&self, dir: &Path, call: &[&str]) -> Result<bool, YardStoreError> {
        Ok(yes_or_no(call, self.run(dir, call)?)?)
    }

    /// The top of the worktree that holds `dir`.
    ///
    /// # Errors
    ///
    /// `dir` is in no repository, or git fails or cannot be run.
    pub fn top_level(&self, dir: &Path) -> Result<PathBuf, YardStoreError> {
        let top = self.line(dir, &TOP_LEVEL)?;
        if top.is_empty() {
            return Err(YardStoreError::Answer {
                call: TOP_LEVEL.join(" "),
                answer: top,
            });
        }
        Ok(PathBuf::from(top))
    }

    /// The branch the worktree that holds `dir` is on, or none on a detached HEAD.
    ///
    /// # Errors
    ///
    /// `dir` is in no repository, or git fails or cannot be run.
    pub fn branch(&self, dir: &Path) -> Result<Option<String>, YardStoreError> {
        let output = self.run(dir, &CURRENT_BRANCH)?;
        Ok(if_found(&CURRENT_BRANCH, output)?.map(|out| first_line(&out)))
    }

    /// In one call, the top of the worktree that holds `dir`, the repository's common directory,
    /// and `yard` under the worktree's own git directory, each as an absolute path.
    ///
    /// # Errors
    ///
    /// `dir` is in no repository, or git fails or cannot be run.
    pub fn locate(&self, dir: &Path) -> Result<(PathBuf, PathBuf, PathBuf), YardStoreError> {
        let printed =
            String::from_utf8_lossy(&succeeded(&LOCATE, self.run(dir, &LOCATE)?)?).into_owned();
        let mut lines = printed.lines();
        let mut next = || lines.next().unwrap_or_default().to_owned();
        let (top, common, yard) = (next(), next(), next());
        if top.is_empty() || common.is_empty() || yard.is_empty() {
            return Err(YardStoreError::Answer {
                call: LOCATE.join(" "),
                answer: printed,
            });
        }
        Ok((PathBuf::from(top), from(dir, &common), from(dir, &yard)))
    }

    /// The path `name` under the worktree's own git directory, as an absolute path.
    ///
    /// # Errors
    ///
    /// `dir` is in no repository, or git fails or cannot be run.
    pub fn git_path(&self, dir: &Path, name: &str) -> Result<PathBuf, YardStoreError> {
        let call: Vec<&str> = GIT_PATH.into_iter().chain([name]).collect();
        Ok(from(dir, &self.line(dir, &call)?))
    }

    /// Whether git ignores `.yard/` in the worktree that holds `dir`.
    ///
    /// # Errors
    ///
    /// `dir` is in no repository, or git fails or cannot be run.
    pub fn ignores_yard(&self, dir: &Path) -> Result<bool, YardStoreError> {
        let call: Vec<&str> = CHECK_IGNORE.into_iter().chain([YARD_PATHSPEC]).collect();
        self.yes(dir, &call)
    }

    /// The person, by the `user.name` and `user.email` that git gives for the repository.
    ///
    /// # Errors
    ///
    /// git fails or cannot be run.
    pub fn person(&self, dir: &Path) -> Result<Person, YardStoreError> {
        let value = |name: &str| -> Result<Option<String>, YardStoreError> {
            let call: Vec<&str> = CONFIG_GET.into_iter().chain([name]).collect();
            let output = self.git.run(dir, &call)?;
            Ok(if_found(&call, output)?.map(|out| first_line(&out)))
        };
        Ok(Person {
            name: value("user.name")?,
            email: value("user.email")?,
        })
    }

    /// Whether `revision` holds `path`, a path from the top of the worktree; no for a revision
    /// that names nothing.
    ///
    /// # Errors
    ///
    /// `dir` is in no repository, or git fails or cannot be run.
    pub fn holds(&self, dir: &Path, revision: &str, path: &str) -> Result<bool, YardStoreError> {
        if revision.is_empty() || revision.starts_with('-') {
            return Ok(false);
        }
        let object = format!("{revision}:{path}");
        let call: Vec<&str> = VERIFY.into_iter().chain([object.as_str()]).collect();
        self.yes(dir, &call)
    }

    /// The local branch that `branch` was created from, as the oldest entry of its reflog names
    /// it; none where that entry names no other local branch, such as `HEAD` or a commit, or
    /// where the branch has no reflog.
    ///
    /// # Errors
    ///
    /// `dir` is in no repository, or git fails or cannot be run.
    pub fn cut_from(&self, dir: &Path, branch: &str) -> Result<Option<String>, YardStoreError> {
        let reference = format!("refs/heads/{branch}");
        let call: Vec<&str> = REFLOG
            .into_iter()
            .chain([reference.as_str(), "--"])
            .collect();
        let output = self.run(dir, &call)?;
        if output.status.code() == Some(128)
            && String::from_utf8_lossy(&output.stderr).starts_with(BAD_REVISION)
        {
            return Ok(None);
        }
        let subjects = String::from_utf8_lossy(&succeeded(&call, output)?).into_owned();
        let from = subjects
            .lines()
            .next_back()
            .and_then(|oldest| oldest.strip_prefix(CREATED_FROM))
            .filter(|from| !from.is_empty() && !from.starts_with('-') && *from != branch);
        let Some(from) = from else {
            return Ok(None);
        };
        let local = format!("refs/heads/{from}");
        let call: Vec<&str> = SHOW_REF.into_iter().chain([local.as_str()]).collect();
        Ok(self.yes(dir, &call)?.then(|| from.to_owned()))
    }
}

/// A path git printed from the directory `dir`, made absolute, with `.` and `..` resolved.
fn from(dir: &Path, printed: &str) -> PathBuf {
    let mut path = PathBuf::new();
    for part in dir.join(printed).components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                path.pop();
            }
            other => path.push(other),
        }
    }
    path
}

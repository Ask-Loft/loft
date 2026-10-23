//! What Yard Client asks of git, through `loft-ports`' runner: where the worktree's top is, which
//! branch it is on, where git keeps its own files for it and for the repository, whether it
//! ignores `.yard/`, who the person is, and whether a revision holds a path. Nothing else in this
//! crate runs git.

use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

use loft_ports::git::{Git, first_line, outside_a_repository, succeeded};

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
        let output = self.run(dir, call)?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => succeeded(call, output).map(|_| true).map_err(Into::into),
        }
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
        if output.status.code() == Some(1) {
            return Ok(None);
        }
        Ok(Some(first_line(&succeeded(&CURRENT_BRANCH, output)?)))
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
            if output.status.code() == Some(1) {
                return Ok(None);
            }
            Ok(Some(first_line(&succeeded(&call, output)?)))
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

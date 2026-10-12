//! The repository port: what the publisher needs to know of the git repository that holds the
//! tree, a merge or rebase stopped in it included, and the local configuration that registers
//! the merge driver, through the `git` command (the accepted decision "Git is an external
//! component reached through the `git` command"). Nothing else in this crate runs git.

use std::path::Path;
use std::process::{Command, Output};
use std::{fs, io};

use loftspec::publish::{LOCK, Operation, RepoState, Stopped};

use crate::StoreError;
use crate::fs::{SPEC, hidden};

/// The working directory's path from the repository's top level.
pub const SHOW_PREFIX: [&str; 2] = ["rev-parse", "--show-prefix"];
/// The checked-out branch; exit 1 on a detached HEAD.
pub const CURRENT_BRANCH: [&str; 4] = ["symbolic-ref", "--quiet", "--short", "HEAD"];
/// The porcelain v1 records of the uncommitted changes under `spec/`.
pub const STATUS: [&str; 6] = [
    "status",
    "--porcelain=v1",
    "-z",
    "--untracked-files=all",
    "--",
    SPEC,
];

/// A name's value in the repository's local configuration; exit 1 when it is unset.
pub const CONFIG_GET: [&str; 3] = ["config", "--local", "--get"];
/// Sets a name in the repository's local configuration.
pub const CONFIG_SET: [&str; 2] = ["config", "--local"];

/// The commit a stopped merge merges; exit 1 when no merge stopped.
pub const MERGE_HEAD: [&str; 4] = ["rev-parse", "-q", "--verify", "MERGE_HEAD"];
/// The commit a stopped rebase replays; exit 1 when no rebase stopped.
pub const REBASE_HEAD: [&str; 4] = ["rev-parse", "-q", "--verify", "REBASE_HEAD"];
/// A path under the git directory, from the working directory, completed by the path.
pub const GIT_PATH: [&str; 2] = ["rev-parse", "--git-path"];
/// Where each backend of a stopped rebase keeps the branch it rebases.
pub const REBASE_HEAD_NAMES: [&str; 2] = ["rebase-merge/head-name", "rebase-apply/head-name"];
/// The index entries of every unmerged path in the repository, with their paths from the top
/// level.
pub const UNMERGED: [&str; 6] = ["ls-files", "-u", "-z", "--full-name", "--", ":/"];

/// The shell alias with which [`GitCommand::launcher_version`] runs `<command> --version`.
#[must_use]
pub fn launcher_call(command: &str) -> [String; 3] {
    [
        "-c".to_owned(),
        format!("alias.loft-launcher=!{command} --version"),
        "loft-launcher".to_owned(),
    ]
}

/// Where the publisher learns the repository's state.
pub trait Repository {
    /// The state of the repository that holds the directory `root`, which holds `spec/`.
    ///
    /// # Errors
    ///
    /// `root` is not inside a repository, the repository cannot be read, or git cannot be run.
    fn state(&self, root: &Path) -> Result<RepoState, StoreError>;
}

/// The repository read by running `git`.
pub struct GitCommand {
    command: Box<dyn Fn() -> Command>,
}

impl Default for GitCommand {
    /// Runs the `git` on the search path.
    fn default() -> Self {
        Self::from_command(|| Command::new("git"))
    }
}

impl GitCommand {
    /// Runs the git command that `command` starts, with the arguments of each call added.
    pub fn from_command(command: impl Fn() -> Command + 'static) -> Self {
        Self {
            command: Box::new(command),
        }
    }

    fn run(&self, root: &Path, call: &[&str]) -> Result<Output, StoreError> {
        (self.command)()
            .arg("-C")
            .arg(root)
            .args(call)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .output()
            .map_err(|error| match error.kind() {
                io::ErrorKind::NotFound => StoreError::GitUnavailable(error.to_string()),
                _ => StoreError::Git {
                    call: call.join(" "),
                    message: error.to_string(),
                },
            })
    }

    /// The working directory's path from the repository's top level.
    ///
    /// # Errors
    ///
    /// `root` is not inside a repository, or git fails or cannot be run.
    pub fn prefix(&self, root: &Path) -> Result<String, StoreError> {
        let call = SHOW_PREFIX;
        let output = self.run(root, &call)?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.code() == Some(128) && stderr.contains("not a git repository") {
            return Err(StoreError::NotARepository(root.to_owned()));
        }
        Ok(text(&succeeded(&call, output)?))
    }

    fn branch(&self, root: &Path) -> Result<Option<String>, StoreError> {
        let call = CURRENT_BRANCH;
        let output = self.run(root, &call)?;
        if output.status.code() == Some(1) {
            return Ok(None);
        }
        Ok(Some(text(&succeeded(&call, output)?)))
    }

    fn status(&self, root: &Path) -> Result<Vec<u8>, StoreError> {
        let call = STATUS;
        succeeded(&call, self.run(root, &call)?)
    }

    /// A name's value in the local configuration of the repository that holds `root`, or none.
    ///
    /// # Errors
    ///
    /// git fails or cannot be run.
    pub fn config_get(&self, root: &Path, name: &str) -> Result<Option<String>, StoreError> {
        let call: Vec<&str> = CONFIG_GET.into_iter().chain([name]).collect();
        let output = self.run(root, &call)?;
        if output.status.code() == Some(1) {
            return Ok(None);
        }
        Ok(Some(text(&succeeded(&call, output)?)))
    }

    /// The first line `<command> --version` prints when git runs it as it runs a merge driver:
    /// through the shell, from the top of the working tree; none when it fails.
    ///
    /// # Errors
    ///
    /// git cannot be run.
    pub fn launcher_version(
        &self,
        root: &Path,
        command: &str,
    ) -> Result<Option<String>, StoreError> {
        let call = launcher_call(command);
        let call: Vec<&str> = call.iter().map(String::as_str).collect();
        let output = self.run(root, &call)?;
        Ok(output.status.success().then(|| text(&output.stdout)))
    }

    /// Sets a name in the local configuration of the repository that holds `root`.
    ///
    /// # Errors
    ///
    /// git fails or cannot be run.
    pub fn config_set(&self, root: &Path, name: &str, value: &str) -> Result<(), StoreError> {
        let call: Vec<&str> = CONFIG_SET.into_iter().chain([name, value]).collect();
        succeeded(&call, self.run(root, &call)?).map(drop)
    }
}

impl GitCommand {
    /// The merge or rebase stopped in the repository, where one is: a rebase first, since a
    /// rebase replays each commit by merging it.
    fn stopped(
        &self,
        root: &Path,
        prefix: &str,
        branch: Option<&String>,
    ) -> Result<Option<Stopped>, StoreError> {
        let (operation, branch) = if self.verified(root, &REBASE_HEAD)? {
            (Operation::Rebase, self.rebased(root)?)
        } else if self.verified(root, &MERGE_HEAD)? {
            (Operation::Merge, branch.cloned())
        } else {
            return Ok(None);
        };
        let call = UNMERGED;
        let listed = succeeded(&call, self.run(root, &call)?)?;
        Ok(Some(Stopped {
            operation,
            branch,
            head_lock: self.lock_at(root, "HEAD")?,
            other_lock: self.lock_at(root, operation.other_head())?,
            unmerged: unmerged(&listed, prefix),
        }))
    }

    /// Whether a `rev-parse -q --verify` call names a commit.
    fn verified(&self, root: &Path, call: &[&str]) -> Result<bool, StoreError> {
        let output = self.run(root, call)?;
        if output.status.code() == Some(1) {
            return Ok(false);
        }
        succeeded(call, output).map(|_| true)
    }

    /// The branch a stopped rebase rebases, from the file either backend keeps it in; none
    /// for a rebase of a detached HEAD.
    fn rebased(&self, root: &Path) -> Result<Option<String>, StoreError> {
        for name in REBASE_HEAD_NAMES {
            let call: Vec<&str> = GIT_PATH.into_iter().chain([name]).collect();
            let path = text(&succeeded(&call, self.run(root, &call)?)?);
            match fs::read_to_string(root.join(path)) {
                Ok(held) => {
                    let branch = held.trim_end().strip_prefix("refs/heads/");
                    return Ok(branch.map(str::to_owned));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(StoreError::Git {
                        call: call.join(" "),
                        message: error.to_string(),
                    });
                }
            }
        }
        Ok(None)
    }

    /// The bytes of the lock beside `spec/` at `revision`, or none where it holds no lock.
    fn lock_at(&self, root: &Path, revision: &str) -> Result<Option<Vec<u8>>, StoreError> {
        let object = format!("{revision}:./{LOCK}");
        let call = ["cat-file", "blob", object.as_str()];
        let output = self.run(root, &call)?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        let missing = ["does not exist in", "exists on disk, but not in"];
        if output.status.code() == Some(128) && missing.iter().any(|m| stderr.contains(m)) {
            return Ok(None);
        }
        succeeded(&call, output).map(Some)
    }
}

impl Repository for GitCommand {
    fn state(&self, root: &Path) -> Result<RepoState, StoreError> {
        let prefix = self.prefix(root)?;
        let branch = self.branch(root)?;
        let status = self.status(root)?;
        let stopped = self.stopped(root, &prefix, branch.as_ref())?;
        Ok(RepoState {
            branch,
            modified: modified(&status, &prefix),
            stopped,
        })
    }
}

/// The paths of `ls-files -u -z` records, sorted and without duplicates: from the directory
/// whose path from the top level is `prefix` where they are under it, and otherwise from the
/// top level after `:/`.
fn unmerged(listed: &[u8], prefix: &str) -> Vec<String> {
    let mut paths: Vec<String> = listed
        .split(|byte| *byte == 0)
        .filter_map(|record| {
            let record = String::from_utf8_lossy(record);
            let (_, path) = record.split_once('\t')?;
            Some(
                path.strip_prefix(prefix)
                    .map_or_else(|| format!(":/{path}"), str::to_owned),
            )
        })
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

fn succeeded(call: &[&str], output: Output) -> Result<Vec<u8>, StoreError> {
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(StoreError::Git {
            call: call.join(" "),
            message: String::from_utf8_lossy(&output.stderr)
                .trim_end()
                .to_owned(),
        })
    }
}

/// The first line of a call's output.
fn text(stdout: &[u8]) -> String {
    String::from_utf8_lossy(stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// The paths of porcelain v1 `-z` records, from the directory whose path from the top level is
/// `prefix`, leaving out a path with a component that begins with `.`, as the reader does.
fn modified(status: &[u8], prefix: &str) -> Vec<String> {
    let mut fields = status
        .split(|byte| *byte == 0)
        .map(|field| String::from_utf8_lossy(field).into_owned());
    let mut paths = Vec::new();
    while let Some(record) = fields.next() {
        let Some((code, path)) = record.split_at_checked(3) else {
            continue;
        };
        paths.push(path.to_owned());
        if code.contains(['R', 'C'])
            && let Some(from) = fields.next()
        {
            paths.push(from);
        }
    }
    let mut paths: Vec<String> = paths
        .into_iter()
        .map(|path| path.strip_prefix(prefix).map(str::to_owned).unwrap_or(path))
        .filter(|path| !path.split('/').any(hidden))
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

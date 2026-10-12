//! The repository port: what the publisher needs to know of the git repository that holds the
//! tree, read through the `git` command (the accepted decision "Git is an external component
//! reached through the `git` command"). Nothing else in this crate runs git.

use std::io;
use std::path::Path;
use std::process::{Command, Output};

use loftspec::publish::RepoState;

use crate::StoreError;
use crate::fs::SPEC;

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
    fn prefix(&self, root: &Path) -> Result<String, StoreError> {
        let call = ["rev-parse", "--show-prefix"];
        let output = self.run(root, &call)?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.code() == Some(128) && stderr.contains("not a git repository") {
            return Err(StoreError::NotARepository(root.to_owned()));
        }
        Ok(text(&succeeded(&call, output)?))
    }

    fn branch(&self, root: &Path) -> Result<Option<String>, StoreError> {
        let call = ["symbolic-ref", "--quiet", "--short", "HEAD"];
        let output = self.run(root, &call)?;
        if output.status.code() == Some(1) {
            return Ok(None);
        }
        Ok(Some(text(&succeeded(&call, output)?)))
    }

    fn status(&self, root: &Path) -> Result<Vec<u8>, StoreError> {
        let call = [
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--",
            SPEC,
        ];
        succeeded(&call, self.run(root, &call)?)
    }
}

impl Repository for GitCommand {
    fn state(&self, root: &Path) -> Result<RepoState, StoreError> {
        let prefix = self.prefix(root)?;
        let branch = self.branch(root)?;
        let status = self.status(root)?;
        Ok(RepoState {
            branch,
            modified: modified(&status, &prefix),
        })
    }
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
        .filter(|path| !path.split('/').any(|part| part.starts_with('.')))
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

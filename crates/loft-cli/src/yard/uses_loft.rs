//! Whether an agent's working tree uses Loft (the Feature "Stay out of a repository that does not
//! use Loft"): where it, or a directory above it up to the top of its worktree, holds the
//! Format's manifest, its lock or a Yard Log of any branch; else where git's configuration sets
//! `loft.yard` to true. Yard Client's adapters ask it before anything else and do nothing where
//! the answer is no, so that the plugin can stay enabled in every repository.
//!
//! What a working tree holds is looked up on disk, with no git, so that a hook in a repository
//! that uses Loft pays for no process. The walk up ends at the directory that holds `.git`, a
//! directory in a repository and a file in a linked worktree or a submodule: what lies above the
//! top of the worktree belongs to another repository, or to none.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use yardlog::Agent;
use yardlog_store::git::YardGit;
use yardlog_store::home::Home;

use super::working_tree;

/// The name in git's configuration that turns Yard Client on where nothing in the working tree
/// shows that it uses Loft.
pub const SETTING: &str = "loft.yard";

/// The entry that marks the top of a worktree.
const GIT: &str = ".git";

/// What shows that a working tree uses Loft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    /// The Format's manifest, `spec/loftspec.yaml`.
    Spec,
    /// The Format's lock, `loft.lock.json`.
    Lock,
    /// A Yard Log of any branch under `.yard/`.
    YardLog,
    /// `loft.yard` set to true in git's configuration.
    Config,
}

impl Shown {
    /// The name `loft yard protocol` reports it by.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Spec => "spec",
            Self::Lock => "lock",
            Self::YardLog => "yard-log",
            Self::Config => "config",
        }
    }
}

/// What shows that the working tree of `dir` uses Loft, where something does.
pub fn of_dir(dir: &Path) -> Option<Shown> {
    let dir = real(dir);
    held(&dir).or_else(|| configured(&dir))
}

/// What shows that the working tree of `agent`, whose hook runs in `dir`, uses Loft: `dir`'s
/// own, else that of the worktree its working stage lies in, else the setting.
pub fn of_agent(dir: &Path, agent: &Agent) -> Option<Shown> {
    let dir = real(dir);
    held(&dir)
        .or_else(|| followed(&dir, agent))
        .or_else(|| configured(&dir))
}

/// `dir` by its real path, as git and Yard Log's place name it, where it resolves.
fn real(dir: &Path) -> PathBuf {
    dir.canonicalize().unwrap_or_else(|_| dir.to_owned())
}

/// The report `loft yard protocol` gives of the working tree of `dir`.
pub fn report(dir: &Path) -> Value {
    match of_dir(dir) {
        Some(shown) => json!({"usesLoft": true, "by": shown.name()}),
        None => json!({"usesLoft": false}),
    }
}

/// What `dir`, or a directory above it up to the top of its worktree, holds that shows Loft.
fn held(dir: &Path) -> Option<Shown> {
    let home = Home::from_env().map(|home| real(home.dir()));
    for each in dir.ancestors() {
        if each
            .join(loftspec_store::fs::SPEC)
            .join(loftspec::codec::MANIFEST)
            .is_file()
        {
            return Some(Shown::Spec);
        }
        if each.join(loftspec::publish::LOCK).is_file() {
            return Some(Shown::Lock);
        }
        let yard = each.join(yardlog_store::YARD);
        // The machine's own `~/.yard/` holds no Yard Log, and may hold many directories.
        if yard.is_dir()
            && home.as_ref() != Some(&yard)
            && yardlog_store::session::branches(each).is_ok_and(|branches| !branches.is_empty())
        {
            return Some(Shown::YardLog);
        }
        if each.join(GIT).symlink_metadata().is_ok() {
            break;
        }
    }
    None
}

/// What the agent's working tree holds, where its working stage lies in a worktree other than
/// `dir`; asked only of a session that wrote Yard Log in one.
fn followed(dir: &Path, agent: &Agent) -> Option<Shown> {
    let home = Home::from_env()?;
    let working = working_tree::resolve(Some(&home), agent, dir).ok()?;
    if working == dir {
        return None;
    }
    held(&working)
}

/// Whether git's configuration, from `dir`, sets `loft.yard` to a value git reads as true; a
/// value git reads as no boolean turns nothing on.
fn configured(dir: &Path) -> Option<Shown> {
    matches!(YardGit::default().flag(dir, SETTING), Ok(Some(true))).then_some(Shown::Config)
}

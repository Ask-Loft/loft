//! The branch's Yard Log a command acts on: the worktree that holds the command's directory,
//! the branch it is on, and the base branch whose tip decides whether a merge closed Yard Log.
//! Yard Log is opened settled: where a merge left it unsettled, the command settles it under
//! its lock, as one Act, before it acts.

use std::path::Path;

use serde_json::{Value, json};
use yardlog::codec::ROOT;
use yardlog::{Actor, Time, YardLog};
use yardlog_store::git::YardGit;
use yardlog_store::{Held, Place, YardStore, YardStoreError};

use super::args::Beyond;
use super::failure::Failure;
use super::page;

/// The branch's Yard Log, where a command finds it.
pub struct Branch {
    pub git: YardGit,
    pub store: YardStore,
    pub beyond: Beyond,
}

impl Branch {
    /// The Yard Log of the branch the worktree holding `dir` is on.
    pub fn locate(dir: &Path, beyond: Beyond) -> Result<Self, Failure> {
        let git = YardGit::default();
        let place = Place::locate(&git, dir)?;
        Self::at(git, place, beyond)
    }

    /// The Yard Log of the branch `place` is on.
    pub fn at(git: YardGit, place: Place, beyond: Beyond) -> Result<Self, Failure> {
        let store = YardStore::new(place)?;
        Ok(Self { git, store, beyond })
    }

    pub fn place(&self) -> &Place {
        self.store.place()
    }

    /// Where the result says it acted: the tree, the branch and the repository's common
    /// directory.
    pub fn located(&self) -> Value {
        let place = self.place();
        json!({
            "tree": place.tree.display().to_string(),
            "branch": self.store.branch(),
            "commonDir": place.common_dir.display().to_string(),
        })
    }

    /// The result's fields that every command acting on Yard Log gives: where it acted, the
    /// revision, and the branch's Yard Log on the page.
    pub fn fields(&self, revision: u64) -> Value {
        let mut fields = self.located();
        fields["revision"] = json!(revision);
        fields["page"] = page::address(&self.place().tree, self.store.branch(), self.beyond.port);
        fields
    }

    /// Whether the tip of the base branch `log`'s root records holds Yard Log's root, so that
    /// the merge closed it.
    pub fn closed_by_merge(&self, log: &YardLog) -> Result<bool, Failure> {
        let base = &log.root.base;
        if self.store.branch() == base {
            return Ok(false);
        }
        let root = format!("{}/{ROOT}", self.store.folder_rel());
        Ok(self.git.holds(&self.place().tree, base, &root)?)
    }

    /// Whether Yard Log as it is held is open, and closed by its merge all the same.
    pub fn merged(&self, held: &Held) -> Result<bool, Failure> {
        match log_of(held) {
            Some(log) if !log.is_closed() => self.closed_by_merge(log),
            _ => Ok(false),
        }
    }

    /// Yard Log as it is, settled first where a merge left it unsettled; `actor` makes the
    /// settling's Act at `at`.
    pub fn open(
        &self,
        at: Time,
        actor: impl FnOnce() -> Result<Actor, Failure>,
    ) -> Result<Held, Failure> {
        let held = self.store.read()?;
        let Held::Log(log) = &held else {
            return Ok(held);
        };
        if log.root.unsettled_since.is_none() {
            return Ok(held);
        }
        let actor = actor()?;
        self.store.update(|current| {
            let mut log = current.expect("Yard Log was read");
            log.settle(at, actor);
            Ok::<_, YardStoreError>((log, ()))
        })?;
        Ok(self.store.read()?)
    }
}

/// The revision of Yard Log as it is held: 0 where the branch has none.
pub fn revision_of(held: &Held) -> u64 {
    match held {
        Held::Log(log) => log.root.revision,
        Held::Absent | Held::Newer(_) => 0,
    }
}

/// Yard Log as it is held, where it is one this `loft` reads.
pub fn log_of(held: &Held) -> Option<&YardLog> {
    match held {
        Held::Log(log) => Some(log),
        Held::Absent | Held::Newer(_) => None,
    }
}

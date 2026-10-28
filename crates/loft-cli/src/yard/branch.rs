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
use super::failure::{self, Failure};
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
        // Yard Log removed since it was read, as by a checkout, is left absent: nothing settles.
        let _gone = self.update(|current| match current {
            Some(mut log) => {
                log.settle(at, actor);
                Ok((log, ()))
            }
            None => Err(Unapplied::Refused(())),
        })?;
        Ok(self.store.read()?)
    }
}

/// Why a change under Yard Log's lock was not applied: the store failed, or the change refused
/// with `R`.
pub enum Unapplied<R> {
    Store(YardStoreError),
    Refused(R),
    Failed(Failure),
}

impl<R> From<YardStoreError> for Unapplied<R> {
    fn from(error: YardStoreError) -> Self {
        Self::Store(error)
    }
}

impl Branch {
    /// Applies `change` under Yard Log's lock: its value, or what it refused with, where it
    /// refused and nothing was written.
    pub fn update<T, R>(
        &self,
        change: impl FnOnce(Option<YardLog>) -> Result<(YardLog, T), Unapplied<R>>,
    ) -> Result<Result<T, R>, Failure> {
        match self.store.update(change) {
            Ok(updated) => Ok(Ok(updated.value)),
            Err(Unapplied::Store(error)) => Err(error.into()),
            Err(Unapplied::Refused(refused)) => Ok(Err(refused)),
            Err(Unapplied::Failed(failure)) => Err(failure),
        }
    }

    /// A refusal's result: where it was refused, Yard Log's revision as it is `held`, and why.
    pub fn refused(&self, held: &Held, errors: Vec<Value>) -> Failure {
        Failure::Refused {
            fields: self.fields(revision_of(held)),
            errors,
        }
    }

    /// Refuses where Yard Log as it is `held` is open and its merge closed it all the same.
    pub fn refuse_if_merged(&self, held: &Held) -> Result<(), Failure> {
        if self.merged(held)? {
            return Err(self.refused(held, vec![closed_by_merge()]));
        }
        Ok(())
    }
}

/// The error of a change refused since the merge closed Yard Log.
pub fn closed_by_merge() -> Value {
    failure::error(
        "closed",
        "Yard Log is closed: the base branch holds it, so the merge closed it",
    )
}

/// The error of a change to a branch that has no Yard Log.
pub fn absent() -> Value {
    failure::error("absent", "the branch has no Yard Log")
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

#[cfg(test)]
mod tests {
    use std::fs;

    use yardlog::{Actor, Agent, Key, Time, YardLog};
    use yardlog_store::git::YardGit;
    use yardlog_store::{Held, Place, YardStoreError};

    use super::Branch;
    use crate::yard::args::Beyond;

    #[test]
    fn yard_log_removed_while_it_is_settled_is_found_absent() {
        let dir = std::env::temp_dir().join(format!("loft-branch-{}", Key::mint()));
        let place = Place {
            tree: dir.clone(),
            branch: Some("feat/x".to_owned()),
            common_dir: dir.join(".git"),
            git_yard: dir.join(".git").join("yard"),
        };
        let Ok(branch) = Branch::at(YardGit::default(), place, Beyond { port: 7411 }) else {
            panic!("the branch's Yard Log is found");
        };
        let at = Time::from_unix_seconds(1_792_670_400).unwrap();
        let agent = Agent::main("session-a");
        branch
            .store
            .update(|_| {
                let mut log = YardLog::open("feat/x", "develop", at, agent.clone());
                log.root.unsettled_since = Some(0);
                Ok::<_, YardStoreError>((log, ()))
            })
            .unwrap();
        let folder = branch.store.folder();
        let held = branch.open(at, || {
            // Between the read and the settling, a checkout or `git clean` removes the folder.
            fs::remove_dir_all(&folder).unwrap();
            Ok(Actor::Agent(agent.clone()))
        });
        let written = folder.exists();
        fs::remove_dir_all(&dir).ok();
        assert!(matches!(held, Ok(Held::Absent)));
        assert!(!written, "nothing is written");
    }
}

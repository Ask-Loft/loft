//! One branch's Yard Log, read and written whole. A write takes the branch's lock for as long as
//! it lasts and no longer, first finishes or undoes a write that was interrupted, reads Yard Log
//! as it now is, and saves only the files whose bytes change, through the journal. A read takes
//! the same lock for as long as it reads, so it never sees a write half done.

use std::path::PathBuf;

use std::collections::{BTreeMap, BTreeSet};

use loft_ports::journal::Journal;
use yardlog::codec::{FileError, Partial, ROOT};
use yardlog::log::Root;
use yardlog::{Format, Lineage, YardLog};

use crate::files::{Folder, YARD};
use crate::home::{Home, HomeFile};
use crate::lock;
use crate::outbox::Outbox;
use crate::{Place, YardStoreError};

/// Yard Log as a read finds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Held {
    /// The branch has no Yard Log yet.
    Absent,
    Log(Box<YardLog>),
    /// Yard Log is written in a format newer than this `loft` reads.
    Newer(Format),
}

/// What a write returned, the files it wrote, by their paths from the top of the worktree, how
/// many entries it put into the outbox, and a warning that names each version the outbox could
/// not take, where any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Updated<T> {
    pub value: T,
    pub written: Vec<String>,
    pub outboxed: usize,
    pub unsent: Option<String>,
}

/// The store of one branch's Yard Log in one worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YardStore {
    place: Place,
    branch: String,
    folder: Folder,
    /// The machine's `~/.yard/`, whose chains of continuations each log read carries; with
    /// none, a log compares sessions as they are.
    home: Option<Home>,
    /// In connected mode, the outbox each new version is copied into.
    outbox: Option<Outbox>,
}

impl YardStore {
    /// The store of the Yard Log of the branch the worktree is on.
    ///
    /// # Errors
    ///
    /// The worktree is on a detached HEAD, or its branch cannot name a folder.
    pub fn new(place: Place) -> Result<Self, YardStoreError> {
        let branch = place.branch()?.to_owned();
        let folder = Folder::of(&place.tree, &branch)?;
        Ok(Self {
            place,
            branch,
            folder,
            home: Home::from_env(),
            outbox: None,
        })
    }

    /// The store, reading the chains of continuations from `home` in place of `~/.yard/`.
    #[must_use]
    pub fn with_home(self, home: Option<Home>) -> Self {
        Self { home, ..self }
    }

    /// The store, copying each version a write adds into `outbox`, in connected mode.
    #[must_use]
    pub fn with_outbox(self, outbox: Option<Outbox>) -> Self {
        Self { outbox, ..self }
    }

    #[must_use]
    pub fn place(&self) -> &Place {
        &self.place
    }

    #[must_use]
    pub fn branch(&self) -> &str {
        &self.branch
    }

    /// Yard Log's folder.
    #[must_use]
    pub fn folder(&self) -> PathBuf {
        self.folder.path()
    }

    /// Yard Log's folder from the top of the worktree, `.yard/<branch>`.
    #[must_use]
    pub fn folder_rel(&self) -> &str {
        self.folder.rel()
    }

    /// The lock under which one write at a time is applied to this Yard Log.
    #[must_use]
    pub fn lock_path(&self) -> PathBuf {
        self.place
            .git_yard
            .join(format!("{}.lock", file_name(&self.branch)))
    }

    /// The journal through which this Yard Log's files are saved.
    #[must_use]
    pub fn journal(&self) -> Journal {
        let dir = self
            .place
            .git_yard
            .join(format!("{}.save", file_name(&self.branch)));
        Journal::new(&self.place.tree, dir).keeping(YARD)
    }

    /// Yard Log as it is.
    ///
    /// # Errors
    ///
    /// A file of Yard Log does not read, an interrupted write cannot be finished, or the lock
    /// cannot be taken.
    pub fn read(&self) -> Result<Held, YardStoreError> {
        let _locked = self.locked()?;
        Ok(self.load()?.0)
    }

    /// Yard Log for what this `loft` can read of it, as Yard Log of a newer format is shown:
    /// none where the branch has no Yard Log. It is never written.
    ///
    /// # Errors
    ///
    /// The root does not read even so, a file cannot be read, an interrupted write cannot be
    /// finished, or the lock cannot be taken.
    pub fn read_partial(&self) -> Result<Option<Partial>, YardStoreError> {
        let _locked = self.locked()?;
        let Some(root) = self.folder.root()? else {
            return Ok(None);
        };
        let (partial, _) = self.with_files(|stage| YardLog::read_partial(&root, stage))?;
        Ok(Some(partial))
    }

    /// Applies one write: `change` is given Yard Log as it is, none where the branch has none,
    /// and returns Yard Log as it is to be written, or refuses and changes nothing.
    ///
    /// # Errors
    ///
    /// `change` refuses; a file of Yard Log does not read; Yard Log is of a newer format; an
    /// interrupted write cannot be finished; or a file cannot be written. A write that fails
    /// before its journal commits leaves every file as it was, and one that fails after is
    /// finished by the next.
    pub fn update<T, E>(
        &self,
        change: impl FnOnce(Option<YardLog>) -> Result<(YardLog, T), E>,
    ) -> Result<Updated<T>, E>
    where
        E: From<YardStoreError>,
    {
        let _locked = self.locked()?;
        let (held, read) = self.load()?;
        let current = match held {
            Held::Absent => {
                // The page server keeps live copies only while Yard Log is open, so any found as
                // it opens were left by an earlier Yard Log under the branch's name.
                self.remove_live();
                None
            }
            Held::Log(log) => {
                self.adopt_live(&log.root.branch);
                Some(self.renamed(*log))
            }
            Held::Newer(found) => {
                return Err(YardStoreError::Newer {
                    found,
                    writes: Format::CURRENT,
                }
                .into());
            }
        };
        // Only the outbox needs the versions held before the change.
        let before = match (&self.outbox, &current) {
            (Some(_), Some(log)) => version_keys(log),
            _ => BTreeSet::new(),
        };
        let (next, value) = change(current)?;
        let mut changes = self.folder.changes(&read, &next);
        let saved_all = self.save_live(&mut changes);
        self.journal()
            .save(&changes)
            .map_err(YardStoreError::from)?;
        // Copied once the write is saved and before the lock is released: the outbox never holds
        // a version Yard Log does not.
        let (outboxed, unsent) = self
            .outbox
            .as_ref()
            .map_or((0, None), |outbox| outbox.put_new(&before, &next));
        if next.is_closed() && saved_all {
            // Saved above, the live copies take no more events once Yard Log is closed; one left
            // behind holds what the folder holds, so a failure to remove it loses nothing. A copy
            // that could not be saved is kept, with the rest.
            self.remove_live();
        }
        Ok(Updated {
            value,
            written: changes
                .write
                .keys()
                .map(|path| path.as_str().to_owned())
                .collect(),
            outboxed,
            unsent,
        })
    }

    /// Yard Log whose folder was moved to the branch's new name, recording the new name and
    /// keeping the name it was opened with.
    fn renamed(&self, mut log: YardLog) -> YardLog {
        let root = &mut log.root;
        if root.branch != self.branch {
            let was = std::mem::replace(&mut root.branch, self.branch.clone());
            let opened = root.opened_as.take().unwrap_or(was);
            root.opened_as = (opened != self.branch).then_some(opened);
        }
        log
    }

    /// Yard Log's root alone, as a listing shows it, with no stage read: none where the branch
    /// has none, and the format of a root newer than this `loft` reads.
    ///
    /// # Errors
    ///
    /// The root does not read, an interrupted write cannot be finished, or the lock cannot be
    /// taken.
    pub fn read_root(&self) -> Result<Option<Result<Root, Format>>, YardStoreError> {
        let _locked = self.locked()?;
        let Some(bytes) = self.folder.root()? else {
            return Ok(None);
        };
        let unreadable = |error| YardStoreError::Unreadable {
            folder: self.folder.rel().to_owned(),
            error,
        };
        let format = yardlog::codec::read_format(&bytes).map_err(unreadable)?;
        if !format.is_known() {
            return Ok(Some(Err(format)));
        }
        let root = yardlog::codec::read_root(&bytes).map_err(unreadable)?;
        Ok(Some(Ok(root)))
    }

    /// Takes the branch's lock and finishes or undoes an interrupted write.
    pub(crate) fn locked(&self) -> Result<lock::Locked, YardStoreError> {
        let locked = lock::lock(&self.lock_path())?;
        self.journal().recover()?;
        Ok(locked)
    }

    /// Yard Log as it is, with the bytes of each file read for it, by name.
    fn load(&self) -> Result<(Held, Read), YardStoreError> {
        let Some(root) = self.folder.root()? else {
            return Ok((Held::Absent, Read::new()));
        };
        let (held, mut read) = self.with_files(|stage| YardLog::read_or_newer(&root, stage))?;
        read.insert(ROOT.to_owned(), root);
        let held = match held {
            Ok(mut log) => {
                log.lineage = self.lineage()?;
                Held::Log(Box::new(log))
            }
            Err(found) => Held::Newer(found),
        };
        Ok((held, read))
    }

    /// The chains of continuations the machine records.
    fn lineage(&self) -> Result<Lineage, YardStoreError> {
        let Some(home) = &self.home else {
            return Ok(Lineage::default());
        };
        Ok(Lineage::read(
            home.read(HomeFile::Continuations)?.as_deref(),
        ))
    }

    /// Runs `reading` over the folder's files, keeping the bytes of each it reads; a file that
    /// cannot be read fails the whole read, and one that does not read names the folder.
    fn with_files<T>(
        &self,
        reading: impl FnOnce(&mut dyn FnMut(&str) -> Option<Vec<u8>>) -> Result<T, FileError>,
    ) -> Result<(T, Read), YardStoreError> {
        let mut failed = None;
        let mut read = Read::new();
        let value = reading(&mut |name| match self.folder.read(name) {
            Ok(bytes) => {
                if let Some(bytes) = &bytes {
                    read.insert(name.to_owned(), bytes.clone());
                }
                bytes
            }
            Err(error) => {
                failed.get_or_insert(error);
                None
            }
        });
        if let Some(error) = failed {
            return Err(error);
        }
        let value = value.map_err(|error| YardStoreError::Unreadable {
            folder: self.folder.rel().to_owned(),
            error,
        })?;
        Ok((value, read))
    }
}

/// The key of every version `log` holds.
fn version_keys(log: &YardLog) -> BTreeSet<yardlog::keys::VersionKey> {
    log.versions().map(|(_, _, version)| version.key).collect()
}

/// `branch`'s name as a single file name: `%` and `/` escaped, so that no two branches, nor a
/// branch and a directory of another's, share a name.
pub(crate) fn file_name(branch: &str) -> String {
    branch.replace('%', "%25").replace('/', "%2F")
}

/// The bytes of each file of a Yard Log's folder that a read met, by name.
pub(crate) type Read = BTreeMap<String, Vec<u8>>;

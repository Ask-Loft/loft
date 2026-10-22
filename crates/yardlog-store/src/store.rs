//! One branch's Yard Log, read and written whole. A write takes the branch's lock for as long as
//! it lasts and no longer, first finishes or undoes a write that was interrupted, reads Yard Log
//! as it now is, and saves only the files whose bytes change, through the journal. A read takes
//! the same lock for as long as it reads, so it never sees a write half done.

use std::path::{Path, PathBuf};

use loft_ports::journal::Journal;
use yardlog::{Format, YardLog};

use crate::files::{Folder, YARD};
use crate::lock;
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

/// What a write returned, and the files it wrote, by their paths from the top of the worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Updated<T> {
    pub value: T,
    pub written: Vec<String>,
}

/// The store of one branch's Yard Log in one worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YardStore {
    place: Place,
    branch: String,
    folder: Folder,
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
        })
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

    /// The branch's name as a single file name: `%` and `/` escaped, so that no two branches,
    /// nor a branch and a directory of another's, share a name.
    fn file_name(&self) -> String {
        self.branch.replace('%', "%25").replace('/', "%2F")
    }

    /// The lock under which one write at a time is applied to this Yard Log.
    #[must_use]
    pub fn lock_path(&self) -> PathBuf {
        self.place
            .git_yard
            .join(format!("{}.lock", self.file_name()))
    }

    /// The journal through which this Yard Log's files are saved.
    #[must_use]
    pub fn journal(&self) -> Journal {
        let dir = self
            .place
            .git_yard
            .join(format!("{}.save", self.file_name()));
        Journal::new(&self.place.tree, dir).keeping(YARD)
    }

    /// Yard Log as it is.
    ///
    /// # Errors
    ///
    /// A file of Yard Log does not read, an interrupted write cannot be finished, or the lock
    /// cannot be taken.
    pub fn read(&self) -> Result<Held, YardStoreError> {
        let _locked = lock::lock(&self.lock_path())?;
        self.journal().recover()?;
        self.load()
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
        let _locked = lock::lock(&self.lock_path())?;
        self.journal().recover().map_err(YardStoreError::from)?;
        let current = match self.load()? {
            Held::Absent => None,
            Held::Log(log) => Some(self.renamed(*log)),
            Held::Newer(found) => {
                return Err(YardStoreError::Newer {
                    found,
                    writes: Format::CURRENT,
                }
                .into());
            }
        };
        let (next, value) = change(current)?;
        let changes = self.folder.changes(&next)?;
        self.journal()
            .save(&changes)
            .map_err(YardStoreError::from)?;
        Ok(Updated {
            value,
            written: changes
                .write
                .keys()
                .map(|path| path.as_str().to_owned())
                .collect(),
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

    fn load(&self) -> Result<Held, YardStoreError> {
        let Some(root) = self.folder.root()? else {
            return Ok(Held::Absent);
        };
        let unreadable = |error| YardStoreError::Unreadable {
            folder: self.folder.rel().to_owned(),
            error,
        };
        let format = yardlog::codec::read_format(&root).map_err(unreadable)?;
        if !format.is_known() {
            return Ok(Held::Newer(format));
        }
        let mut failed = None;
        let read = YardLog::read(&root, |name| {
            self.folder.read(name).unwrap_or_else(|error| {
                failed.get_or_insert(error);
                None
            })
        });
        if let Some(error) = failed {
            return Err(error);
        }
        read.map(|log| Held::Log(Box::new(log))).map_err(unreadable)
    }
}

/// The folder that holds every branch's Yard Log in the worktree whose top is `tree`.
#[must_use]
pub fn yard_dir(tree: &Path) -> PathBuf {
    tree.join(YARD)
}

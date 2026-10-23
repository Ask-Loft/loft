//! Locks on files under git's own directory for the worktree: the lock under which one write at a
//! time is applied to a branch's Yard Log, and the lease a running wait holds on its submission.
//! Each is the operating system's lock on an open file, so it is free the moment the process
//! that holds it ends, however it ends, and a lock left by a process that ended needs no
//! clearing. The files stay where they are, empty; removing one could part two processes that
//! each think they hold it. The file beside a lease that names its holder is removed when the
//! holder lets go; only a holder that was killed leaves its name, for the next to replace.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use loft_ports::io_error;
use loft_ports::sync::{read_if_present, replace_synced};

use crate::YardStoreError;

/// A lock this process holds, until it is dropped.
#[derive(Debug)]
pub struct Locked {
    file: File,
    path: PathBuf,
    /// The file that names the holder, where [`try_lock_as`] wrote one: removed before the lock
    /// is let go, so that a later holder's name is never read as this one's.
    named: Option<PathBuf>,
}

impl Locked {
    /// The locked file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Locked {
    fn drop(&mut self) {
        if let Some(named) = &self.named {
            let _ = fs::remove_file(named);
        }
        let _ = self.file.unlock();
    }
}

fn open(path: &Path) -> Result<File, YardStoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_error(parent))?;
    }
    Ok(OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .map_err(io_error(path))?)
}

/// Takes the lock on `path`, waiting while another process or another handle holds it.
///
/// # Errors
///
/// The file cannot be created, opened or locked.
pub fn lock(path: &Path) -> Result<Locked, YardStoreError> {
    let file = open(path)?;
    file.lock().map_err(io_error(path))?;
    Ok(Locked {
        file,
        path: path.to_owned(),
        named: None,
    })
}

/// Takes the lock on `path` where it is free, and none where another holds it.
///
/// # Errors
///
/// The file cannot be created, opened or locked.
pub fn try_lock(path: &Path) -> Result<Option<Locked>, YardStoreError> {
    let file = open(path)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(Locked {
            file,
            path: path.to_owned(),
            named: None,
        })),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(error)) => Err(io_error(path)(error).into()),
    }
}

/// Whether another holds the lock on `path` now.
///
/// # Errors
///
/// The file cannot be created, opened or locked.
pub fn is_held(path: &Path) -> Result<bool, YardStoreError> {
    Ok(try_lock(path)?.is_none())
}

/// The file beside a lock that names its holder. It is a file of its own, not the lock's, since
/// on some systems a locked file cannot be read by another process.
fn holder_path(path: &Path) -> PathBuf {
    path.with_extension("holder")
}

/// Takes the lock on `path` where it is free, as [`try_lock`] does, and records `holder`, who
/// holds it, beside it.
///
/// # Errors
///
/// The file cannot be locked, or its holder's file cannot be written.
pub fn try_lock_as(path: &Path, holder: &[u8]) -> Result<Option<Locked>, YardStoreError> {
    let Some(mut locked) = try_lock(path)? else {
        return Ok(None);
    };
    // Replaced whole, so that a reader sees one holder's name or the one before, never half of
    // either.
    let named = holder_path(path);
    replace_synced(&named, holder, false)?;
    locked.named = Some(named);
    Ok(Some(locked))
}

/// Who holds the lock on `path`, as [`try_lock_as`] recorded it, read without taking the lock,
/// so that looking never holds it from one that would take it: none where no holder has named
/// itself, as where the lock is free or its holder has not yet written its name. A holder takes
/// its name away before it lets go; only one that was killed leaves it.
///
/// # Errors
///
/// The holder's file cannot be read.
pub fn holder(path: &Path) -> Result<Option<Vec<u8>>, YardStoreError> {
    Ok(read_if_present(&holder_path(path))?)
}

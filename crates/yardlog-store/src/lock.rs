//! Locks on files under git's own directory for the worktree: the lock under which one write at a
//! time is applied to a branch's Yard Log, and the lease a running wait holds on its submission.
//! Each is the operating system's lock on an open file, so it is free the moment the process
//! that holds it ends, however it ends, and a lock left by a process that ended needs no
//! clearing. The files stay where they are, empty; removing one could part two processes that
//! each think they hold it.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use loft_ports::io_error;

use crate::YardStoreError;

/// A lock this process holds, until it is dropped.
#[derive(Debug)]
pub struct Locked {
    file: File,
    path: PathBuf,
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

//! Writing to disk so that a write survives a crash or a power loss: a file synced before it is
//! renamed into place, and a directory synced after a rename into it.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{PortError, io_error};

/// A file's bytes, or none where it is not there.
///
/// # Errors
///
/// The file is there and cannot be read.
pub fn read_if_present(path: &Path) -> Result<Option<Vec<u8>>, PortError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error(path)(error)),
    }
}

/// Removes a file, where it is there.
///
/// # Errors
///
/// The file is there and cannot be removed.
pub fn remove_if_present(path: &Path) -> Result<(), PortError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(path)(error)),
    }
}

/// Replaces the file at `path` with `bytes`: written whole and synced to a temporary file beside
/// it, readable only by its owner where `private`, then moved into place, and the folder synced,
/// so a reader sees the old bytes or the new, never part of either. The folder must be there.
///
/// # Errors
///
/// The temporary file cannot be written, or cannot be moved into place.
pub fn replace_synced(path: &Path, bytes: &[u8], private: bool) -> Result<(), PortError> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let temporary = path.with_file_name(format!(
        ".{name}.{}.{}.tmp",
        process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let written =
        write_new(&temporary, bytes, private).and_then(|()| move_into_place(&temporary, path));
    if written.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    written?;
    match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => sync_dir(dir),
        _ => Ok(()),
    }
}

/// Writes a new file, synced, readable only by its owner where `private`.
fn write_new(path: &Path, bytes: &[u8], private: bool) -> Result<(), PortError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = private;
    let mut handle = options.open(path).map_err(io_error(path))?;
    handle.write_all(bytes).map_err(io_error(path))?;
    handle.sync_all().map_err(io_error(path))
}

/// Writes `bytes` to a new file at `path`, synced to disk.
///
/// # Errors
///
/// The file cannot be created, written or synced.
pub fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), PortError> {
    let mut file = fs::File::create(path).map_err(io_error(path))?;
    file.write_all(bytes).map_err(io_error(path))?;
    file.sync_all().map_err(io_error(path))
}

/// Syncs a directory, so the renames into it survive a power loss. Only Unix can open a
/// directory to sync it.
///
/// # Errors
///
/// The directory cannot be opened or synced.
pub fn sync_dir(dir: &Path) -> Result<(), PortError> {
    #[cfg(unix)]
    fs::File::open(dir)
        .and_then(|handle| handle.sync_all())
        .map_err(io_error(dir))?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

/// Moves `from` to `to`, replacing `to`. Where the two lie on different file systems, which a
/// rename cannot cross, `from` is copied beside `to`, synced and renamed into place, then
/// removed; run again after an interruption, it does the same.
///
/// # Errors
///
/// The file cannot be renamed, copied or removed.
pub fn move_into_place(from: &Path, to: &Path) -> Result<(), PortError> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices => {
            let beside = to.with_file_name(format!(
                ".{}.loft-moving",
                to.file_name().unwrap_or_default().to_string_lossy()
            ));
            let bytes = fs::read(from).map_err(io_error(from))?;
            write_synced(&beside, &bytes)?;
            fs::rename(&beside, to).map_err(io_error(to))?;
            fs::remove_file(from).map_err(io_error(from))
        }
        Err(error) => Err(io_error(to)(error)),
    }
}

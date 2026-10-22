//! Writing to disk so that a write survives a crash or a power loss: a file synced before it is
//! renamed into place, and a directory synced after a rename into it.

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::{PortError, io_error};

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

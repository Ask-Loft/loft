//! Writing a package: the files go to a directory beside the skill's directory, which is then
//! renamed into its place, so that the skill's directory holds one package whole.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;

/// Replaces `directory` with one holding exactly `files`, each at its relative path.
///
/// # Errors
///
/// A file or directory that cannot be written, renamed or removed.
pub fn replace(directory: &Path, files: &[(String, String)]) -> io::Result<()> {
    let fresh = beside(directory, "new");
    remove(&fresh)?;
    for (path, text) in files {
        let at = fresh.join(path);
        fs::create_dir_all(at.parent().expect("a file lies in a directory"))?;
        fs::write(at, text)?;
    }
    if directory.exists() {
        let old = beside(directory, "old");
        remove(&old)?;
        fs::rename(directory, &old)?;
        fs::rename(&fresh, directory)?;
        fs::remove_dir_all(&old)
    } else {
        fs::rename(&fresh, directory)
    }
}

/// A hidden directory beside `directory`, named for its use and this process.
fn beside(directory: &Path, purpose: &str) -> PathBuf {
    let name = directory
        .file_name()
        .expect("a skill directory has a name")
        .to_string_lossy();
    directory.with_file_name(format!(".{name}.{purpose}-{}", process::id()))
}

fn remove(directory: &Path) -> io::Result<()> {
    match fs::remove_dir_all(directory) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

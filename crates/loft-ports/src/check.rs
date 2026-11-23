//! What a journal may name. A journal's plan is a file on disk, which anything may have written
//! there, a repository's own history included, so a journal is checked whole before a file
//! moves: each path is a plain path of a file the journal saves, outside git's own directory
//! and behind no link that leads out of the root, and each staged file is one of the journal's
//! own. A save checks its own paths the same way, so that it never commits a plan the next
//! command would refuse.

use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path};

use crate::journal::Journal;

/// The name of git's own directory, which no save writes into.
const GIT: &str = ".git";

/// Why the journal does not save `path`, where it does not.
pub(crate) fn path(journal: &Journal, path: &str) -> Result<(), &'static str> {
    if !plain(path) {
        return Err("is not a plain path under the directory the journal saves in");
    }
    if path.split('/').any(of_git) {
        return Err("lies in git's own directory");
    }
    if !journal.saves(path) {
        return Err("is not a file the journal saves");
    }
    within(journal.root(), path)
}

/// Why the journal does not put the staged file `name` of the directory `staged` in place,
/// where it does not: a file is staged under a number, and is a regular file where it is still
/// there.
pub(crate) fn staged(staged: &Path, name: &str) -> Result<(), &'static str> {
    if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("is not a staged file's name");
    }
    match fs::symlink_metadata(staged.join(name)) {
        Ok(found) if !found.file_type().is_file() => Err("is not a regular file"),
        _ => Ok(()),
    }
}

/// Whether `path` is one or more parts separated by `/`, each a file's or a directory's own
/// name: no empty part, no `.` or `..`, nothing the platform reads as a root, a drive or two
/// parts.
fn plain(path: &str) -> bool {
    path.split('/').all(|part| {
        let mut components = Path::new(part).components();
        let only = (components.next(), components.next());
        !part.contains('\0')
            && matches!(only, (Some(Component::Normal(name)), None) if name == Path::new(part))
    })
}

/// Whether `part` names git's own directory, in any letter case.
fn of_git(part: &str) -> bool {
    part.eq_ignore_ascii_case(GIT)
}

/// Refuses `path` where a link takes the directory that holds it out of `root`, or into git's
/// own directory: the deepest directory above the file that is there is where the file would
/// be written, whatever the path says.
fn within(root: &Path, path: &str) -> Result<(), &'static str> {
    const LINKED: &str = "lies behind a link that leads out of the directory the journal saves in";
    let Ok(root) = fs::canonicalize(root) else {
        // Nothing is under a root that is not there, so no link is.
        return Ok(());
    };
    let target = root.join(path);
    let mut dir = target.parent();
    while let Some(current) = dir {
        match fs::symlink_metadata(current) {
            Err(error) if error.kind() == ErrorKind::NotFound => dir = current.parent(),
            _ => break,
        }
    }
    let real = dir
        .map(fs::canonicalize)
        .and_then(Result::ok)
        .ok_or(LINKED)?;
    let inside = real.strip_prefix(&root).map_err(|_| LINKED)?;
    let in_git = inside
        .components()
        .any(|part| part.as_os_str().to_str().is_some_and(of_git));
    if in_git {
        return Err("lies behind a link into git's own directory");
    }
    Ok(())
}

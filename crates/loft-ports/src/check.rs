//! What a journal may name. A journal's plan is a file on disk, which anything may have written
//! there, a repository's own history included, so a journal is checked whole before a file
//! moves: its own directory is where it is named, each path is a plain path of a file the
//! journal saves, outside git's own directory and behind no link that leads out of where the
//! journal saves it, and each staged file is one of the journal's own. A save checks its own
//! paths the same way, so that it never commits a plan the next command would refuse.

use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use crate::journal::Journal;

/// The name of git's own directory, which no save writes into.
const GIT: &str = ".git";

/// The short name a file system that keeps one gives git's own directory.
const GIT_SHORT: &str = "git~1";

/// A path the journal does not save, and why.
pub(crate) struct Refused<'p> {
    pub(crate) path: &'p str,
    pub(crate) why: &'static str,
}

/// Refuses the first of `paths` that the journal does not save.
pub(crate) fn paths<'p>(
    journal: &Journal,
    paths: impl IntoIterator<Item = &'p str>,
) -> Result<(), Refused<'p>> {
    let mut ground = None;
    for path in paths {
        let refused = |why| Refused { path, why };
        named(journal, path).map_err(refused)?;
        if ground.is_none() {
            ground = Some(Ground::of(journal).map_err(refused)?);
        }
        let ground = ground.as_ref().expect("the ground was just found");
        ground.holds(journal, path).map_err(refused)?;
    }
    Ok(())
}

/// Why the journal does not save a file under the name `path`, where it does not, whatever
/// lies on disk.
fn named(journal: &Journal, path: &str) -> Result<(), &'static str> {
    if !path.split('/').all(plain) {
        return Err("is not a plain path under the directory the journal saves in");
    }
    if path.split('/').any(of_git) {
        return Err("names git's own directory");
    }
    if !journal.saves(path) {
        return Err("is not a file the journal saves");
    }
    Ok(())
}

/// Whether `part` is a file's or a directory's own name: not empty, `.` or `..`, nothing the
/// platform reads as a root, a drive or two parts, and without the `:` and the backslash that
/// some platforms read as more than a name.
fn plain(part: &str) -> bool {
    !part.contains(['\0', ':', '\\'])
        && Path::new(part)
            .components()
            .eq([Component::Normal(part.as_ref())])
}

/// Whether a file system may read `part` as git's own directory: in any letter case, with
/// trailing dots and spaces, with the characters a file system ignores in a name, or as the
/// short name of a file system that keeps one.
fn of_git(part: &str) -> bool {
    let ignored = |character: char| matches!(character, '\u{200c}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{206a}'..='\u{206f}' | '\u{feff}');
    let read: String = part
        .chars()
        .filter(|character| !ignored(*character))
        .collect();
    let read = read.trim_end_matches(['.', ' ']);
    read.eq_ignore_ascii_case(GIT) || read.eq_ignore_ascii_case(GIT_SHORT)
}

/// Why the journal's own directory is not where it is named, where it is not: below the root, no
/// part of it, and not its directory of staged files, is a link, so that what recovery moves
/// and deletes is the journal's and nothing a link leads to. A journal outside the root, in a
/// directory its caller keeps, is its caller's to vouch for.
pub(crate) fn place(journal: &Journal, staged: &str) -> Result<(), &'static str> {
    let Ok(below) = journal.dir().strip_prefix(journal.root()) else {
        return Ok(());
    };
    let mut part = journal.root().to_owned();
    for name in below
        .components()
        .chain([Component::Normal(staged.as_ref())])
    {
        part.push(name);
        if fs::symlink_metadata(&part).is_ok_and(|found| found.file_type().is_symlink()) {
            return Err("lies behind a link, so no save wrote it there");
        }
    }
    Ok(())
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

/// Where the journal's files really lie, with every link followed: the root and, where the
/// journal has a floor that is there, the floor.
struct Ground {
    /// None where the root is not there, and so nothing under it is.
    root: Option<PathBuf>,
    floor: Option<PathBuf>,
}

const LINKED: &str = "lies behind a link that leads out of where the journal saves it";

impl Ground {
    fn of(journal: &Journal) -> Result<Self, &'static str> {
        let root = match fs::canonicalize(journal.root()) {
            Ok(root) => root,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(Self {
                    root: None,
                    floor: None,
                });
            }
            Err(_) => return Err("lies under a directory whose links cannot be followed"),
        };
        let floor = match journal.floor_name() {
            Some(name) if fs::symlink_metadata(journal.root().join(name)).is_ok() => {
                let floor = fs::canonicalize(journal.root().join(name)).map_err(|_| LINKED)?;
                if !floor.starts_with(&root) {
                    return Err(LINKED);
                }
                Some(floor)
            }
            _ => None,
        };
        Ok(Self {
            root: Some(root),
            floor,
        })
    }

    /// Refuses `path` where a link takes the directory that holds it out of where the journal
    /// saves it, the floor for a path under the floor and the root for any other, or into git's
    /// own directory: the deepest directory above the file that is there is where the file
    /// would be written, whatever the path says.
    fn holds(&self, journal: &Journal, path: &str) -> Result<(), &'static str> {
        let Some(root) = &self.root else {
            return Ok(());
        };
        let within = match &self.floor {
            Some(floor) if journal.under_floor(path) => floor,
            _ => root,
        };
        let target = journal.root().join(path);
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
        if !real.starts_with(within) {
            return Err(LINKED);
        }
        let inside = real.strip_prefix(root).map_err(|_| LINKED)?;
        let in_git = inside
            .components()
            .any(|part| part.as_os_str().to_str().is_some_and(of_git));
        if in_git {
            return Err("lies behind a link into git's own directory");
        }
        Ok(())
    }
}

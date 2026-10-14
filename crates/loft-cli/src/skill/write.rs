//! Writing a package: the files go to a directory beside the skill's directory, which is then
//! renamed into its place, so that the skill's directory holds one package whole. A write that
//! fails leaves the skill's directory as it was and no directory beside it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;

/// Replaces `directory` with one holding exactly `files`, each at its relative path.
///
/// # Errors
///
/// A file or directory that cannot be written, renamed or removed. The directory staged beside
/// `directory` is then removed, and `directory` is left as it was unless the package is already
/// in its place and only the directory it replaced cannot be removed.
pub fn replace(directory: &Path, files: &[(String, String)]) -> io::Result<()> {
    let staged = stage(directory, files)?;
    swap(&staged, directory).inspect_err(|_| discard(&staged))
}

/// Writes `files` into a new directory beside `directory`, and returns its path. `directory`
/// itself is not touched.
///
/// # Errors
///
/// A file or directory that cannot be written; whatever was staged is then removed.
fn stage(directory: &Path, files: &[(String, String)]) -> io::Result<PathBuf> {
    let staged = beside(directory, "new");
    remove(&staged)?;
    let written = files.iter().try_for_each(|(path, text)| {
        let at = staged.join(path);
        fs::create_dir_all(at.parent().expect("a file lies in a directory"))?;
        fs::write(at, text)
    });
    written
        .map(|()| staged.clone())
        .inspect_err(|_| discard(&staged))
}

/// Renames `staged` into the place of `directory`, keeping the directory it replaces aside
/// until it is in place, then removing that one.
///
/// # Errors
///
/// A directory that cannot be renamed or removed. A rename that fails puts the directory kept
/// aside back in its place.
fn swap(staged: &Path, directory: &Path) -> io::Result<()> {
    if !directory.exists() {
        return fs::rename(staged, directory);
    }
    let old = beside(directory, "old");
    remove(&old)?;
    fs::rename(directory, &old)?;
    if let Err(error) = fs::rename(staged, directory) {
        // The error that stopped the swap is the one reported, even if the restore fails too.
        fs::rename(&old, directory).ok();
        return Err(error);
    }
    fs::remove_dir_all(&old)
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

/// Removes a staged directory after a failure, which the error already reports.
fn discard(staged: &Path) {
    remove(staged).ok();
}

#[cfg(test)]
mod tests {
    use super::{stage, swap};
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::{env, fs, process};

    /// Every file under `dir`, by its path from `root`, with its text.
    fn files(root: &Path, dir: &Path, found: &mut BTreeMap<String, String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files(root, &path, found);
                continue;
            }
            let relative = path.strip_prefix(root).unwrap().display().to_string();
            found.insert(relative, fs::read_to_string(&path).unwrap());
        }
    }

    fn listed(dir: &Path) -> BTreeMap<String, String> {
        let mut found = BTreeMap::new();
        files(dir, dir, &mut found);
        found
    }

    #[test]
    fn the_package_is_staged_whole_beside_the_skill_directory_which_is_left_untouched() {
        let root = env::temp_dir().join(format!("loft-cli-stage-{}", process::id()));
        let directory = root.join("skills/loft-spec");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("mine.md"), "kept").unwrap();
        let package = [
            ("SKILL.md".to_owned(), "skill".to_owned()),
            ("references/format.md".to_owned(), "format".to_owned()),
        ];
        let staged = stage(&directory, &package).unwrap();
        assert_eq!(staged.parent(), directory.parent());
        assert_ne!(staged, directory);
        assert_eq!(listed(&staged), BTreeMap::from(package));
        assert_eq!(
            listed(&directory),
            BTreeMap::from([("mine.md".to_owned(), "kept".to_owned())])
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_swap_that_fails_puts_the_replaced_directory_back_and_leaves_nothing_beside_it() {
        let root = env::temp_dir().join(format!("loft-cli-swap-{}", process::id()));
        let directory = root.join("skills/loft-spec");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("mine.md"), "kept").unwrap();
        let missing = root.join("skills/.loft-spec.never-staged");
        assert!(swap(&missing, &directory).is_err());
        assert_eq!(
            listed(&directory),
            BTreeMap::from([("mine.md".to_owned(), "kept".to_owned())])
        );
        let names: Vec<String> = fs::read_dir(root.join("skills"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, ["loft-spec"]);
        fs::remove_dir_all(root).unwrap();
    }
}
